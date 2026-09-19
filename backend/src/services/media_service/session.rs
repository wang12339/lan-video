//! 分片续传的会话状态与追加/校验逻辑。
//!
//! 设计要点：
//! - 进行中的上传**不落库**，状态保存在 `MediaService.upload_slots`
//!   （进程内 DashMap，键为 `{uploader}:{hash}`）。
//! - 每个槽位持有增量 SHA-256，每次追加同步更新，finalize 时无需
//!   再对整文件二次全量读取（续传首次接触时仍需读一遍已有临时文件
//!   重建哈希状态——服务重启后无法序列化哈希器）。
//! - 追加按 `x-upload-offset` 严格校验偏移：超时重试导致的重复分片
//!   会得到 409 `offset_mismatch` 而不会把临时文件写坏（幂等）。
//! - finalize 时比对服务端增量哈希与客户端声明的 `x-upload-hash`，
//!   不一致（传输损坏/客户端哈希错误）返回 400 `hash_mismatch`。
//! - 完成后槽位保留一段时间（`COMPLETED_SLOT_TTL`）以幂等响应
//!   丢失的 201（重放同一个 final 分片仍返回原 ID）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use super::MediaService;
use crate::util::error::ServiceError;

/// 并发分片操作上限（磁盘追加 + fsync），防止大量并发上传打满 IO。
pub const MAX_CONCURRENT_UPLOAD_OPS: usize = 32;

/// 已完成槽位的保留时长：覆盖客户端 201 丢失后的重试窗口。
pub const COMPLETED_SLOT_TTL: Duration = Duration::from_secs(10 * 60);

enum UploadPhase {
    Receiving,
    Completed { video_id: i64 },
}

/// 单个上传会话的进程内状态。
pub struct UploadSlot {
    /// 是否已用磁盘上的临时文件初始化过哈希状态。
    initialized: bool,
    hasher: Sha256,
    size: i64,
    phase: UploadPhase,
    last_activity: Instant,
}

impl UploadSlot {
    fn new() -> Self {
        Self {
            initialized: false,
            hasher: Sha256::new(),
            size: 0,
            phase: UploadPhase::Receiving,
            last_activity: Instant::now(),
        }
    }

    fn reset(&mut self) {
        self.initialized = true;
        self.hasher = Sha256::new();
        self.size = 0;
        self.phase = UploadPhase::Receiving;
        self.last_activity = Instant::now();
    }

    fn completed(&self) -> Option<i64> {
        match self.phase {
            UploadPhase::Completed { video_id } => Some(video_id),
            UploadPhase::Receiving => None,
        }
    }
}

/// 一次分片追加/查询的结果。
#[derive(Debug)]
pub struct UploadAppendOutcome {
    pub received: i64,
    /// 仅当本次请求完成 finalize（或幂等重放已完成的上传）时有值。
    pub video_id: Option<i64>,
}

/// 分片上传错误（带机器可读语义，供 handler 映射状态码与 `code`）。
#[derive(Debug)]
pub enum UploadAppendError {
    /// 请求的 offset 与服务端已接收字节数不一致 —— 客户端应回退到
    /// `received` 处重新分片（409）。
    OffsetMismatch { received: i64 },
    /// 服务端增量哈希与客户端声明不一致（400）。
    HashMismatch,
    /// 同上传者已有相同内容（409）。
    Duplicate(String),
    /// 存储配额超限（507）。
    QuotaExceeded(String),
    /// 请求不合法（400）。
    BadRequest(String),
    /// 服务端错误（500）。
    Internal(String),
}

impl From<ServiceError> for UploadAppendError {
    fn from(e: ServiceError) -> Self {
        match e {
            ServiceError::Duplicate(m) => Self::Duplicate(m),
            ServiceError::QuotaExceeded(m) => Self::QuotaExceeded(m),
            ServiceError::BadRequest(m) => Self::BadRequest(m),
            other => Self::Internal(other.to_string()),
        }
    }
}

/// 读取文件并重建 SHA-256 状态（服务重启后续传时使用一次）。
async fn hash_file_into(path: &Path) -> Result<(i64, Sha256), ServiceError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| ServiceError::Internal(format!("打开临时文件失败: {}", e)))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total: i64 = 0;
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| ServiceError::Internal(format!("读取临时文件失败: {}", e)))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as i64;
    }
    Ok((total, hasher))
}

impl MediaService {
    fn slot_key(uploader_id: i64, hash: &str) -> String {
        format!("{}:{}", uploader_id, hash)
    }

    /// 该上传者对应的临时文件路径。按 uploader 隔离，避免不同
    /// 用户上传相同内容时互相踩踏同一个临时文件。
    pub fn upload_temp_path(&self, uploader_id: i64, hash: &str) -> PathBuf {
        self.config
            .media_root
            .join(format!(".upload_{}_{}", uploader_id, hash))
    }

    fn slot_handle(&self, key: &str) -> Arc<Mutex<UploadSlot>> {
        self.upload_slots
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(UploadSlot::new())))
            .value()
            .clone()
    }

    /// 首次接触槽位时用磁盘上已有的临时文件重建哈希状态。
    async fn init_slot(&self, slot: &mut UploadSlot, tmp: &Path) -> Result<(), ServiceError> {
        if slot.initialized {
            return Ok(());
        }
        if let Ok(meta) = tokio::fs::metadata(tmp).await {
            if meta.is_file() {
                let (size, hasher) = hash_file_into(tmp).await?;
                slot.hasher = hasher;
                slot.size = size;
            }
        }
        slot.initialized = true;
        Ok(())
    }

    /// 只读查询：已接收字节数（供 `GET /admin/videos/upload-status`）。
    pub async fn upload_received_bytes(
        &self,
        uploader_id: i64,
        hash: &str,
    ) -> Result<i64, ServiceError> {
        let tmp = self.upload_temp_path(uploader_id, hash);
        let key = Self::slot_key(uploader_id, hash);
        let handle = self.slot_handle(&key);
        let _permit = self
            .upload_semaphore
            .acquire()
            .await
            .map_err(|e| ServiceError::Internal(format!("上传信号量已关闭: {}", e)))?;
        let mut guard = handle.lock().await;
        self.init_slot(&mut guard, &tmp).await?;
        guard.last_activity = Instant::now();
        Ok(guard.size)
    }

    /// 去重预检：返回同上传者已存在的视频 ID（按 file_hash）。
    pub async fn find_upload_duplicate(
        &self,
        uploader_id: i64,
        hash: &str,
    ) -> Result<Option<i64>, ServiceError> {
        Ok(self
            .repo
            .find_video_by_file_hash(uploader_id, hash)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?
            .map(|row| row.id))
    }

    /// 配额预检（`additional` 为该文件的完整大小；上传前快速失败）。
    pub async fn ensure_upload_quota(
        &self,
        uploader_id: i64,
        additional: i64,
    ) -> Result<(), ServiceError> {
        let quota = self.config.upload_quota_bytes;
        if quota <= 0 {
            return Ok(());
        }
        let used = self
            .repo
            .get_storage_used(uploader_id)
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        if used + additional > quota {
            return Err(ServiceError::QuotaExceeded("存储配额已用尽".into()));
        }
        Ok(())
    }

    /// 追加一个分片；`data` 为空时表示进度查询（`offset == Some(total_size)`
    /// 时触发 finalize，用于客户端恢复"临时文件已完整但尚未入库"的场景）。
    ///
    /// `offset` 为 `None` 时按旧协议在文件末尾追加（不具备幂等性，仅为
    /// 兼容旧客户端）。
    #[allow(clippy::too_many_arguments)]
    pub async fn append_upload_chunk(
        &self,
        uploader_id: i64,
        hash: &str,
        file_name: &str,
        total_size: i64,
        category: &str,
        offset: Option<i64>,
        data: &[u8],
    ) -> Result<UploadAppendOutcome, UploadAppendError> {
        let tmp = self.upload_temp_path(uploader_id, hash);
        let key = Self::slot_key(uploader_id, hash);
        // 锁顺序固定为 信号量 → 槽位锁，避免与只读查询路径反向加锁死锁。
        let _permit = self
            .upload_semaphore
            .acquire()
            .await
            .map_err(|e| UploadAppendError::Internal(format!("上传信号量已关闭: {}", e)))?;
        let handle = self.slot_handle(&key);
        let mut guard = handle.lock().await;

        self.init_slot(&mut guard, &tmp)
            .await
            .map_err(UploadAppendError::from)?;
        guard.last_activity = Instant::now();

        // 服务端临时文件超过声明大小（异常残留）：从头开始，要求客户端重置。
        if guard.size > total_size {
            guard.reset();
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(UploadAppendError::OffsetMismatch { received: 0 });
        }

        // 已完成：幂等重放 / 重新上传 / 拒绝乱序。
        if let Some(video_id) = guard.completed() {
            if !data.is_empty() {
                let replayed = offset
                    .map(|off| off + data.len() as i64 == guard.size)
                    .unwrap_or(false);
                if replayed {
                    return Ok(UploadAppendOutcome {
                        received: guard.size,
                        video_id: Some(video_id),
                    });
                }
                if offset.unwrap_or(0) == 0 {
                    // 同一内容重新发起上传：允许从零开始（最终会被去重拦截）
                    guard.reset();
                    let _ = tokio::fs::remove_file(&tmp).await;
                } else {
                    return Err(UploadAppendError::OffsetMismatch { received: 0 });
                }
            } else if offset == Some(guard.size) {
                // final 分片的 201 响应丢失后，客户端用空 body 再触发一次
                return Ok(UploadAppendOutcome {
                    received: guard.size,
                    video_id: Some(video_id),
                });
            }
        }

        // 空 body：进度查询或显式 finalize 触发。
        if data.is_empty() {
            if offset == Some(guard.size) && guard.size == total_size {
                let id = self
                    .finalize_slot(&mut guard, uploader_id, file_name, category, hash, &tmp)
                    .await?;
                return Ok(UploadAppendOutcome {
                    received: guard.size,
                    video_id: Some(id),
                });
            }
            return Ok(UploadAppendOutcome {
                received: guard.size,
                video_id: None,
            });
        }

        // 偏移校验：客户端必须从服务端已接收位置继续。
        let expected = guard.size;
        let start = offset.unwrap_or(expected);
        if start != expected {
            return Err(UploadAppendError::OffsetMismatch { received: expected });
        }
        if expected + data.len() as i64 > total_size {
            return Err(UploadAppendError::BadRequest(
                "分片超出声明的文件总大小".into(),
            ));
        }

        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tmp)
            .await
            .map_err(|_| UploadAppendError::Internal("打开临时文件失败".into()))?;
        f.write_all(data)
            .await
            .map_err(|_| UploadAppendError::Internal("写入失败".into()))?;
        f.flush()
            .await
            .map_err(|_| UploadAppendError::Internal("flush失败".into()))?;
        drop(f);

        guard.hasher.update(data);
        guard.size += data.len() as i64;

        if guard.size == total_size {
            let id = self
                .finalize_slot(&mut guard, uploader_id, file_name, category, hash, &tmp)
                .await?;
            return Ok(UploadAppendOutcome {
                received: guard.size,
                video_id: Some(id),
            });
        }

        Ok(UploadAppendOutcome {
            received: guard.size,
            video_id: None,
        })
    }

    /// 校验增量哈希并落库。持有槽位锁期间调用，保证 finalize 期间不会
    /// 被重试分片追加。
    #[allow(clippy::too_many_arguments)]
    async fn finalize_slot(
        &self,
        slot: &mut UploadSlot,
        uploader_id: i64,
        file_name: &str,
        category: &str,
        declared_hash: &str,
        tmp: &Path,
    ) -> Result<i64, UploadAppendError> {
        let actual = format!("{:x}", slot.hasher.clone().finalize());
        if !actual.eq_ignore_ascii_case(declared_hash) {
            tracing::warn!(
                uploader = uploader_id,
                "upload hash mismatch: declared {} actual {}",
                declared_hash,
                actual
            );
            slot.reset();
            let _ = tokio::fs::remove_file(tmp).await;
            return Err(UploadAppendError::HashMismatch);
        }
        match self
            .upload_video_file_for_finalize(
                file_name,
                tmp,
                category,
                uploader_id,
                Some((slot.size, actual)),
            )
            .await
        {
            Ok(id) => {
                slot.phase = UploadPhase::Completed { video_id: id };
                slot.last_activity = Instant::now();
                Ok(id)
            }
            Err(ServiceError::Internal(msg))
                if tokio::fs::try_exists(tmp).await.unwrap_or(false) =>
            {
                // 瞬时错误（DB/磁盘抖动）且临时文件仍在：保留哈希状态与已接收
                // 字节数，客户端可用空 body + offset=total 重试 finalize，
                // 不必重传整个文件。临时文件由 24h 清扫任务兜底。
                tracing::warn!(
                    uploader = uploader_id,
                    size = slot.size,
                    "finalize failed transiently, keeping upload for retry: {}",
                    msg
                );
                slot.last_activity = Instant::now();
                Err(UploadAppendError::Internal(msg))
            }
            Err(e) => {
                // 永久错误（参数/重复/配额/类型校验）或临时文件已随失败清理：
                // 重置槽位，客户端需从头重传。
                // upload_video_file 的错误路径已清理临时文件；这里再兜底一次。
                slot.reset();
                let _ = tokio::fs::remove_file(tmp).await;
                Err(UploadAppendError::from(e))
            }
        }
    }

    /// 清理过期槽位（随临时文件清扫任务周期执行）：
    /// - 完成态保留 [`COMPLETED_SLOT_TTL`]（覆盖 201 丢失重试）
    /// - 接收态保留 [`super::UPLOAD_TEMP_TTL`]（与临时文件 TTL 对齐）
    ///
    /// 正在被使用的槽位（锁被持有）一律保留；临时文件由文件清扫任务兜底。
    pub fn cleanup_upload_slots(&self) {
        let now = Instant::now();
        self.upload_slots.retain(|_, handle| {
            let Ok(slot) = handle.try_lock() else {
                return true;
            };
            let ttl = if slot.completed().is_some() {
                COMPLETED_SLOT_TTL
            } else {
                super::UPLOAD_TEMP_TTL
            };
            now.duration_since(slot.last_activity) < ttl
        });
    }
}
