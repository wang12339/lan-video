//! Upload-lock sweeper (SECURITY L-07).
//!
//! Chunked uploads hold a per-hash `DashMap` mutex in `AppState::upload_locks`;
//! only successful completion removes the entry.  Abandoned / failed uploads
//! leave behind stale entries that slowly grow process memory.
//!
//! This module runs a lazy, once-per-process Tokio task that periodically
//! prunes entries whose backing `.upload_{hash}` temp file is missing or older
//! than [`UPLOAD_TEMP_TTL`] (the same criterion the file-sweeper uses).

use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::services::media_service::UPLOAD_TEMP_TTL;
use crate::state::AppState;

/// upload_locks 清理间隔：与 media_service 临时文件清扫同频。
const UPLOAD_LOCK_CLEANUP_INTERVAL: Duration = Duration::from_secs(3600);

/// 惰性启动 upload_locks 的周期性清理（SECURITY L-07）。续传上传在
/// DashMap 中为每个 hash 创建互斥锁，仅在成功收尾时移除；放弃/失败的上传
/// 会遗留条目导致内存缓慢增长。本任务移除"临时文件不存在或超过 24h 未变化"
/// 的条目（与 media_service 临时文件清扫同一判定标准）。由首次 /videos
/// 列表请求触发，进程内只启动一个任务（OnceLock 幂等）。
pub fn ensure_upload_lock_cleanup(state: &Arc<AppState>) {
    static CLEANUP_STARTED: OnceLock<()> = OnceLock::new();
    if CLEANUP_STARTED.get().is_some() {
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let state = state.clone();
    CLEANUP_STARTED.get_or_init(|| {
        std::mem::drop(handle.spawn(async move {
            let mut interval = tokio::time::interval(UPLOAD_LOCK_CLEANUP_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                prune_upload_locks(&state).await;
            }
        }));
    });
}

async fn prune_upload_locks(state: &Arc<AppState>) {
    if state.upload_locks.is_empty() {
        return;
    }
    let root = state.config.media_root.clone();
    let locks = state.upload_locks.clone();
    let stale = tokio::task::spawn_blocking(move || {
        stale_upload_lock_keys_inner(&locks, &root, UPLOAD_TEMP_TTL)
    })
    .await
    .unwrap_or_default();
    if stale.is_empty() {
        return;
    }
    for key in &stale {
        state.upload_locks.remove(key);
    }
    tracing::info!(
        removed = stale.len(),
        remaining = state.upload_locks.len(),
        "pruned stale upload lock entries"
    );
}

/// 返回应移除的锁 key：其 `.upload_{hash}` 临时文件缺失，或 mtime 超过
/// `ttl`（与 media_service 临时文件清扫同标准——文件已超龄，锁必然已死）。
/// 对 hash 做格式校验，防止意外 key 拼出目录外路径。竞态说明：entry 创建到
/// 临时文件落盘之间是微秒级窗口，且清理每小时一次；最坏情况是同一 hash
/// 短暂出现两个互斥锁，下一次 chunk 请求会重建串行化，可接受。
#[cfg(test)]
pub(crate) fn stale_upload_lock_keys(
    locks: &dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>>,
    media_root: &Path,
    ttl: Duration,
) -> Vec<String> {
    stale_upload_lock_keys_inner(locks, media_root, ttl)
}

fn stale_upload_lock_keys_inner(
    locks: &dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>>,
    media_root: &Path,
    ttl: Duration,
) -> Vec<String> {
    let now = std::time::SystemTime::now();
    locks
        .iter()
        .filter_map(|entry| {
            let hash = entry.key();
            let valid_hash = hash.len() <= 128
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
            if !valid_hash {
                return Some(hash.clone());
            }
            let path = media_root.join(format!(".upload_{}", hash));
            // 保守判定：文件存在且 mtime 可读且未超龄 → 存活；其余情况（含
            // mtime 读取失败、时钟异常）一律视为死亡并移除锁条目。
            let live = std::fs::metadata(&path)
                .map(|m| {
                    m.is_file()
                        && m.modified()
                            .map(|t| now.duration_since(t).map(|age| age < ttl).unwrap_or(true))
                            .unwrap_or(true)
                })
                .unwrap_or(false);
            if live {
                None
            } else {
                Some(hash.clone())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atmos_sweeper_{}_{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_stale_upload_lock_keys_prunes_dead_entries() {
        let dir = test_dir("lockprune");
        let locks: dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>> = dashmap::DashMap::new();
        locks.insert("alive".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        locks.insert("dead".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        locks.insert("old".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        std::fs::write(dir.join(".upload_alive"), b"x").unwrap();
        std::fs::write(dir.join(".upload_old"), b"y").unwrap();
        std::fs::File::open(dir.join(".upload_old"))
            .unwrap()
            .set_modified(
                std::time::SystemTime::now()
                    .checked_sub(std::time::Duration::from_secs(25 * 60 * 60))
                    .unwrap(),
            )
            .unwrap();

        let stale = stale_upload_lock_keys(&locks, &dir, UPLOAD_TEMP_TTL);
        assert!(
            stale.contains(&"dead".to_string()),
            "临时文件缺失的条目应移除"
        );
        assert!(
            stale.contains(&"old".to_string()),
            "临时文件超龄的条目应移除"
        );
        assert!(
            !stale.contains(&"alive".to_string()),
            "临时文件新鲜的条目应保留"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_stale_upload_lock_keys_defensive_hash_format() {
        let dir = test_dir("lockprune2");
        let locks: dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>> = dashmap::DashMap::new();
        locks.insert("../evil".to_string(), Arc::new(tokio::sync::Mutex::new(())));
        locks.insert(
            "valid_hash_1".to_string(),
            Arc::new(tokio::sync::Mutex::new(())),
        );

        let stale = stale_upload_lock_keys(&locks, &dir, UPLOAD_TEMP_TTL);
        assert!(
            stale.contains(&"../evil".to_string()),
            "非 hash 格式的 key 应被防御性移除"
        );
        assert!(
            stale.contains(&"valid_hash_1".to_string()),
            "临时文件不存在时合法 key 也应收敛"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
