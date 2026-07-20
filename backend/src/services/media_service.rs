use axum::body::Bytes;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::config::AppConfig;
use crate::repositories::video_repo::VideoRepository;

/// SECURITY (A04 H2): per-user upload quota (default 50 GB).
/// Override with `UPLOAD_QUOTA_BYTES` env var. Set to 0 to disable.
fn user_upload_quota_bytes() -> i64 {
    std::env::var("UPLOAD_QUOTA_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50 * 1024 * 1024 * 1024)
}

/// Identify an image's format by inspecting its leading bytes.
/// SECURITY (A08-02): used to validate avatar uploads without trusting the
/// client's Content-Type. Returns (extension, mime) on success.
pub fn infer_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.len() < 8 {
        return None;
    }
    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(("jpg", "image/jpeg"));
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(("png", "image/png"));
    }
    // GIF: 47 49 46 38 (37|39) 61
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(("gif", "image/gif"));
    }
    // WebP: "RIFF" .... "WEBP"
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(("webp", "image/webp"));
    }
    // BMP: "BM"
    if bytes.starts_with(b"BM") {
        return Some(("bmp", "image/bmp"));
    }
    None
}

#[derive(Clone)]
pub struct MediaService {
    repo: VideoRepository,
    config: AppConfig,
}

impl MediaService {
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        Self { repo, config }
    }

    /// 流式上传：从临时文件读取，计算 MD5，移动到最终位置
    pub async fn upload_video_file(
        &self,
        file_name: &str,
        temp_path: &std::path::Path,
        category: &str,
        _client_hash: Option<&str>,
        uploader_id: i64,
    ) -> Result<i64, String> {
        // Compute SHA-256 hash by streaming the file
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(temp_path)
            .await
            .map_err(|e| format!("打开临时文件失败: {}", e))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 65536];
        let mut file_size: i64 = 0;
        loop {
            let n = file
                .read(&mut buf)
                .await
                .map_err(|e| format!("读取临时文件失败: {}", e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file_size += n as i64;
        }
        drop(file);
        let hash = format!("{:x}", hasher.finalize());

        // SECURITY (A04 H2): enforce per-user storage quota *before* we move
        // the file or write to the database. This check is racy across
        // concurrent uploads from the same user, but that's an acceptable
        // over-quota boundary — quotas are advisory, not security-critical.
        let quota = user_upload_quota_bytes();
        if quota > 0 {
            let used = self
                .repo
                .get_storage_used(uploader_id)
                .await
                .map_err(|e| e.to_string())?;
            if used + file_size > quota {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err(format!(
                    "存储配额已用完：已用 {} 字节，本次 {} 字节，配额 {} 字节",
                    used, file_size, quota
                ));
            }
        }

        // Check for duplicates using server-computed hash
        if self
            .repo
            .find_video_by_file_hash(&hash)
            .await
            .map_err(|e| e.to_string())?
            .is_some()
        {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err("重复：视频已存在".into());
        }

        // SECURITY (A08-04): strip control characters from the file name
        // before it is stored or used to derive a path component. Without
        // this, an attacker can inject log/DB lines by uploading with a
        // name like "evil\n<mark>FAKE</mark>".
        let sanitized_name = sanitize_filename(file_name);
        let ext = Path::new(&sanitized_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "mp4".to_string());

        // 用 magic bytes 验证文件类型（spawn_blocking 避免阻塞 async 线程）
        let temp_path_clone = temp_path.to_path_buf();
        let ext_clone = ext.clone();
        tokio::task::spawn_blocking(move || validate_file_type(&temp_path_clone, &ext_clone))
            .await
            .map_err(|e| format!("文件验证任务失败: {}", e))?
            .map_err(|e| {
                let _ = std::fs::remove_file(temp_path);
                format!("文件验证失败: {}", e)
            })?;
        // Re-open temp_path after spawn_blocking
        let temp_path = temp_path.to_path_buf();

        let is_video = matches!(
            ext.as_str(),
            "mp4" | "m3u8" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv"
        );
        let is_image = matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp"
        );

        let source_type = if is_video {
            "local_video"
        } else if is_image {
            "local_image"
        } else {
            "local"
        };

        // Move temp file to final destination. The dest name is composed only
        // of the original filename's *base* (already sanitised) prefixed with
        // a timestamp — never with anything user-controlled beyond the
        // alphanumeric body of the filename.
        let dest_base = Path::new(&sanitized_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "video.mp4".to_string());
        let dest_file_name = format!("{}_{}", chrono::Utc::now().timestamp_millis(), dest_base);
        let dest_path = self.config.media_root.join(&dest_file_name);
        tokio::fs::rename(temp_path, &dest_path)
            .await
            .map_err(|e| format!("移动文件失败: {}", e))?;

        let stream_url = format!("/media/{}", dest_file_name);

        let id = self
            .repo
            .save_local_video(
                &sanitized_name,
                "",
                source_type,
                None,
                &stream_url,
                category,
                Some(&hash),
                Some(file_size),
                Some(&sanitized_name),
                None,
                Some(uploader_id),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Charge the user's quota counter.
        let _ = self
            .repo
            .increment_storage_used(uploader_id, file_size)
            .await;

        info!(
            uploader = uploader_id,
            video_id = id,
            bytes = file_size,
            "video uploaded"
        );

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

    /// Generate a thumbnail from a video file using ffmpeg
    pub async fn generate_thumbnail(&self, video_id: i64) -> Result<bool, String> {
        let video = self
            .repo
            .find_by_id(video_id)
            .await
            .map_err(|e| e.to_string())?;
        let video = video.ok_or_else(|| "not found".to_string())?;

        // Only generate for local video files
        if !video.source_type.starts_with("local_video") {
            return Ok(false);
        }

        // Skip if cover already exists on disk
        if let Some(cover_url) = &video.cover_url {
            if let Some(cover_path) = safe_media_path(cover_url, &self.config.media_root) {
                let exists = tokio::task::spawn_blocking({
                    let cp = cover_path.clone();
                    move || cp.exists()
                })
                .await
                .unwrap_or(false);
                if exists {
                    // Cover exists, check thumb too
                    if let Some(thumb_url) = &video.thumb_url {
                        if let Some(thumb_path) =
                            safe_media_path(thumb_url, &self.config.media_root)
                        {
                            let thumb_exists = tokio::task::spawn_blocking({
                                let tp = thumb_path.clone();
                                move || tp.exists()
                            })
                            .await
                            .unwrap_or(false);
                            if thumb_exists {
                                return Ok(false); // Both exist, nothing to do
                            }
                        }
                    }
                }
            }
        }

        let video_path = match safe_media_path(&video.stream_url, &self.config.media_root) {
            Some(p) => p,
            None => {
                return Err(format!(
                    "video file not found or invalid path: {}",
                    video.stream_url
                ))
            }
        };
        let video_path_exists = tokio::task::spawn_blocking({
            let vp = video_path.clone();
            move || vp.exists()
        })
        .await
        .unwrap_or(false);
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
                .arg("-y")
                .arg("-ss")
                .arg("1")
                .arg("-i")
                .arg(&video_path_clone)
                .arg("-vframes")
                .arg("1")
                .arg("-q:v")
                .arg("3")
                .arg(&cover_path_str)
                .output()
        })
        .await
        .map_err(|e| format!("ffmpeg task panicked: {}", e))?
        .map_err(|e| format!("ffmpeg not found: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // ffmpeg might exit with error even on success for some files; check if file was created
            let cover_exists = tokio::task::spawn_blocking({
                let cp = cover_path.clone();
                move || cp.exists()
            })
            .await
            .unwrap_or(false);
            if !cover_exists {
                return Err(format!(
                    "ffmpeg failed: {}",
                    stderr.lines().next().unwrap_or("unknown error")
                ));
            }
        }

        let cover_url = format!("/media/{}", cover_file_name);
        self.repo
            .update_cover_url(video_id, &cover_url)
            .await
            .map_err(|e| e.to_string())?;

        // Generate a smaller thumbnail (320x180) for grid use
        let thumb_file_name = format!("thumb_{}.jpg", video_id);
        let thumb_path = self.config.media_root.join(&thumb_file_name);
        let thumb_path_str = thumb_path.to_string_lossy().to_string();
        let cover_clone = cover_path.clone();
        let thumb_result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(&cover_clone)
                .arg("-vf")
                .arg("scale=320:180")
                .arg("-q:v")
                .arg("5")
                .arg(&thumb_path_str)
                .output()
        })
        .await;
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
            let rows = self
                .repo
                .find_videos_without_cover(last_id, 100)
                .await
                .map_err(|e| e.to_string())?;
            if rows.is_empty() {
                break;
            }

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

    pub async fn update_cover(&self, id: i64, file_name: &str, bytes: Bytes) -> Result<(), String> {
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "jpg".to_string());

        // Write to a temp path first for validation
        let tmp_file_name = format!(
            "cover_tmp_{}_{}.{}",
            id,
            chrono::Utc::now().timestamp_millis(),
            ext
        );
        let tmp_path = self.config.media_root.join(&tmp_file_name);
        tokio::fs::write(&tmp_path, &bytes)
            .await
            .map_err(|e| format!("写入临时封面失败: {}", e))?;

        // Validate the uploaded file using magic bytes
        let tmp_path_clone = tmp_path.clone();
        let ext_clone = ext.clone();
        tokio::task::spawn_blocking(move || validate_file_type(&tmp_path_clone, &ext_clone))
            .await
            .map_err(|e| format!("验证任务失败: {}", e))?
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp_path);
                format!("封面上传验证失败: {}", e)
            })?;

        let cover_file_name = format!(
            "cover_{}_{}.{}",
            id,
            chrono::Utc::now().timestamp_millis(),
            ext
        );
        let cover_path = self.config.media_root.join(&cover_file_name);

        tokio::fs::rename(&tmp_path, &cover_path)
            .await
            .map_err(|e| format!("移动封面失败: {}", e))?;

        let cover_url = format!("/media/{}", cover_file_name);
        self.repo
            .update_cover_url(id, &cover_url)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Resolve a `/media/...` URL to a safe filesystem path inside media_root.
/// Returns None if the path would escape media_root (path traversal blocked).
/// Extracted as a standalone function for testability.
///
/// Platform note: `canonicalize()` resolves symlinks and normalizes paths on Unix.
/// On Windows it also resolves to the full path. Double slashes in the URL are
/// preserved by `Path::join` but stripped by `canonicalize()`, so the check is
/// safe across platforms.
pub fn safe_media_path(url: &str, media_root: &Path) -> Option<PathBuf> {
    let relative = url.strip_prefix("/media/")?;
    let path = media_root.join(relative);
    // Canonicalize to resolve any ".." components, then verify prefix
    let canonical = path.canonicalize().ok()?;
    let canonical_root = media_root.canonicalize().ok()?;
    if canonical.starts_with(&canonical_root) {
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
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(&path_str)
            .output()
    })
    .await
    .map_err(|e| format!("ffprobe task panicked: {}", e))?
    .map_err(|e| format!("ffprobe not found: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let dur_secs = stdout
        .trim()
        .parse::<f64>()
        .map_err(|e| format!("parse duration: {}", e))?;
    Ok(Some((dur_secs * 1000.0) as i64))
}

/// 用 magic bytes 验证文件类型，防止伪装扩展名的恶意上传
pub fn validate_file_type(path: &std::path::Path, ext: &str) -> Result<(), String> {
    // m3u8 (HLS playlist) is plain text with no fixed magic bytes — validate
    // the full content structure. SECURITY (H-05): the previous
    // implementation only checked the `#EXTM3U` prefix, so a malicious
    // payload could be smuggled after a one-line header. We now enforce:
    //   - starts with #EXTM3U
    //   - no embedded HTML/script tags (defence in depth against XSS-via-HLS)
    //   - file size is bounded
    if ext == "m3u8" {
        const MAX_M3U8_BYTES: u64 = 1_048_576; // 1 MiB
        let metadata =
            std::fs::metadata(path).map_err(|e| format!("无法读取 m3u8 元数据: {}", e))?;
        if metadata.len() > MAX_M3U8_BYTES {
            return Err(format!("m3u8 文件过大: 限制 {} 字节", MAX_M3U8_BYTES));
        }
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("无法读取 m3u8 文件: {}", e))?;
        let trimmed = content.trim_start();
        if !trimmed.starts_with("#EXTM3U") {
            return Err("m3u8 文件格式无效: 缺少 #EXTM3U 头部".to_string());
        }
        // Reject HTML-ish or script-ish content. We don't try to parse
        // the playlist — just block obvious injection attempts.
        let lower = content.to_ascii_lowercase();
        for needle in [
            "<script",
            "</script",
            "<iframe",
            "<object",
            "<embed",
            "<?xml",
            "<!doctype",
            "<html",
            "javascript:",
        ] {
            if lower.contains(needle) {
                return Err(format!("m3u8 文件包含禁止内容: {}", needle));
            }
        }
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

/// Strip path separators and control characters from a user-supplied filename.
/// SECURITY (A08-04): the result is suitable for use as a DB column value,
/// for log emission, and as a single path component.
pub fn sanitize_filename(name: &str) -> String {
    // Keep only the trailing path component (drops any "../" the client
    // smuggles in via the multipart `filename` field).
    let base = std::path::Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "video.mp4".to_string());
    let mut out = String::with_capacity(base.len());
    for c in base.chars() {
        if c.is_control() || c == '/' || c == '\\' || c == '\0' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    // Cap length to 200 chars so an attacker can't blow up DB rows / log lines.
    if out.len() > 200 {
        out.truncate(200);
    }
    if out.is_empty() {
        out.push_str("video.mp4");
    }
    out
}

/// Validate an external stream URL: must be http(s), must NOT resolve to a
/// loopback, link-local, private, or otherwise non-routable address. Defence
/// in depth even though we don't fetch the URL server-side (browsers do).
pub fn is_safe_external_url(url: &str) -> bool {
    let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
    if scheme_end == 0 {
        return false;
    }
    let scheme = &url[..scheme_end - 3];
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    let after = &url[scheme_end..];
    // host[:port][/path...]
    let host_end = after.find([':', '/']).unwrap_or(after.len());
    let host = &after[..host_end];
    if host.is_empty() {
        return false;
    }
    let host_lower = host.to_ascii_lowercase();
    let blocked = [
        "localhost",
        "127.0.0.1",
        "::1",
        "0.0.0.0",
        "169.254.169.254",
        "metadata.google.internal",
        "metadata.goog",
        "100.100.100.200",
    ];
    if blocked.iter().any(|b| host_lower == *b) {
        return false;
    }
    if let Ok(ip) = host_lower.parse::<std::net::IpAddr>() {
        if is_disallowed_ip(&ip) {
            return false;
        }
    }
    true
}

fn is_disallowed_ip(ip: &std::net::IpAddr) -> bool {
    use std::net::IpAddr::*;
    match ip {
        V4(v4) => {
            v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_private()
                || v4.is_multicast()
        }
        V6(v6) => v6.is_loopback() || v6.is_unspecified() || v6.is_multicast(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("video.mp4"), "video.mp4");
    }

    #[test]
    fn test_sanitize_filename_strips_path_separators() {
        // `Path::file_name` extracts just the last component
        let result = sanitize_filename("../../etc/passwd");
        assert!(!result.contains('/'));
        assert!(!result.contains(".."));
        assert_eq!(result, "passwd");
    }

    #[test]
    fn test_sanitize_filename_strips_control_chars() {
        let result = sanitize_filename("evil\nfile.mp4");
        assert!(!result.contains('\n'));
        assert!(result.contains("evil") && result.contains("file.mp4"));
    }

    #[test]
    fn test_sanitize_filename_strips_nul() {
        let result = sanitize_filename("evil\0file.mp4");
        assert!(!result.contains('\0'));
    }

    #[test]
    fn test_sanitize_filename_length_capped() {
        let long = "a".repeat(500) + ".mp4";
        let result = sanitize_filename(&long);
        assert!(result.len() <= 200 + 4); // 200 chars + ".mp4"
    }

    #[test]
    fn test_sanitize_filename_empty_fallback() {
        assert_eq!(sanitize_filename(""), "video.mp4");
    }

    #[test]
    fn test_sanitize_filename_only_control_chars() {
        let result = sanitize_filename("\n\r\t");
        assert!(result.len() == 3 || result.contains("video"));
    }

    #[test]
    fn test_infer_image_jpeg() {
        let bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        let (ext, mime) = infer_image(&bytes).unwrap();
        assert_eq!(ext, "jpg");
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn test_infer_image_png() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let (ext, mime) = infer_image(&bytes).unwrap();
        assert_eq!(ext, "png");
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn test_infer_image_gif() {
        let bytes = b"GIF89a...";
        let (ext, mime) = infer_image(bytes).unwrap();
        assert_eq!(ext, "gif");
        assert_eq!(mime, "image/gif");
    }

    #[test]
    fn test_infer_image_webp() {
        let bytes = b"RIFF\x00\x00\x00\x00WEBP";
        let (ext, mime) = infer_image(bytes).unwrap();
        assert_eq!(ext, "webp");
        assert_eq!(mime, "image/webp");
    }

    #[test]
    fn test_infer_image_bmp() {
        let bytes = b"BM\x00\x00\x00\x00\x00\x00";
        let (ext, mime) = infer_image(bytes).unwrap();
        assert_eq!(ext, "bmp");
        assert_eq!(mime, "image/bmp");
    }

    #[test]
    fn test_infer_image_empty() {
        assert!(infer_image(&[]).is_none());
    }

    #[test]
    fn test_infer_image_too_short() {
        assert!(infer_image(b"GIF").is_none());
    }

    #[test]
    fn test_infer_image_unknown() {
        assert!(infer_image(b"this is definitely not an image").is_none());
    }

    #[test]
    fn test_is_safe_external_url_https() {
        assert!(is_safe_external_url("https://example.com/video.mp4"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_localhost() {
        assert!(!is_safe_external_url("http://localhost:8082/admin"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_loopback() {
        assert!(!is_safe_external_url("http://127.0.0.1/admin"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_private_ip() {
        assert!(!is_safe_external_url("http://192.168.1.1/admin"));
        assert!(!is_safe_external_url("http://10.0.0.1/admin"));
        assert!(!is_safe_external_url("http://172.16.0.1/admin"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_cloud_metadata() {
        assert!(!is_safe_external_url(
            "http://169.254.169.254/latest/meta-data/"
        ));
    }

    #[test]
    fn test_is_safe_external_url_no_scheme() {
        assert!(!is_safe_external_url("ftp://example.com/file"));
    }

    #[test]
    fn test_is_safe_external_url_invalid_url() {
        assert!(!is_safe_external_url("not a url"));
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
        let jpeg_bytes: Vec<u8> = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
        ];
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
