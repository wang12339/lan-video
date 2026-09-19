#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! 分片续传会话（media_service::session）离线单元测试。
//!
//! 覆盖不依赖 PostgreSQL 的路径：
//!   - offset 校验与 409 语义（OffsetMismatch 携带服务端已接收字节数）
//!   - finalize 时的哈希校验（HashMismatch 并清理临时文件）
//!   - 服务重启后从磁盘临时文件重建增量哈希状态
//!   - 进度查询与槽位清理
//!
//! 运行：`cargo test --test test_upload_session`（无需数据库）。
//! 用例中的 DB 访问仅在哈希校验通过后才发生（会因惰性连接池失败返回
//! Internal，用于证明"哈希已通过"），因此不会真正写入数据库。

mod integration_test_helpers;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use atmos_video_backend::repositories::video_repo::VideoRepository;
use atmos_video_backend::services::media_service::{MediaService, UploadAppendError};
use integration_test_helpers::test_config;
use sha2::{Digest, Sha256};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir(prefix: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}_{}_n{}", prefix, std::process::id(), n))
}

/// 构造不连库的 MediaService：惰性连接池，仅哈希校验失败路径不触库。
fn test_service(media_root: &Path) -> MediaService {
    let mut config = test_config();
    config.media_root = media_root.to_path_buf();
    config.upload_quota_bytes = 0;
    let pool = sqlx::PgPool::connect_lazy("postgres://localhost/atmos_upload_session_test")
        .expect("惰性连接池构造失败");
    MediaService::new(VideoRepository::new(pool), config)
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

#[tokio::test]
async fn offset_mismatch_reports_server_offset() {
    let dir = unique_dir("atmos_upload_offset");
    std::fs::create_dir_all(&dir).unwrap();
    let svc = test_service(&dir);

    // 服务端为空，客户端却从 offset=5 发起 → 协议错误，给出 received=0
    let err = svc
        .append_upload_chunk(1, "hashoffset", "a.mp4", 100, "local", Some(5), b"hello")
        .await
        .unwrap_err();
    match err {
        UploadAppendError::OffsetMismatch { received } => assert_eq!(received, 0),
        other => panic!("应返回 OffsetMismatch，实际: {:?}", other),
    }

    // 先正确追加 5 字节，再以过期 offset 重发 → 返回当前偏移
    let out = svc
        .append_upload_chunk(1, "hashoffset", "a.mp4", 100, "local", Some(0), b"hello")
        .await
        .unwrap();
    assert_eq!(out.received, 5);
    let err = svc
        .append_upload_chunk(1, "hashoffset", "a.mp4", 100, "local", Some(0), b"hello")
        .await
        .unwrap_err();
    match err {
        UploadAppendError::OffsetMismatch { received } => assert_eq!(received, 5),
        other => panic!("应返回 OffsetMismatch，实际: {:?}", other),
    }

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn finalize_hash_mismatch_rejected_and_temp_cleaned() {
    let dir = unique_dir("atmos_upload_hash");
    std::fs::create_dir_all(&dir).unwrap();
    let svc = test_service(&dir);

    let part1 = vec![7u8; 512];
    let part2 = vec![9u8; 512];
    let out = svc
        .append_upload_chunk(2, "hashmismatch", "a.mp4", 1024, "local", Some(0), &part1)
        .await
        .unwrap();
    assert_eq!(out.received, 512);
    assert!(out.video_id.is_none());

    // 最后一个分片：客户端声明的哈希（key）与实际内容不符 → 400 hash_mismatch
    let err = svc
        .append_upload_chunk(2, "hashmismatch", "a.mp4", 1024, "local", Some(512), &part2)
        .await
        .unwrap_err();
    assert!(matches!(err, UploadAppendError::HashMismatch));
    assert!(
        !svc.upload_temp_path(2, "hashmismatch").exists(),
        "哈希校验失败后临时文件应被清理"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn resume_rebuilds_hasher_from_disk_after_restart() {
    let dir = unique_dir("atmos_upload_resume");
    std::fs::create_dir_all(&dir).unwrap();

    let part1 = vec![1u8; 700];
    let part2 = vec![2u8; 324];
    let declared = {
        let mut full = part1.clone();
        full.extend_from_slice(&part2);
        sha256_hex(&full)
    };

    // 第一个服务实例：只传前半段（key 即客户端声明的完整文件哈希）
    {
        let svc = test_service(&dir);
        let out = svc
            .append_upload_chunk(3, &declared, "a.mp4", 1024, "local", Some(0), &part1)
            .await
            .unwrap();
        assert_eq!(out.received, 700);
    }

    // 模拟进程重启：新实例对同一临时文件重建哈希状态
    let svc = test_service(&dir);
    assert_eq!(svc.upload_received_bytes(3, &declared).await.unwrap(), 700);

    // 续传最后一个分片；哈希校验通过后触库（惰性池连接失败 → Internal）
    let err = svc
        .append_upload_chunk(3, &declared, "a.mp4", 1024, "local", Some(700), &part2)
        .await
        .unwrap_err();
    assert!(
        matches!(err, UploadAppendError::Internal(_)),
        "正确哈希应通过校验并在 DB 阶段失败，实际: {:?}",
        err
    );

    // 完整临时文件 + 空 body finalize：声明错误哈希必须被拒绝
    // （模拟"临时文件已完整但 finalize 前进程重启"的恢复路径）
    let part3 = vec![3u8; 1024];
    let tmp3 = svc.upload_temp_path(4, "restart2");
    std::fs::write(&tmp3, &part3).unwrap();
    let err = svc
        .append_upload_chunk(4, "restart2", "b.mp4", 1024, "local", Some(1024), b"")
        .await
        .unwrap_err();
    assert!(
        matches!(err, UploadAppendError::HashMismatch),
        "空 body finalize 触发时错误哈希应被拒绝，实际: {:?}",
        err
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn legacy_append_without_offset_is_supported() {
    let dir = unique_dir("atmos_upload_legacy");
    std::fs::create_dir_all(&dir).unwrap();
    let svc = test_service(&dir);

    // 旧客户端不带 x-upload-offset：按文件末尾追加
    let out = svc
        .append_upload_chunk(5, "legacy", "a.mp4", 10, "local", None, b"01234")
        .await
        .unwrap();
    assert_eq!(out.received, 5);
    let err = svc
        .append_upload_chunk(5, "legacy", "a.mp4", 10, "local", None, b"56789")
        .await
        .unwrap_err();
    // 最后一片哈希不符（声明是 "legacy" 的临时名而非真实哈希）→ HashMismatch
    assert!(matches!(err, UploadAppendError::HashMismatch));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn progress_query_from_disk_and_cleanup_keeps_active_slot() {
    let dir = unique_dir("atmos_upload_progress");
    std::fs::create_dir_all(&dir).unwrap();
    let svc = test_service(&dir);

    // 手工放入一个 3 字节临时文件（模拟中断的上传），进度查询应能读到
    let tmp = svc.upload_temp_path(6, "progress");
    std::fs::write(&tmp, b"abc").unwrap();

    assert_eq!(
        svc.append_upload_chunk(6, "progress", "a.mp4", 10, "local", Some(0), b"")
            .await
            .unwrap()
            .received,
        3
    );

    // 未过期槽位不应被清理
    svc.cleanup_upload_slots();
    assert_eq!(svc.upload_received_bytes(6, "progress").await.unwrap(), 3);

    std::fs::remove_dir_all(&dir).unwrap();
}
