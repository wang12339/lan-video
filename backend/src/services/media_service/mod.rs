use axum::body::Bytes;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::repositories::video_repo::{SaveVideoOutcome, VideoRepository};
use crate::services::exif_service::parse_exif_file;
use crate::util::error::ServiceError;

pub mod session;
pub mod upload;
mod validate;

pub use session::{UploadAppendError, UploadAppendOutcome};

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

/// EXIF 回填的每批数量（`backfill_image_exif`），避免一次性加载全部图片。
const EXIF_BACKFILL_BATCH: i64 = 200;

/// Cap on the number of concurrent thumbnail-generation ffmpeg processes.
/// Each one is CPU-heavy, so unlimited parallelism would saturate the host.
static THUMBNAIL_SEMAPHORE: OnceLock<tokio::sync::Semaphore> = OnceLock::new();

fn thumbnail_semaphore() -> &'static tokio::sync::Semaphore {
    THUMBNAIL_SEMAPHORE.get_or_init(|| tokio::sync::Semaphore::new(2))
}

#[derive(Clone)]
pub struct MediaService {
    repo: VideoRepository,
    config: AppConfig,
    /// 分片续传会话状态（按 uploader+hash 隔离）：
    /// 保存增量哈希器与已接收字节数，避免 finalize 时对整文件二次全量读取。
    pub(super) upload_slots:
        Arc<dashmap::DashMap<String, Arc<tokio::sync::Mutex<session::UploadSlot>>>>,
    /// 限制并发分片追加/finalize 操作（磁盘写入 + fsync）。
    pub(super) upload_semaphore: Arc<tokio::sync::Semaphore>,
}

impl MediaService {
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        let svc = Self {
            repo,
            config,
            upload_slots: Arc::new(dashmap::DashMap::new()),
            upload_semaphore: Arc::new(tokio::sync::Semaphore::new(
                session::MAX_CONCURRENT_UPLOAD_OPS,
            )),
        };
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
                    svc.cleanup_upload_slots();
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
        tokio::task::spawn_blocking(move || {
            let mut removed = sweep_upload_temps_blocking(&root, UPLOAD_TEMP_TTL)?;
            // 聊天媒体目录（chat/）的中断上传临时文件同样兜底清理
            let chat_dir = root.join("chat");
            if chat_dir.is_dir() {
                if let Ok(n) = sweep_upload_temps_blocking(&chat_dir, UPLOAD_TEMP_TTL) {
                    removed += n;
                }
            }
            Ok::<usize, String>(removed)
        })
        .await
        .map_err(|e| ServiceError::Internal(format!("临时文件清理任务失败: {}", e)))?
        .map_err(ServiceError::Internal)
    }

    /// 流式上传：从临时文件读取，计算 SHA-256，移动到最终位置。
    ///
    /// 任何失败都会清理临时文件（multipart 整文件上传等调用方沿用此语义）。
    pub async fn upload_video_file(
        &self,
        file_name: &str,
        temp_path: &std::path::Path,
        category: &str,
        uploader_id: i64,
        precomputed: Option<(i64, String)>,
    ) -> Result<i64, ServiceError> {
        self.upload_video_file_inner(
            file_name,
            temp_path,
            category,
            uploader_id,
            precomputed,
            false,
        )
        .await
    }

    /// finalize 专用入口：瞬时错误（Internal：DB/磁盘抖动）时保留临时文件与
    /// 哈希状态，允许客户端用空 body + offset=total 重试 finalize，而不是整个
    /// 文件重传。永久错误（参数/重复/配额/类型校验）仍会清理临时文件。
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn upload_video_file_for_finalize(
        &self,
        file_name: &str,
        temp_path: &std::path::Path,
        category: &str,
        uploader_id: i64,
        precomputed: Option<(i64, String)>,
    ) -> Result<i64, ServiceError> {
        self.upload_video_file_inner(
            file_name,
            temp_path,
            category,
            uploader_id,
            precomputed,
            true,
        )
        .await
    }

    /// 上传失败时的临时文件清理：仅在 `keep` 为 false 时删除。
    /// 瞬时错误保留临时文件，由 24h 清扫任务兜底。
    async fn cleanup_upload_temp(path: &Path, keep: bool) {
        if !keep {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn upload_video_file_inner(
        &self,
        file_name: &str,
        temp_path: &std::path::Path,
        category: &str,
        uploader_id: i64,
        precomputed: Option<(i64, String)>,
        keep_temp_on_internal_error: bool,
    ) -> Result<i64, ServiceError> {
        // SECURITY (L-03): 上传入口（multipart 字段 / x-upload-category 头）此前
        // 无 category 校验。在读取文件前快速失败——50GB 文件不应为错误分类
        // 浪费一次全量读取；临时文件由调用方与下方错误路径共同清理。
        if let Err(e) = validate_category(category) {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(ServiceError::BadRequest(e));
        }

        // 计算 SHA-256：优先复用收流阶段边写边算的结果（避免对大文件二次
        // 全量读取）；续传路径无法预计算，退回自身流式读取。
        let (file_size, hash) = match precomputed {
            Some((size, h)) => (size, h),
            None => {
                use tokio::io::AsyncReadExt;
                let mut file = tokio::fs::File::open(temp_path)
                    .await
                    .map_err(|e| ServiceError::Internal(format!("打开临时文件失败: {}", e)))?;
                let mut hasher = Sha256::new();
                let mut buf = vec![0u8; 65536];
                let mut size: i64 = 0;
                loop {
                    let n = file
                        .read(&mut buf)
                        .await
                        .map_err(|e| ServiceError::Internal(format!("读取临时文件失败: {}", e)))?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buf[..n]);
                    size += n as i64;
                }
                drop(file);
                (size, format!("{:x}", hasher.finalize()))
            }
        };

        // SECURITY (A04 H2): 配额校验与扣减已下沉到
        // `save_local_video_with_quota` 的单个事务（用户行 FOR UPDATE 串行化），
        // 不再在这里做无锁预检查；这样并发上传不会超卖，计费失败也不会被吞掉。

        // Check for duplicates using server-computed hash（按上传者隔离）
        match self.repo.find_video_by_file_hash(uploader_id, &hash).await {
            Ok(Some(_)) => {
                let _ = tokio::fs::remove_file(temp_path).await;
                return Err(ServiceError::Duplicate("文件已存在".into()));
            }
            Ok(None) => {}
            Err(e) => {
                Self::cleanup_upload_temp(temp_path, keep_temp_on_internal_error).await;
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
                Self::cleanup_upload_temp(&temp_path, keep_temp_on_internal_error).await;
                return Err(ServiceError::Internal(format!("打开临时文件失败: {}", e)));
            }
        };
        if let Err(e) = sync_file.sync_all().await {
            drop(sync_file);
            Self::cleanup_upload_temp(&temp_path, keep_temp_on_internal_error).await;
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
            // it up (unless the caller wants to retry finalize) so we don't
            // leak the temp file.
            Self::cleanup_upload_temp(&temp_path, keep_temp_on_internal_error).await;
            return Err(ServiceError::Internal(format!("移动文件失败: {}", e)));
        }

        let stream_url = format!("/media/{}", dest_file_name);

        let id = match self
            .repo
            .save_local_video_with_quota(
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
                self.config.upload_quota_bytes,
            )
            .await
        {
            Ok(SaveVideoOutcome::Ok(row)) => row.id,
            Ok(SaveVideoOutcome::Duplicate) => {
                // 并发下唯一索引 uq_videos_uploader_file_hash 命中的情况：
                // 与预检查同语义，映射为"文件已存在"而不是 500。
                let _ = tokio::fs::remove_file(&dest_path).await;
                return Err(ServiceError::Duplicate("文件已存在".into()));
            }
            Ok(SaveVideoOutcome::QuotaExceeded) => {
                let _ = tokio::fs::remove_file(&dest_path).await;
                return Err(ServiceError::QuotaExceeded("存储配额已用尽".into()));
            }
            Ok(SaveVideoOutcome::UserNotFound) => {
                let _ = tokio::fs::remove_file(&dest_path).await;
                return Err(ServiceError::Internal("上传者不存在".into()));
            }
            Err(e) => {
                // The file was already moved to its final destination but no
                // DB row references it — remove it to avoid an orphaned file
                // that will never be cleaned up or served.
                let _ = tokio::fs::remove_file(&dest_path).await;
                return Err(ServiceError::Internal(e.to_string()));
            }
        };

        // 配额扣减已在 save_local_video_with_quota 的事务内完成（失败整体
        // 回滚），这里不再单独 increment，也不会吞掉计费失败。

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

        // 图片：后台解析原图 EXIF。解析不到（截图/导出图）也要把
        // exif_extracted 置 TRUE，否则后台回填任务会对同一张图反复重试。
        // 所有失败仅告警，绝不影响上传主流程。
        if is_image {
            let svc = self.clone();
            let vid = id;
            let path = dest_path.clone();
            tokio::spawn(async move {
                let parsed = tokio::task::spawn_blocking(move || parse_exif_file(&path)).await;
                match parsed {
                    Ok(Some(exif)) => {
                        if let Err(e) = svc.repo.update_video_exif(vid, &exif).await {
                            tracing::warn!(video_id = vid, error = %e, "exif update failed");
                        }
                    }
                    Ok(None) => {
                        if let Err(e) = mark_exif_extracted(svc.repo.pool(), vid).await {
                            tracing::warn!(video_id = vid, error = %e, "exif mark failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(video_id = vid, error = %e, "exif parse task failed");
                    }
                }
            });
        }

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

        // Only generate for local videos and local images
        if !video.source_type.starts_with("local_video")
            && !video.source_type.starts_with("local_image")
        {
            return Ok(false);
        }

        let is_image = video.source_type.starts_with("local_image");

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
        } else if is_image && video.thumb_url.is_some() {
            // 图片没有封面帧概念：只要网格缩略图已存在就无事可做
            if let Some(thumb_url) = &video.thumb_url {
                if let Some(thumb_path) = safe_media_path(thumb_url, &self.config.media_root) {
                    let thumb_exists = tokio::task::spawn_blocking({
                        let tp = thumb_path.clone();
                        move || tp.exists()
                    })
                    .await
                    .unwrap_or(false);
                    if thumb_exists {
                        return Ok(false);
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
            return Err(ServiceError::Internal(format!(
                "video file not found: {}",
                video_path.display()
            )));
        }

        // Limit concurrent ffmpeg thumbnail jobs and give each one a hard
        // timeout, so a hung process can't pin a blocking worker forever.
        let _permit = match thumbnail_semaphore().acquire().await {
            Ok(p) => p,
            Err(_) => {
                return Err(ServiceError::Internal(
                    "thumbnail semaphore closed".to_string(),
                ))
            }
        };

        // ── 图片分支：stream_url 就是原图本身 ──
        // 只生成网格缩略图（最长边≤640、保持宽高比、只缩不放），
        // 不做封面帧提取。移动端网格因此不再加载全尺寸原图。
        if is_image {
            let thumb_file_name = format!("thumb_{}.jpg", video_id);
            let thumb_path = self.config.media_root.join(&thumb_file_name);
            let thumb_path_str = thumb_path.to_string_lossy().to_string();
            let image_path_str = video_path.to_string_lossy().to_string();

            let output = tokio::time::timeout(
                Duration::from_secs(THUMBNAIL_FFMPEG_TIMEOUT_SECS),
                tokio::process::Command::new("ffmpeg")
                    .kill_on_drop(true)
                    .arg("-y")
                    .arg("-i")
                    .arg(&image_path_str)
                    .arg("-vframes")
                    .arg("1")
                    .arg("-vf")
                    // 缩到宽 640 内（保持宽高比，-2 保证偶数），小图不放大
                    .arg("scale=min(640\\,iw):-2")
                    .arg("-q:v")
                    .arg("5")
                    .arg(&thumb_path_str)
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

            if output.status.success() {
                let thumb_url = format!("/media/{}", thumb_file_name);
                self.repo
                    .update_thumb_url(video_id, &thumb_url)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;
                info!(
                    "Generated image thumbnail for video {}: {}",
                    video_id, thumb_url
                );
                return Ok(true);
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ServiceError::Internal(format!(
                "ffmpeg image thumbnail failed: {}",
                stderr.lines().next().unwrap_or("unknown error")
            )));
        }

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

        // Generate a smaller thumbnail (640x360，适配 @2x/@3x 网格) for grid use
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
                .arg("scale=640:360")
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

    /// Backfill EXIF metadata for local images that have not been parsed yet.
    ///
    /// 分批（每批 [`EXIF_BACKFILL_BATCH`]）取 `exif_extracted = FALSE`
    /// 的本地图片，把 `/media/...` 的 `stream_url` 经 `media_root` 映射为真实
    /// 文件后在阻塞线程池解析：
    /// - 解析出 EXIF：写入全部 exif 列并置 `exif_extracted = TRUE`
    /// - 无 EXIF / 文件缺失 / 非法路径：仅置 `exif_extracted = TRUE`
    ///
    /// 返回 `(处理数量, 错误列表)`；单个文件失败不中止整体回填。
    /// 本地 `attempted` 集合保证 DB 抖动导致置位失败时循环仍会终止
    /// （`find_images_without_exif` 无游标，失败行下一轮会被再次返回）。
    pub async fn backfill_image_exif(&self) -> Result<(i64, Vec<String>), ServiceError> {
        let mut processed = 0i64;
        let mut errors: Vec<String> = Vec::new();
        let mut attempted: std::collections::HashSet<i64> = std::collections::HashSet::new();

        loop {
            let rows = self
                .repo
                .find_images_without_exif(EXIF_BACKFILL_BATCH)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
            if rows.is_empty() {
                break;
            }

            let mut progressed = false;
            for (id, stream_url) in rows {
                if !attempted.insert(id) {
                    continue;
                }
                progressed = true;
                processed += 1;

                let Some(path) = safe_media_path(&stream_url, &self.config.media_root) else {
                    // 文件缺失 / 路径非法：标记已尝试，避免每轮重复处理
                    if let Err(e) = mark_exif_extracted(self.repo.pool(), id).await {
                        errors.push(format!("id={}: {}", id, e));
                    }
                    continue;
                };

                let parsed = tokio::task::spawn_blocking(move || parse_exif_file(&path)).await;
                match parsed {
                    Ok(Some(exif)) => {
                        if let Err(e) = self.repo.update_video_exif(id, &exif).await {
                            errors.push(format!("id={}: {}", id, e));
                        }
                    }
                    Ok(None) => {
                        if let Err(e) = mark_exif_extracted(self.repo.pool(), id).await {
                            errors.push(format!("id={}: {}", id, e));
                        }
                    }
                    Err(e) => errors.push(format!("id={}: parse task failed: {}", id, e)),
                }
            }

            if !progressed {
                break;
            }
        }

        Ok((processed, errors))
    }

    pub async fn update_cover(
        &self,
        id: i64,
        file_name: &str,
        bytes: Bytes,
    ) -> Result<(), ServiceError> {
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

/// 仅标记"已尝试解析 EXIF"（无 EXIF / 文件缺失），不写任何 exif 列。
///
/// `videos` 表的这条 `UPDATE` 语义与 `VideoRepository::update_video_exif`
/// 的 `exif_extracted = TRUE` 收尾一致；参数化绑定，无注入面。放在 service
/// 层是因为仓储层未提供单独的 mark 方法，而避免为此新增跨文件改动。
async fn mark_exif_extracted(pool: &sqlx::PgPool, video_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE videos SET exif_extracted = TRUE WHERE id = $1")
        .bind(video_id)
        .execute(pool)
        .await
        .map(|_| ())
}
