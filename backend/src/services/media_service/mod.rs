use axum::body::Bytes;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::repositories::video_repo::VideoRepository;
use crate::util::error::ServiceError;

mod validate;
pub mod sweeper;
pub mod upload;

use validate::{extract_duration, sweep_upload_temps_blocking};
pub use validate::{
    infer_image, is_safe_external_url, safe_media_path, sanitize_filename, validate_category,
    validate_file_type, MAX_CATEGORY_CHARS, UPLOAD_TEMP_TTL,
};

/// Hard timeout for a single thumbnail/cover ffmpeg invocation. Seeking to a
/// frame of a local file is fast; a hang this long means ffmpeg is stuck and
/// the child must be killed.
const THUMBNAIL_FFMPEG_TIMEOUT_SECS: u64 = 60;

/// 上传临时文件清扫任务的执行间隔。
pub const UPLOAD_TEMP_SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Cap on the number of concurrent thumbnail-generation ffmpeg processes.
/// Each one is CPU-heavy, so unlimited parallelism would saturate the host.
static THUMBNAIL_SEMAPHORE: OnceLock<tokio::sync::Semaphore> = OnceLock::new();

fn thumbnail_semaphore() -> &'static tokio::sync::Semaphore {
    THUMBNAIL_SEMAPHORE.get_or_init(|| tokio::sync::Semaphore::new(2))
}

/// SECURITY (A04 H2): per-user upload quota (default 50 GB).
/// Override with `UPLOAD_QUOTA_BYTES` env var. Set to 0 to disable.
fn user_upload_quota_bytes() -> i64 {
    std::env::var("UPLOAD_QUOTA_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50 * 1024 * 1024 * 1024)
}

#[derive(Clone)]
pub struct MediaService {
    repo: VideoRepository,
    config: AppConfig,
}

impl MediaService {
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        let svc = Self { repo, config };
        svc.start_upload_temp_sweeper();
        svc
    }

    /// 启动周期性的上传临时文件清扫任务（SECURITY L-05/L-07）：首次 tick
    /// 立即执行一次启动清理，之后每小时一次。进程内只启动一个任务
    /// （OnceLock 保证幂等）；无 tokio 运行时（如同步测试环境）时静默跳过。
    fn start_upload_temp_sweeper(&self) {
        static SWEEPER_STARTED: OnceLock<()> = OnceLock::new();
        if SWEEPER_STARTED.get().is_some() {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let svc = self.clone();
        SWEEPER_STARTED.get_or_init(|| {
            std::mem::drop(handle.spawn(async move {
                tracing::info!(
                    interval_secs = UPLOAD_TEMP_SWEEP_INTERVAL.as_secs(),
                    ttl_secs = UPLOAD_TEMP_TTL.as_secs(),
                    "upload temp sweeper started"
                );
                let mut interval = tokio::time::interval(UPLOAD_TEMP_SWEEP_INTERVAL);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    interval.tick().await;
                    match svc.sweep_stale_upload_temps().await {
                        Ok(0) => tracing::debug!("upload temp sweep: nothing to clean"),
                        Ok(n) => {
                            tracing::info!(removed = n, "upload temp sweep removed stale files")
                        }
                        Err(e) => tracing::warn!("upload temp sweep failed: {}", e),
                    }
                }
            }));
        });
    }

    /// 清理 media_root 中残留的临时文件：
    /// - `.upload_*`：放弃/中断/崩溃的上传（multipart 与续传两种路径的兜底清理）
    /// - `cover_tmp_*`：封面上传崩溃残留
    ///
    /// 只删除 mtime 超过 [`UPLOAD_TEMP_TTL`] 的文件；进行中的上传每次写入都会
    /// 刷新 mtime，因此不会误删。幂等，可重复执行。返回删除的文件数。
    pub async fn sweep_stale_upload_temps(&self) -> Result<usize, ServiceError> {
        let root = self.config.media_root.clone();
        tokio::task::spawn_blocking(move || sweep_upload_temps_blocking(&root, UPLOAD_TEMP_TTL))
            .await
            .map_err(|e| ServiceError::Internal(format!("临时文件清理任务失败: {}", e)))?
            .map_err(|e| ServiceError::Internal(e))
    }

    /// 流式上传：从临时文件读取，计算 SHA-256，移动到最终位置
    pub async fn upload_video_file(
        &self,
        file_name: &str,
        temp_path: &std::path::Path,
        category: &str,
        uploader_id: i64,
    ) -> Result<i64, ServiceError> {
        // SECURITY (L-03): 上传入口（multipart 字段 / x-upload-category 头）此前
        // 无 category 校验。在读取文件前快速失败——50GB 文件不应为错误分类
        // 浪费一次全量读取；临时文件由调用方与下方错误路径共同清理。
        if let Err(e) = validate_category(category) {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(ServiceError::BadRequest(e));
        }

        // Compute SHA-256 hash by streaming the file
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(temp_path)
            .await
            .map_err(|e| ServiceError::Internal(format!("打开临时文件失败: {}", e)))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 65536];
        let mut file_size: i64 = 0;
        loop {
            let n = file
                .read(&mut buf)
                .await
                .map_err(|e| ServiceError::Internal(format!("读取临时文件失败: {}", e)))?;
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
            let used = match self.repo.get_storage_used(uploader_id).await {
                Ok(u) => u,
                Err(e) => {
                    // DB failure: clean up the temp file before propagating.
                    let _ = tokio::fs::remove_file(temp_path).await;
                    return Err(ServiceError::Internal(e.to_string()));
                }
            };
            if used + file_size > quota {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err(ServiceError::QuotaExceeded("存储配额已用尽".into()));
            }
        }

        // Check for duplicates using server-computed hash
        match self.repo.find_video_by_file_hash(&hash).await {
            Ok(Some(_)) => {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err(ServiceError::Duplicate("文件已存在".into()));
            }
            Ok(None) => {}
            Err(e) => {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err(ServiceError::Internal(e.to_string()));
            }
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
        let validation =
            tokio::task::spawn_blocking(move || validate_file_type(&temp_path_clone, &ext_clone))
                .await
                .map_err(|e| ServiceError::Internal(format!("文件验证任务失败: {}", e)))?;
        if let Err(e) = validation {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(ServiceError::BadRequest(format!("文件验证失败: {}", e)));
        }
        // Re-open temp_path after spawn_blocking
        let temp_path = temp_path.to_path_buf();

        let is_video = matches!(
            ext.as_str(),
            "mp4" | "m4v" | "m3u8" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv"
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

        // 持久化：rename 前对文件内容 fsync。flush()（handler 侧）只把数据
        // 交到内核页缓存，宕机时内容可能残缺；先 sync_all 再原子 rename，
        // 保证崩溃后磁盘上要么是完整文件，要么仍是临时文件（由清扫任务兜底），
        // 不存在"已重命名为正式名但内容残缺"的中间态。
        let sync_file = match tokio::fs::OpenOptions::new()
            .write(true)
            .open(&temp_path)
            .await
        {
            Ok(f) => f,
            Err(e) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(ServiceError::Internal(format!("打开临时文件失败: {}", e)));
            }
        };
        if let Err(e) = sync_file.sync_all().await {
            drop(sync_file);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(ServiceError::Internal(format!("同步临时文件失败: {}", e)));
        }
        drop(sync_file);

        // Move temp file to final destination. The dest name is composed only
        // of the original filename's *base* (already sanitised) prefixed with
        // a timestamp and a random component — never with anything
        // user-controlled beyond the alphanumeric body of the filename. The
        // UUID guard prevents two concurrent uploads with the same name from
        // silently clobbering each other (rename(2) replaces atomically on
        // Unix).
        let dest_base = Path::new(&sanitized_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "video.mp4".to_string());
        let dest_file_name = format!(
            "{}_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            Uuid::new_v4().simple(),
            dest_base
        );
        let dest_path = self.config.media_root.join(&dest_file_name);
        if let Err(e) = tokio::fs::rename(&temp_path, &dest_path).await {
            // rename failed — the upload still lives at the temp path; clean
            // it up so we don't leak the temp file.
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(ServiceError::Internal(format!("移动文件失败: {}", e)));
        }

        let stream_url = format!("/media/{}", dest_file_name);

        let id = match self
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
        {
            Ok(id) => id,
            Err(e) => {
                // The file was already moved to its final destination but no
                // DB row references it — remove it to avoid an orphaned file
                // that will never be cleaned up or served.
                let _ = tokio::fs::remove_file(&dest_path).await;
                return Err(ServiceError::Internal(e.to_string()));
            }
        };

        // Charge the user's quota counter.
        if let Err(e) = self
            .repo
            .increment_storage_used(uploader_id, file_size)
            .await
        {
            tracing::warn!(
                uploader = uploader_id,
                bytes = file_size,
                "Failed to update storage quota: {}",
                e
            );
        }

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
    pub async fn generate_thumbnail(&self, video_id: i64) -> Result<bool, ServiceError> {
        let video = self
            .repo
            .find_by_id(video_id)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        let video = video.ok_or_else(|| ServiceError::Internal("not found".to_string()))?;

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
                return Err(ServiceError::Internal(format!(
                    "video file not found or invalid path: {}",
                    video.stream_url
                )))
            }
        };
        let video_path_exists = tokio::task::spawn_blocking({
            let vp = video_path.clone();
            move || vp.exists()
        })
        .await
        .unwrap_or(false);
        if !video_path_exists {
            return Err(ServiceError::Internal(format!("video file not found: {}", video_path.display())));
        }

        // Limit concurrent ffmpeg thumbnail jobs and give each one a hard
        // timeout, so a hung process can't pin a blocking worker forever.
        let _permit = match thumbnail_semaphore().acquire().await {
            Ok(p) => p,
            Err(_) => return Err(ServiceError::Internal("thumbnail semaphore closed".to_string())),
        };

        // Extract frame at 1 second using ffmpeg
        let cover_file_name = format!("cover_{}.jpg", video_id);
        let cover_path = self.config.media_root.join(&cover_file_name);
        let cover_path_str = cover_path.to_string_lossy().to_string();
        let video_path_clone = video_path.clone();

        let output = tokio::time::timeout(
            Duration::from_secs(THUMBNAIL_FFMPEG_TIMEOUT_SECS),
            tokio::process::Command::new("ffmpeg")
                .kill_on_drop(true)
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
                .output(),
        )
        .await
        .map_err(|_| {
            ServiceError::Internal(format!(
                "ffmpeg thumbnail generation timed out after {}s",
                THUMBNAIL_FFMPEG_TIMEOUT_SECS
            ))
        })?
        .map_err(|e| ServiceError::Internal(format!("ffmpeg not found: {}", e)))?;

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
                return Err(ServiceError::Internal(format!(
                    "ffmpeg failed: {}",
                    stderr.lines().next().unwrap_or("unknown error")
                )));
            }
        }

        let cover_url = format!("/media/{}", cover_file_name);
        self.repo
            .update_cover_url(video_id, &cover_url)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        // Generate a smaller thumbnail (320x180) for grid use
        let thumb_file_name = format!("thumb_{}.jpg", video_id);
        let thumb_path = self.config.media_root.join(&thumb_file_name);
        let thumb_path_str = thumb_path.to_string_lossy().to_string();
        let cover_clone = cover_path.clone();
        let thumb_result = tokio::time::timeout(
            Duration::from_secs(THUMBNAIL_FFMPEG_TIMEOUT_SECS),
            tokio::process::Command::new("ffmpeg")
                .kill_on_drop(true)
                .arg("-y")
                .arg("-i")
                .arg(&cover_clone)
                .arg("-vf")
                .arg("scale=320:180")
                .arg("-q:v")
                .arg("5")
                .arg(&thumb_path_str)
                .output(),
        )
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
    pub async fn backfill_thumbnails(&self) -> Result<(i64, Vec<String>), ServiceError> {
        let mut generated = 0i64;
        let mut errors = Vec::new();
        let mut last_id: i64 = 0;

        loop {
            let rows = self
                .repo
                .find_videos_without_cover(last_id, 100)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
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

    pub async fn update_cover(&self, id: i64, file_name: &str, bytes: Bytes) -> Result<(), ServiceError> {
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            // The extension is embedded into filesystem paths, so keep only
            // ASCII alphanumerics and bound its length (the client's filename
            // is otherwise untrusted).
            .map(|e| {
                e.chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .take(16)
                    .collect::<String>()
            })
            .filter(|e| !e.is_empty())
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
            .map_err(|e| ServiceError::Internal(format!("写入临时封面失败: {}", e)))?;

        // Validate the uploaded file using magic bytes
        let validation = tokio::task::spawn_blocking({
            let tp = tmp_path.clone();
            let ex = ext.clone();
            move || validate_file_type(&tp, &ex)
        })
        .await
        .map_err(|e| {
            let tp = tmp_path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(&tp).await;
            });
            ServiceError::Internal(format!("验证任务失败: {}", e))
        })?;
        if let Err(e) = validation {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(ServiceError::BadRequest(format!("封面上传验证失败: {}", e)));
        }

        let cover_file_name = format!(
            "cover_{}_{}.{}",
            id,
            chrono::Utc::now().timestamp_millis(),
            ext
        );
        let cover_path = self.config.media_root.join(&cover_file_name);

        if let Err(e) = tokio::fs::rename(&tmp_path, &cover_path).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(ServiceError::Internal(format!("移动封面失败: {}", e)));
        }

        let cover_url = format!("/media/{}", cover_file_name);
        if let Err(e) = self.repo.update_cover_url(id, &cover_url).await {
            // The cover was moved to its final name but no DB row points to
            // it — remove it so we don't leak an unreferenced file.
            let _ = tokio::fs::remove_file(&cover_path).await;
            return Err(ServiceError::Internal(e.to_string()));
        }
        Ok(())
    }
}
