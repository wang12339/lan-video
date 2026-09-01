//! 上传相关的公共工具函数。
//!
//! 从 `admin_video.rs` 提取的流式上传、Content-Type 校验和临时文件
//! finalize 逻辑，便于在多个上传 handler 间复用。

use std::path::Path;

use axum::extract::multipart::Field;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::util::error::ServiceError;

/// 流式读取 multipart 字段并写入临时文件。
///
/// 从 `field` 中逐块读取数据，写入 `file_path`，累计字节数超过
/// `max_size` 时立即清理临时文件并返回错误。**边写边计算 SHA-256**，
/// 返回 `(总字节数, sha256 hex)`，避免落库阶段对 50GB 大文件二次全量读取。
/// 写入 0 字节视为"文件为空"错误。
///
/// # 参数
/// - `field`: multipart 中的文件字段
/// - `file_path`: 临时文件路径（调用方负责路径的唯一性）
/// - `max_size`: 最大允许字节数（硬上限）
///
/// # 错误
/// - `ServiceError::BadRequest` — 请求体读取失败、文件为空
/// - `ServiceError::Internal` — 临时文件创建/写入/flush 失败
/// - `ServiceError::PayloadTooLarge` — 文件超过 `max_size`（见下方注释）
///
/// > 注意：`ServiceError` 目前没有 `PayloadTooLarge` 变体，这里复用
/// > `BadRequest` 并附带"文件大小超过限制"的文案；如需独立状态码，
/// > 可后续在 `ServiceError` 中添加 413 变体。
pub async fn stream_multipart_to_file(
    mut field: Field<'_>,
    file_path: &Path,
    max_size: u64,
) -> Result<(u64, String), ServiceError> {
    let file = tokio::fs::File::create(file_path)
        .await
        .map_err(|e| ServiceError::Internal(format!("创建临时文件失败: {}", e)))?;
    // 缓冲写入：大文件减少系统调用，显著提升吞吐。
    let mut writer = tokio::io::BufWriter::with_capacity(1024 * 1024, file);

    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    loop {
        let chunk = field
            .chunk()
            .await
            .map_err(|e| ServiceError::BadRequest(format!("读取文件数据失败: {}", e)))?;
        match chunk {
            Some(data) => {
                total += data.len() as u64;
                if total > max_size {
                    // 超限：清理临时文件后返回错误
                    let _ = tokio::fs::remove_file(file_path).await;
                    return Err(ServiceError::BadRequest(format!(
                        "文件大小超过 {} 限制",
                        format_size(max_size),
                    )));
                }
                writer
                    .write_all(&data)
                    .await
                    .map_err(|e| ServiceError::Internal(format!("写入文件失败: {}", e)))?;
                hasher.update(&data);
            }
            None => break,
        }
    }

    // BufWriter 必须显式 flush（同时刷底层文件）后再 drop，否则数据可能丢失。
    writer
        .flush()
        .await
        .map_err(|e| ServiceError::Internal(format!("保存文件失败: {}", e)))?;
    drop(writer);

    if total == 0 {
        let _ = tokio::fs::remove_file(file_path).await;
        return Err(ServiceError::BadRequest("文件为空".into()));
    }

    Ok((total, format!("{:x}", hasher.finalize())))
}

/// 校验上传的 Content-Type 是否在允许列表中。
///
/// 将 `content_type` 的 MIME 部分（`;` 前，小写，trim）与 `expected`
/// 逐一比较。允许 `content_type` 包含额外参数（如
/// `video/mp4; charset=binary`）。
///
/// # 示例
/// ```ignore
/// validate_upload_content_type("video/mp4; charset=binary", &["video/mp4"])?;
/// ```
pub fn validate_upload_content_type(
    content_type: &str,
    expected: &[&str],
) -> Result<(), ServiceError> {
    // 提取 MIME 部分（去掉 `;` 后的参数），统一小写比较
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if expected.iter().any(|e| e.eq_ignore_ascii_case(&mime)) {
        Ok(())
    } else {
        Err(ServiceError::BadRequest(format!(
            "不支持的文件类型: {}",
            content_type,
        )))
    }
}

/// 将临时文件移动到最终路径。
///
/// 原子操作：先 `fsync` 临时文件确保持久化，再 `rename` 到 `final_path`。
/// 若 `final_path` 已存在则静默覆盖（`rename(2)` 语义）。
///
/// # 错误
/// - `ServiceError::Internal` — fsync 或 rename 失败
pub async fn finalize_upload(temp_path: &Path, final_path: &Path) -> Result<(), ServiceError> {
    // fsync 确保数据落盘后再 rename，避免崩溃后出现残缺文件
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(temp_path)
        .await
        .map_err(|e| ServiceError::Internal(format!("打开临时文件失败: {}", e)))?;

    file.sync_all()
        .await
        .map_err(|e| ServiceError::Internal(format!("同步临时文件失败: {}", e)))?;
    drop(file);

    tokio::fs::rename(temp_path, final_path)
        .await
        .map_err(|e| ServiceError::Internal(format!("移动文件失败: {}", e)))?;

    Ok(())
}

/// 将字节数格式化为人类可读的大小字符串。
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_content_type_exact_match() {
        assert!(validate_upload_content_type("video/mp4", &["video/mp4"]).is_ok());
    }

    #[test]
    fn test_validate_content_type_with_params() {
        assert!(validate_upload_content_type("video/mp4; charset=binary", &["video/mp4"]).is_ok());
    }

    #[test]
    fn test_validate_content_type_case_insensitive() {
        assert!(validate_upload_content_type("Video/MP4", &["video/mp4"]).is_ok());
    }

    #[test]
    fn test_validate_content_type_multiple_allowed() {
        assert!(validate_upload_content_type("image/jpeg", &["video/mp4", "image/jpeg"]).is_ok());
    }

    #[test]
    fn test_validate_content_type_rejected() {
        assert!(validate_upload_content_type("application/octet-stream", &["video/mp4"]).is_err());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(50 * 1024 * 1024 * 1024), "50.0 GB");
    }
}
