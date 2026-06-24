use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::io::Read;
use md5::{Md5, Digest};
use tokio::fs as tokio_fs;
use axum::body::Bytes;
use tracing::info;

use crate::config::AppConfig;
use crate::repositories::video_repo::VideoRepository;
use crate::models::video::{VideoItem, FileCheckItem};

#[derive(Clone)]
pub struct VideoService {
    repo: VideoRepository,
    config: AppConfig,
}

impl VideoService {
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        Self { repo, config }
    }

    /// Resolve a `/media/...` URL to a safe filesystem path inside media_root.
    /// Returns None if the path would escape media_root (path traversal blocked).
    fn safe_media_path(&self, url: &str) -> Option<PathBuf> {
        safe_media_path(url, &self.config.media_root)
    }

    #[allow(dead_code)]
    pub async fn list_videos(&self, query: Option<&str>) -> Result<Vec<VideoItem>, sqlx::Error> {
        let rows = self.repo.find_all(query).await?;
        Ok(rows.into_iter().map(VideoItem::from).collect())
    }

    pub async fn list_videos_paged(
        &self,
        page: i64,
        size: i64,
        query: Option<&str>,
        source_type: Option<&str>,
        category: Option<&str>,
        username: Option<&str>,
    ) -> Result<(Vec<VideoItem>, i64), sqlx::Error> {
        let total = self.repo.count_all(query, source_type, category).await?;
        let rows = self.repo.find_all_paged(page, size, query, source_type, category, username).await?;
        let items: Vec<VideoItem> = rows.into_iter().map(VideoItem::from).collect();
        Ok((items, total))
    }

    pub async fn get_video(&self, id: i64) -> Result<Option<VideoItem>, sqlx::Error> {
        let row = self.repo.find_by_id(id).await?;
        Ok(row.map(VideoItem::from))
    }

    pub async fn add_external_video(
        &self,
        title: &str,
        description: Option<&str>,
        category: Option<&str>,
        stream_url: &str,
        cover_url: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let desc = description.unwrap_or("");
        let cat = category.unwrap_or("general");
        let id = self.repo.save_external_video(title, desc, cat, cover_url, stream_url).await?;
        Ok(id)
    }

    pub async fn check_existing_hashes(&self, hashes: Vec<String>) -> Result<Vec<String>, sqlx::Error> {
        self.repo.find_existing_hashes(&hashes).await
    }

    pub async fn check_existing_files(&self, files: &[FileCheckItem]) -> Result<HashSet<usize>, sqlx::Error> {
        // Single batch query instead of N+1
        let pairs: Vec<(String, i64)> = files.iter().map(|f| (f.name.clone(), f.size)).collect();
        let existing_pairs = self.repo.find_existing_by_name_and_size_batch(&pairs).await?;

        let mut existing = HashSet::new();
        for (i, file) in files.iter().enumerate() {
            if existing_pairs.contains(&(file.name.clone(), file.size)) {
                existing.insert(i);
            }
        }
        Ok(existing)
    }

    /// 流式上传：从临时文件读取，计算 MD5，移动到最终位置
    pub async fn upload_video_file(
        &self,
        file_name: &str,
        temp_path: &std::path::Path,
        category: &str,
        client_hash: Option<&str>,
    ) -> Result<i64, String> {
        // Compute MD5 hash by streaming the file
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(temp_path).await
            .map_err(|e| format!("打开临时文件失败: {}", e))?;
        let mut hasher = Md5::new();
        let mut buf = vec![0u8; 65536];
        let mut file_size: i64 = 0;
        loop {
            let n = file.read(&mut buf).await
                .map_err(|e| format!("读取临时文件失败: {}", e))?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            file_size += n as i64;
        }
        drop(file);
        let hash = format!("{:x}", hasher.finalize());

        // Check for duplicates if client provided hash, or always check
        if let Some(ch) = client_hash {
            if ch == hash
                && self.repo.find_video_by_file_hash(&hash).await.map_err(|e| e.to_string())?.is_some()
            {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err("重复：视频已存在".into());
            }
        } else {
            if self.repo.find_video_by_file_hash(&hash).await.map_err(|e| e.to_string())?.is_some() {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err("重复：视频已存在".into());
            }
        }

        // Determine file extension
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "mp4".to_string());

        // 用 magic bytes 验证文件类型
        if let Err(e) = validate_file_type(temp_path, &ext) {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(e);
        }

        let is_video = matches!(ext.as_str(), "mp4" | "m3u8" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv");
        let is_image = matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp");

        let source_type = if is_video { "local_video" } else if is_image { "local_image" } else { "local" };

        // Move temp file to final destination
        let dest_file_name = format!("{}_{}", chrono::Utc::now().timestamp_millis(), file_name);
        let dest_path = self.config.media_root.join(&dest_file_name);
        tokio::fs::rename(temp_path, &dest_path).await
            .map_err(|e| format!("移动文件失败: {}", e))?;

        let stream_url = format!("/media/{}", dest_file_name);

        let id = self.repo.save_local_video(
            file_name,
            "",
            source_type,
            None,
            &stream_url,
            category,
            Some(&hash),
            Some(file_size),
            Some(file_name),
            None,
        ).await.map_err(|e| e.to_string())?;

        info!("Uploaded video id={} as {} ({} bytes, streaming)", id, stream_url, file_size);

        // Extract duration in background and update
        if is_video {
            let svc = self.clone();
            let vid = id;
            let path = dest_path.clone();
            tokio::spawn(async move {
                if let Ok(Some(dur)) = extract_duration(&path).await {
                    let _ = svc.repo.update_duration(vid, dur).await;
                }
            });
        }

        // Generate thumbnail in background
        let svc = self.clone();
        let vid = id;
        tokio::spawn(async move {
            if let Err(e) = svc.generate_thumbnail(vid).await {
                info!("Thumbnail generation for video {}: {}", vid, e);
            }
        });

        Ok(id)
    }

    pub async fn scan_media_directory(&self, category: &str) -> Result<i64, sqlx::Error> {
        const MAX_SCAN_FILES: usize = 5000;
        let existing_urls = self.repo.find_all_local_file_names().await?;
        let media_root = self.config.media_root.clone();
        let video_exts: HashSet<&str> = ["mp4", "m3u8", "mov", "avi", "mkv", "webm", "flv", "wmv"].into();
        let image_exts: HashSet<&str> = ["jpg", "jpeg", "png", "webp", "gif", "bmp"].into();

        // Offload blocking FS discovery + MD5 hashing to the blocking thread pool.
        // Uses streaming read to avoid loading entire files into memory.
        #[derive(Clone)]
        struct FileCandidate {
            file_name: String,
            stream_url: String,
            source_type: &'static str,
            file_hash: String,
            file_size: i64,
        }

        let candidates: Vec<FileCandidate> = tokio::task::spawn_blocking(move || {
            let mut entries = match std::fs::read_dir(&media_root) {
                Ok(e) => e,
                Err(_) => return vec![],
            };
            let mut out = Vec::new();
            while let Some(entry) = entries.next().and_then(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() { continue; }
                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let stream_url = format!("/media/{}", file_name);
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                let source_type = if video_exts.contains(ext.as_str()) {
                    "local_video"
                } else if image_exts.contains(ext.as_str()) {
                    "local_image"
                } else {
                    continue;
                };
                // Stream-read file to compute MD5 without loading into memory
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let file_size = match file.metadata() {
                    Ok(m) => m.len() as i64,
                    Err(_) => continue,
                };
                let mut hasher = Md5::new();
                let mut buf = [0u8; 65536];
                let mut reader = file;
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => hasher.update(&buf[..n]),
                        Err(_) => break,
                    }
                }
                let file_hash = format!("{:x}", hasher.finalize());
                out.push(FileCandidate { file_name, stream_url, source_type, file_hash, file_size });
            }
            out
        }).await.map_err(|e| {
            tracing::error!("scan directory panicked: {}", e);
            sqlx::Error::Protocol("scan thread panicked".into())
        })?;

        let mut added = 0i64;
        for cand in candidates {
            if added as usize >= MAX_SCAN_FILES {
                info!("Scan limit reached ({} files)", MAX_SCAN_FILES);
                break;
            }
            if existing_urls.contains(&cand.stream_url) { continue; }
            self.repo.save_local_video(
                &cand.file_name, "",
                cand.source_type, None,
                &cand.stream_url, category,
                Some(&cand.file_hash), Some(cand.file_size), Some(&cand.file_name), None,
            ).await?;
            added += 1;
        }
        if added > 0 {
            info!("Scanned and added {} new files", added);
        }
        Ok(added)
    }

    pub async fn delete_video(&self, id: i64) -> Result<bool, String> {
        let video = self.repo.find_by_id(id).await.map_err(|e| e.to_string())?;
        if let Some(v) = video {
            // Delete physical file if it's a local file
            if let Some(file_path) = self.safe_media_path(&v.stream_url) {
                let fp = file_path.clone();
                let deleted = tokio::task::spawn_blocking(move || {
                    if fp.exists() { std::fs::remove_file(&fp).map(|_| true) } else { Ok(false) }
                }).await.map_err(|e| format!("delete thread panicked: {}", e))?
                  .map_err(|e| format!("删除文件失败: {}", e))?;
                if deleted {
                    info!("Deleted media file: {:?}", file_path);
                }
            }
            // Delete cover image if it exists
            if let Some(ref cover_url) = v.cover_url {
                if let Some(cover_path) = self.safe_media_path(cover_url) {
                    let cp = cover_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        if cp.exists() { let _ = std::fs::remove_file(&cp); }
                    }).await;
                }
            }
            // Delete thumbnail if it exists
            if let Some(ref thumb_url) = v.thumb_url {
                if let Some(thumb_path) = self.safe_media_path(thumb_url) {
                    let tp = thumb_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        if tp.exists() { let _ = std::fs::remove_file(&tp); }
                    }).await;
                }
            }
            // Delete playback history, likes, and favorites first, then the video record
            let _ = self.repo.delete_playback_history_by_video(id).await;
            let _ = self.repo.delete_likes_by_video(id).await;
            let _ = self.repo.delete_favorites_by_video(id).await;
            self.repo.delete_video(id).await.map_err(|e| e.to_string())?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn delete_videos(&self, ids: Vec<i64>) -> Result<u64, String> {
        // Load video info to delete physical files
        for id in &ids {
            if let Ok(Some(v)) = self.repo.find_by_id(*id).await {
                // Delete physical files (best-effort, in background)
                for url in [&v.stream_url, v.cover_url.as_deref().unwrap_or(""), v.thumb_url.as_deref().unwrap_or("")] {
                    if let Some(path) = self.safe_media_path(url) {
                        let _ = tokio::task::spawn_blocking(move || {
                            if path.exists() { let _ = std::fs::remove_file(path); }
                        }).await;
                    }
                }
            }
        }
        // Batch delete from DB in a single transaction
        self.repo.batch_delete_videos(&ids).await.map_err(|e| e.to_string())
    }

    pub async fn update_video(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        category: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let existing = self.repo.find_by_id(id).await?;
        if existing.is_none() {
            return Ok(false);
        }
        self.repo.update_video(id, title, description, category).await?;
        Ok(true)
    }

    pub async fn update_cover(&self, id: i64, file_name: &str, bytes: Bytes) -> Result<(), String> {
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "jpg".to_string());

        let cover_file_name = format!("cover_{}_{}.{}", id, chrono::Utc::now().timestamp_millis(), ext);
        let cover_path = self.config.media_root.join(&cover_file_name);

        tokio_fs::write(&cover_path, &bytes)
            .await
            .map_err(|e| format!("写入封面失败: {}", e))?;

        let cover_url = format!("/media/{}", cover_file_name);
        self.repo.update_cover_url(id, &cover_url).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Generate a thumbnail from a video file using ffmpeg
    pub async fn generate_thumbnail(&self, video_id: i64) -> Result<bool, String> {
        let video = self.repo.find_by_id(video_id).await.map_err(|e| e.to_string())?;
        let video = video.ok_or_else(|| "not found".to_string())?;

        // Only generate for local video files
        if !video.source_type.starts_with("local_video") {
            return Ok(false);
        }

        let video_path = match self.safe_media_path(&video.stream_url) {
            Some(p) => p,
            None => return Err(format!("video file not found or invalid path: {}", video.stream_url)),
        };
        let video_path_exists = tokio::task::spawn_blocking({
            let vp = video_path.clone();
            move || vp.exists()
        }).await.unwrap_or(false);
        if !video_path_exists {
            return Err(format!("video file not found: {}", video_path.display()));
        }

        // Extract frame at 1 second using ffmpeg (offloaded to blocking thread pool)
        let cover_file_name = format!("cover_{}.jpg", video_id);
        let cover_path = self.config.media_root.join(&cover_file_name);
        let cover_path_str = cover_path.to_string_lossy().to_string();
        let video_path_clone = video_path.clone();

        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .arg("-y").arg("-ss").arg("1")
                .arg("-i").arg(&video_path_clone)
                .arg("-vframes").arg("1").arg("-q:v").arg("3")
                .arg(&cover_path_str)
                .output()
        }).await
            .map_err(|e| format!("ffmpeg task panicked: {}", e))?
            .map_err(|e| format!("ffmpeg not found: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // ffmpeg might exit with error even on success for some files; check if file was created
            let cover_exists = tokio::task::spawn_blocking({
                let cp = cover_path.clone();
                move || cp.exists()
            }).await.unwrap_or(false);
            if !cover_exists {
                return Err(format!("ffmpeg failed: {}", stderr.lines().next().unwrap_or("unknown error")));
            }
        }

        let cover_url = format!("/media/{}", cover_file_name);
        self.repo.update_cover_url(video_id, &cover_url).await.map_err(|e| e.to_string())?;

        // Generate a smaller thumbnail (320x180) for grid use
        let thumb_file_name = format!("thumb_{}.jpg", video_id);
        let thumb_path = self.config.media_root.join(&thumb_file_name);
        let thumb_path_str = thumb_path.to_string_lossy().to_string();
        let cover_clone = cover_path.clone();
        let thumb_result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .arg("-y").arg("-i").arg(&cover_clone)
                .arg("-vf").arg("scale=320:180").arg("-q:v").arg("5")
                .arg(&thumb_path_str)
                .output()
        }).await;
        if let Ok(Ok(output)) = thumb_result {
            if output.status.success() {
                let thumb_url = format!("/media/{}", thumb_file_name);
                let _ = self.repo.update_thumb_url(video_id, &thumb_url).await;
                info!("Generated thumbnail for video {}: {}", video_id, cover_url);
            }
        }

        Ok(true)
    }

    /// Backfill thumbnails for local videos without a cover (paginated to avoid memory spike)
    pub async fn backfill_thumbnails(&self) -> Result<(i64, Vec<String>), String> {
        let mut generated = 0i64;
        let mut errors = Vec::new();
        let mut last_id: i64 = 0;

        loop {
            let rows = self.repo.find_videos_without_cover(last_id, 100).await
                .map_err(|e| e.to_string())?;
            if rows.is_empty() { break; }

            for row in &rows {
                last_id = row.id;
                match self.generate_thumbnail(row.id).await {
                    Ok(true) => generated += 1,
                    Ok(false) => {}
                    Err(e) => errors.push(format!("id={}: {}", row.id, e)),
                }
            }
        }
        Ok((generated, errors))
    }

    // ── Playback History ──

    pub async fn get_playback_position(&self, username: &str, video_id: i64) -> Result<Option<i64>, sqlx::Error> {
        self.repo.get_playback_position(username, video_id).await
    }

    pub async fn get_playback_duration(&self, username: &str, video_id: i64) -> Result<Option<i64>, sqlx::Error> {
        self.repo.get_playback_duration(username, video_id).await
    }

    pub async fn get_playback_history(&self, username: &str) -> Result<Vec<crate::models::playback::RecentWatchItem>, sqlx::Error> {
        self.repo.find_playback_history_by_username(username).await
    }

    pub async fn update_playback(&self, username: &str, video_id: i64, position_ms: i64, duration_ms: i64) -> Result<(), sqlx::Error> {
        self.repo.upsert_playback(username, video_id, position_ms, duration_ms).await
    }

    pub async fn get_user_profile_data(&self, username: &str) -> Result<(i64, i64, Vec<crate::models::playback::RecentWatchItem>), sqlx::Error> {
        let total_videos_watched = self.repo.count_watched_videos(username).await?;
        let total_watch_time = self.repo.sum_watch_time(username).await?;
        let recent_history = self.repo.find_recent_history_with_details(username, 20).await?;
        Ok((total_videos_watched, total_watch_time, recent_history))
    }

    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.toggle_like(username, video_id).await
    }

    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_liked(username, video_id).await
    }

    pub async fn toggle_favorite(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.toggle_favorite(username, video_id).await
    }

    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_favorited(username, video_id).await
    }

    pub async fn increment_views(&self, id: i64) -> Result<(), sqlx::Error> {
        self.repo.increment_views(id).await
    }
}

/// Resolve a `/media/...` URL to a safe filesystem path inside media_root.
/// Returns None if the path would escape media_root (path traversal blocked).
/// Extracted as a standalone function for testability.
pub fn safe_media_path(url: &str, media_root: &Path) -> Option<PathBuf> {
    let relative = url.strip_prefix("/media/")?;
    let path = media_root.join(relative);
    // Canonicalize to resolve any ".." components, then verify prefix
    let canonical = path.canonicalize().ok()?;
    if canonical.starts_with(media_root) {
        Some(canonical)
    } else {
        tracing::warn!("Path traversal blocked: {:?}", path);
        None
    }
}

async fn extract_duration(path: &std::path::Path) -> Result<Option<i64>, String> {
    let path_str = path.to_string_lossy().to_string();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("ffprobe")
            .arg("-v").arg("error")
            .arg("-show_entries").arg("format=duration")
            .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
            .arg(&path_str)
            .output()
    }).await
        .map_err(|e| format!("ffprobe task panicked: {}", e))?
        .map_err(|e| format!("ffprobe not found: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let dur_secs = stdout.trim().parse::<f64>().map_err(|e| format!("parse duration: {}", e))?;
    Ok(Some((dur_secs * 1000.0) as i64))
}

/// 用 magic bytes 验证文件类型，防止伪装扩展名的恶意上传
pub fn validate_file_type(path: &std::path::Path, ext: &str) -> Result<(), String> {
    // m3u8 (HLS playlist) is plain text with no fixed magic bytes — always allow
    if ext == "m3u8" {
        return Ok(());
    }

    let kind = infer::get_from_path(path)
        .map_err(|e| format!("无法读取文件类型: {}", e))?
        .ok_or_else(|| format!("无法识别的文件类型: {}", ext))?;

    let mime_type = kind.mime_type();
    let is_valid = match ext {
        "mp4" | "m4v" => mime_type.starts_with("video/mp4"),
        "mov" => mime_type == "video/quicktime",
        "avi" => mime_type == "video/x-msvideo",
        "mkv" => mime_type == "video/x-matroska",
        "webm" => mime_type == "video/webm",
        "flv" => mime_type == "video/x-flv",
        "wmv" => mime_type == "video/x-ms-wmv",
        "jpg" | "jpeg" => mime_type.starts_with("image/jpeg"),
        "png" => mime_type == "image/png",
        "webp" => mime_type.starts_with("image/webp"),
        "gif" => mime_type == "image/gif",
        "bmp" => mime_type == "image/bmp",
        _ => false,
    };

    if !is_valid {
        return Err(format!(
            "文件类型不匹配: 扩展名 .{} 但实际 MIME 类型为 {}",
            ext, mime_type
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension_detection() {
        let video_exts: HashSet<&str> = ["mp4", "m3u8", "mov", "avi", "mkv", "webm", "flv", "wmv"].into();
        let image_exts: HashSet<&str> = ["jpg", "jpeg", "png", "webp", "gif", "bmp"].into();

        assert!(video_exts.contains("mp4"));
        assert!(video_exts.contains("mkv"));
        assert!(!video_exts.contains("jpg"));
        assert!(image_exts.contains("jpg"));
        assert!(!image_exts.contains("mp4"));
    }

    #[test]
    fn test_validate_file_type_png() {
        // Minimal valid PNG: 8-byte signature + IHDR chunk
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // chunk length
            0x49, 0x48, 0x44, 0x52, // "IHDR"
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 pixel
            0x08, 0x02, 0x00, 0x00, 0x00, // bit depth + color type + compression etc
            0x90, 0x77, 0x53, 0xDE, // CRC
        ];
        let dir = std::env::temp_dir();
        let path = dir.join("test_valid_png.png");
        std::fs::write(&path, &png_bytes).unwrap();
        assert!(validate_file_type(&path, "png").is_ok());
        assert!(validate_file_type(&path, "jpg").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_file_type_jpeg() {
        // Minimal valid JPEG: SOI + EOI markers
        let jpeg_bytes: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9];
        let dir = std::env::temp_dir();
        let path = dir.join("test_valid_jpeg.jpg");
        std::fs::write(&path, &jpeg_bytes).unwrap();
        assert!(validate_file_type(&path, "jpg").is_ok());
        assert!(validate_file_type(&path, "png").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_file_type_rejects_text_as_mp4() {
        // Plain text file pretending to be mp4
        let dir = std::env::temp_dir();
        let path = dir.join("fake.mp4");
        std::fs::write(&path, b"this is not a video file").unwrap();
        assert!(validate_file_type(&path, "mp4").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_file_type_unknown_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("file.xyz");
        std::fs::write(&path, b"some content").unwrap();
        assert!(validate_file_type(&path, "xyz").is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
