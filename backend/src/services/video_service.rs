use std::path::Path;
use std::collections::HashSet;
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
        username: Option<&str>,
    ) -> Result<(Vec<VideoItem>, i64), sqlx::Error> {
        let total = self.repo.count_all(query, source_type).await?;
        let rows = self.repo.find_all_paged(page, size, query, source_type, username).await?;
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
        let mut existing = HashSet::new();
        for (i, file) in files.iter().enumerate() {
            if self.repo.find_existing_by_name_and_size(&file.name, file.size).await? {
                existing.insert(i);
            }
        }
        Ok(existing)
    }

    pub async fn upload_video(
        &self,
        file_name: &str,
        bytes: Bytes,
        category: &str,
        client_hash: Option<&str>,
    ) -> Result<i64, String> {
        // Compute MD5 hash
        let hash = compute_md5(&bytes);

        // Check for duplicates if client provided hash, or always check
        if let Some(ch) = client_hash {
            if ch == &hash {
                if let Some(existing) = self.repo.find_video_by_file_hash(&hash).await.map_err(|e| e.to_string())? {
                    return Err(format!("duplicate: video already exists with id={}", existing.id));
                }
            }
        } else {
            // Check if hash already exists
            if let Some(existing) = self.repo.find_video_by_file_hash(&hash).await.map_err(|e| e.to_string())? {
                return Err(format!("duplicate: video already exists with id={}", existing.id));
            }
        }

        // Determine file extension
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "mp4".to_string());

        let is_video = matches!(ext.as_str(), "mp4" | "m3u8" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv");
        let is_image = matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp");

        let source_type = if is_video { "local_video" } else if is_image { "local_image" } else { "local" };

        // Save file
        let dest_file_name = format!("{}_{}", chrono::Utc::now().timestamp_millis(), file_name);
        let dest_path = self.config.media_root.join(&dest_file_name);

        // Write file
        tokio_fs::write(&dest_path, &bytes)
            .await
            .map_err(|e| format!("failed to write file: {}", e))?;

        let stream_url = format!("/media/{}", dest_file_name);
        let file_size = bytes.len() as i64;

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

        info!("Uploaded video id={} as {}", id, stream_url);

        // Generate thumbnail in background (offloaded to blocking thread pool)
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
        let existing_urls = self.repo.find_all_local_file_names().await?;
        let media_root = self.config.media_root.clone();
        let video_exts: HashSet<&str> = ["mp4", "m3u8", "mov", "avi", "mkv", "webm", "flv", "wmv"].into();
        let image_exts: HashSet<&str> = ["jpg", "jpeg", "png", "webp", "gif", "bmp"].into();

        // Offload blocking FS discovery to the blocking thread pool
        #[derive(Clone)]
        struct FileCandidate {
            file_name: String,
            stream_url: String,
            source_type: &'static str,
            file_bytes: Vec<u8>,
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
                let file_bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                out.push(FileCandidate { file_name, stream_url, source_type, file_bytes });
            }
            out
        }).await.map_err(|e| {
            tracing::error!("scan directory panicked: {}", e);
            sqlx::Error::Protocol("scan thread panicked".into())
        })?;

        let mut added = 0i64;
        for cand in candidates {
            if existing_urls.contains(&cand.stream_url) { continue; }
            let hash = compute_md5(&cand.file_bytes);
            let file_size = cand.file_bytes.len() as i64;
            self.repo.save_local_video(
                &cand.file_name, "",
                cand.source_type, None,
                &cand.stream_url, category,
                Some(&hash), Some(file_size), Some(&cand.file_name), None,
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
            if v.stream_url.starts_with("/media/") {
                let file_name = v.stream_url.trim_start_matches("/media/");
                let file_path = self.config.media_root.join(file_name);
                let fp = file_path.clone();
                let deleted = tokio::task::spawn_blocking(move || {
                    if fp.exists() { std::fs::remove_file(&fp).map(|_| true) } else { Ok(false) }
                }).await.map_err(|e| format!("delete thread panicked: {}", e))?
                  .map_err(|e| format!("failed to delete file: {}", e))?;
                if deleted {
                    info!("Deleted media file: {:?}", file_path);
                }
            }
            // Delete cover image if it exists
            if let Some(ref cover_url) = v.cover_url {
                if cover_url.starts_with("/media/") {
                    let cover_name = cover_url.trim_start_matches("/media/");
                    let cover_path = self.config.media_root.join(cover_name);
                    let cp = cover_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        if cp.exists() { let _ = std::fs::remove_file(&cp); }
                    }).await;
                }
            }
            // Delete playback history first, then the video record
            let _ = self.repo.delete_playback_history_by_video(id).await;
            self.repo.delete_video(id).await.map_err(|e| e.to_string())?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn delete_videos(&self, ids: Vec<i64>) -> Result<u64, String> {
        let mut deleted = 0u64;
        for id in &ids {
            if self.delete_video(*id).await? {
                deleted += 1;
            }
        }
        Ok(deleted)
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
            .map_err(|e| format!("failed to write cover: {}", e))?;

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

        let file_name = video.stream_url.trim_start_matches("/media/");
        let video_path = self.config.media_root.join(file_name);
        let video_path_exists = tokio::task::spawn_blocking({
            let vp = video_path.clone();
            move || vp.exists()
        }).await.unwrap_or(false);
        if !video_path_exists {
            return Err(format!("video file not found: {}", video_path.display()));
        }

        // Extract frame at 5 seconds using ffmpeg (offloaded to blocking thread pool)
        let cover_file_name = format!("cover_{}.jpg", video_id);
        let cover_path = self.config.media_root.join(&cover_file_name);
        let video_path_str = video_path.to_string_lossy().to_string();
        let cover_path_str = cover_path.to_string_lossy().to_string();

        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .args(["-y", "-ss", "5", "-i", &video_path_str,
                       "-vframes", "1", "-q:v", "3", &cover_path_str])
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
        let cover_path_str2 = cover_path.to_string_lossy().to_string();
        let thumb_result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .args(["-y", "-i", &cover_path_str2, "-vf", "scale=320:180", "-q:v", "5", &thumb_path_str])
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

    /// Backfill thumbnails for all local videos without a cover
    pub async fn backfill_thumbnails(&self) -> Result<(i64, Vec<String>), String> {
        let rows = self.repo.find_all(None).await.map_err(|e| e.to_string())?;
        let mut generated = 0i64;
        let mut errors = Vec::new();

        for row in &rows {
            if row.cover_url.is_some() {
                continue; // skip videos with existing covers
            }
            match self.generate_thumbnail(row.id).await {
                Ok(true) => generated += 1,
                Ok(false) => {}
                Err(e) => errors.push(format!("id={}: {}", row.id, e)),
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

    pub async fn get_playback_history(&self, username: &str) -> Result<Vec<crate::models::playback::PlaybackHistoryResponse>, sqlx::Error> {
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
}

fn compute_md5(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
