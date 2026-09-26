use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::AuthUser;
use crate::models::video::*;
use crate::services::media_service::is_safe_external_url;
use crate::services::media_service::upload::stream_multipart_to_file;
use crate::services::media_service::UploadAppendError;
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::hashid;
use crate::util::response::{
    error_response, error_response_code, error_response_data, internal_error_log, ErrorResponse,
    SafeJson,
};
use serde::Deserialize;

/// 上传进度接口响应禁止缓存（代理/CDN 不得缓存进度）。
fn no_store_headers() -> [(axum::http::HeaderName, &'static str); 1] {
    [(axum::http::header::CACHE_CONTROL, "no-store")]
}

/// 将分片追加错误映射为 HTTP 状态码 + 机器可读 code。
fn map_upload_append_error(e: UploadAppendError) -> (StatusCode, Json<ErrorResponse>) {
    match e {
        UploadAppendError::OffsetMismatch { received } => error_response_data(
            StatusCode::CONFLICT,
            "offset_mismatch",
            "上传偏移不一致，请从已接收位置继续",
            serde_json::json!({ "received": received }),
        ),
        UploadAppendError::HashMismatch => error_response_code(
            StatusCode::BAD_REQUEST,
            "hash_mismatch",
            "文件校验失败，请重新上传",
        ),
        UploadAppendError::Duplicate(_) => error_response_code(
            StatusCode::CONFLICT,
            "duplicate",
            "文件已存在，请勿重复上传",
        ),
        UploadAppendError::QuotaExceeded(_) => error_response_code(
            StatusCode::INSUFFICIENT_STORAGE,
            "quota_exceeded",
            "存储配额已用尽，请删除部分文件后重试",
        ),
        UploadAppendError::BadRequest(msg) => {
            error_response_code(StatusCode::BAD_REQUEST, "invalid_request", msg)
        }
        UploadAppendError::Internal(msg) => {
            tracing::warn!("upload_resume failed: {}", msg);
            error_response_code(StatusCode::INTERNAL_SERVER_ERROR, "internal", "上传失败")
        }
    }
}

/// 将 `ServiceError` 映射为 handler 错误元组（上传场景专用）。
///
/// 与通用 `ServiceError::into_tuple` 的区别：`BadRequest` 映射到
/// `PAYLOAD_TOO_LARGE`（当消息包含"超过"时）或保持 `BAD_REQUEST`；
/// `Internal` 映射到具体的上传失败文案。
fn map_upload_service_error(e: &ServiceError) -> (StatusCode, &'static str) {
    match e {
        ServiceError::BadRequest(msg) if msg.contains("超过") => {
            (StatusCode::PAYLOAD_TOO_LARGE, "文件大小超过限制")
        }
        ServiceError::BadRequest(_) => (StatusCode::BAD_REQUEST, "请求格式错误"),
        ServiceError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "上传失败"),
    }
}

#[utoipa::path(
    post,
    path = "/admin/videos/external",
    tag = "admin",
    summary = "Add an external video",
    description = "Register an external video URL (http/https). Title must be 1-500 characters.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Created — returns the new video ID", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn add_external_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(req): SafeJson<ExternalVideoRequest>,
) -> Result<(StatusCode, Json<IdResponse>), (StatusCode, Json<ErrorResponse>)> {
    if req.title.trim().is_empty() || req.title.len() > 500 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "标题长度需在 1-500 个字符之间",
        ));
    }
    if let Some(ref cat) = req.category {
        if cat.len() > 100 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "分类名称长度不能超过 100 个字符",
            ));
        }
    }
    if !is_safe_external_url(&req.stream_url) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "stream_url 指向不被允许的主机",
        ));
    }
    if let Some(ref cover) = req.cover_url {
        if !is_safe_external_url(cover) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "cover_url 指向不被允许的主机",
            ));
        }
    }

    let id = state
        .services
        .video
        .add_external_video(
            &req.title,
            req.description.as_deref(),
            req.category.as_deref(),
            &req.stream_url,
            req.cover_url.as_deref(),
            Some(auth_user.id),
        )
        .await
        .map_err(|e| {
            tracing::error!("add_external_video failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    state.invalidate_caches();
    tracing::info!(
        actor = %auth_user.username,
        video_id = id,
        stream_url = %req.stream_url,
        "admin added external video"
    );
    Ok((StatusCode::CREATED, Json(IdResponse { id })))
}

/// POST /admin/videos/upload
#[utoipa::path(
    post,
    path = "/admin/videos/upload",
    tag = "admin",
    description = "Upload a single video or image file via multipart form. Streamed to disk, SHA-256 checked, duplicates rejected. Available to any authenticated identity with role >= 1.",
    security(("bearerAuth" = [])),
    request_body(content_type = "multipart/form-data", description = "Multipart form with a single `file` field (50 GB max) and optional `category`"),
    responses(
        (status = 201, description = "Upload successful — returns the new video ID", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Duplicate file (code `duplicate`)"),
        (status = 413, description = "Payload too large"),
        (status = 500, description = "Internal server error"),
        (status = 507, description = "Per-user storage quota exhausted (code `quota_exceeded`)")
    )
)]
pub async fn upload_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<IdResponse>), (StatusCode, Json<ErrorResponse>)> {
    // 50GB max upload size
    const MAX_UPLOAD_SIZE: u64 = 50 * 1024 * 1024 * 1024;

    let mut file_name: Option<String> = None;
    let mut category = "local".to_string();
    let mut temp_path: Option<std::path::PathBuf> = None;
    let mut precomputed: Option<(i64, String)> = None;
    let mut file_field_seen = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "无效的请求格式"))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                // 本接口是单文件上传（批量请用前端上传队列，逐文件走
                // upload-resume 续传）。收到多个 file 字段时拒绝，避免
                // 静默覆盖只保留最后一个、丢弃已写入的临时文件。
                if file_field_seen {
                    if let Some(p) = &temp_path {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    return Err(error_response(
                        StatusCode::BAD_REQUEST,
                        "单次请求仅支持一个文件，请使用断点续传接口批量上传",
                    ));
                }
                file_field_seen = true;
                let raw_name = field.file_name().unwrap_or("video.mp4").to_string();
                // Sanitize: strip path separators and control characters, keep only the filename
                let fname = crate::services::media_service::sanitize_filename(&raw_name);
                file_name = Some(fname);

                // 流式写入临时文件（复用公共函数，边写边算 SHA-256）
                let tmp = state
                    .config
                    .media_root
                    .join(format!(".upload_{}", Uuid::new_v4()));
                let (size, sha256) = stream_multipart_to_file(field, &tmp, MAX_UPLOAD_SIZE)
                    .await
                    .map_err(|e| {
                        let (status, body) = map_upload_service_error(&e);
                        error_response(status, body)
                    })?;
                temp_path = Some(tmp);
                precomputed = Some((size as i64, sha256));
            }
            "category" => {
                // category 不是文件字段，手动读取文本
                let text = field.text().await.unwrap_or_default();
                category = text;
            }
            _ => {}
        }
    }

    let file_name = file_name.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "缺少文件"))?;
    let tmp_path = temp_path.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "缺少文件"))?;

    let id = match state
        .services
        .media
        .upload_video_file(&file_name, &tmp_path, &category, auth_user.id, precomputed)
        .await
    {
        Ok(id) => id,
        Err(e)
            if matches!(
                e,
                ServiceError::Duplicate(_) | ServiceError::QuotaExceeded(_)
            ) =>
        {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            tracing::warn!(actor = %auth_user.username, "upload conflict");
            // 重复与配额超限分开 code：前端 worker 据此判定去重成功/配额错误
            if let ServiceError::QuotaExceeded(_) = e {
                return Err(error_response_code(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "quota_exceeded",
                    "存储配额已用尽，请删除部分文件后重试",
                ));
            }
            return Err(error_response_code(
                StatusCode::CONFLICT,
                "duplicate",
                "文件已存在，请勿重复上传",
            ));
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            tracing::warn!(actor = %auth_user.username, "upload failed: {}", e);
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "上传失败",
            ));
        }
    };

    state.invalidate_caches();
    state.metrics.record_video_upload();
    tracing::info!(
        actor = %auth_user.username,
        video_id = id,
        "admin uploaded video"
    );
    Ok((StatusCode::CREATED, Json(IdResponse { id })))
}

#[derive(serde::Deserialize)]
pub struct UploadStatusQuery {
    pub hash: String,
    /// 文件总大小（可选）。提供时在真正传输前做配额预检，超限直接 507。
    pub size: Option<i64>,
}

fn is_valid_upload_hash(s: &str) -> bool {
    s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// GET /admin/videos/upload-status?hash=xxx&size=yyy
///
/// 客户端在开始传输前调用：命中同上传者已有内容时返回 `exists: true`
/// （可零传输跳过），带 `size` 时同时做配额预检。
#[utoipa::path(
    get,
    path = "/admin/videos/upload-status",
    tag = "admin",
    description = "Pre-flight check before uploading: duplicate detection and optional quota check. Response is never cached (`Cache-Control: no-store`).",
    security(("bearerAuth" = [])),
    params(
        ("hash" = String, Query, description = "Upload key — SHA-256 hex of the complete file"),
        ("size" = Option<i64>, Query, description = "Total file size in bytes (optional; enables the quota pre-check)")
    ),
    responses(
        (status = 200, description = "Upload pre-flight result", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
        (status = 507, description = "Per-user storage quota exhausted (code `quota_exceeded`)")
    )
)]
pub async fn upload_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Query(q): Query<UploadStatusQuery>,
) -> Result<
    (
        [(axum::http::HeaderName, &'static str); 1],
        Json<serde_json::Value>,
    ),
    (StatusCode, Json<ErrorResponse>),
> {
    if !is_valid_upload_hash(&q.hash) {
        return Err(error_response(StatusCode::BAD_REQUEST, "invalid hash"));
    }

    // 去重预检：命中则无需传输任何字节。查询失败不阻断上传，
    // finalize 时仍会做权威去重。
    match state
        .services
        .media
        .find_upload_duplicate(auth_user.id, &q.hash)
        .await
    {
        Ok(Some(id)) => {
            return Ok((
                no_store_headers(),
                Json(serde_json::json!({
                    "received": 0,
                    "exists": true,
                    "existing_id": id,
                })),
            ));
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("upload-status dedup check failed: {}", e),
    }

    // 配额预检：超限在传输第一个字节前返回 507。
    if let Some(size) = q.size.filter(|s| *s > 0) {
        if let Err(e) = state
            .services
            .media
            .ensure_upload_quota(auth_user.id, size)
            .await
        {
            if let ServiceError::QuotaExceeded(_) = e {
                return Err(error_response_code(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "quota_exceeded",
                    "存储配额已用尽，请删除部分文件后重试",
                ));
            }
            tracing::warn!("upload-status quota check failed: {}", e);
        }
    }

    let received = state
        .services
        .media
        .upload_received_bytes(auth_user.id, &q.hash)
        .await
        .map_err(|e| {
            tracing::warn!("upload-status progress failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询上传进度失败")
        })?;

    Ok((
        no_store_headers(),
        Json(serde_json::json!({
            "received": received,
            "exists": false,
        })),
    ))
}

/// POST /admin/videos/upload-resume
///
/// 幂等分片追加：`x-upload-offset` 必须等于服务端已接收字节数，
/// 否则返回 409 `offset_mismatch`（body.data.received 为服务端偏移），
/// 客户端据此回退重切分片。最后一个分片触发 finalize 并做哈希校验。
#[utoipa::path(
    post,
    path = "/admin/videos/upload-resume",
    tag = "admin",
    description = "Append a chunk to a partial upload identified by `x-upload-hash`. `x-upload-offset` must equal the bytes already received (409 `offset_mismatch` otherwise). Finalization verifies the incremental SHA-256 and returns 201 with the new video ID. Body capped at 32 MB; available to any authenticated identity with role >= 1.",
    security(("bearerAuth" = [])),
    params(
        ("x-upload-hash" = String, Header, description = "SHA-256 hex of the complete file (upload key, verified at finalization)"),
        ("x-upload-offset" = Option<i64>, Header, description = "Byte offset at which this chunk starts (must equal server-received bytes)"),
        ("x-upload-name" = Option<String>, Header, description = "Original filename"),
        ("x-upload-size" = i64, Header, description = "Total expected file size in bytes (max 50 GB)"),
        ("x-upload-category" = Option<String>, Header, description = "Category for the uploaded file")
    ),
    request_body(content_type = "application/octet-stream", description = "Raw chunk bytes; empty body performs a progress query"),
    responses(
        (status = 200, description = "Progress query (empty body) — returns bytes received", body = serde_json::Value),
        (status = 201, description = "Upload finalized (idempotent for a replayed final chunk)", body = serde_json::Value),
        (status = 206, description = "Partial content — more data needed", body = serde_json::Value),
        (status = 400, description = "Invalid headers/offset, or final hash mismatch (code `hash_mismatch`)"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Offset mismatch (code `offset_mismatch`) or duplicate content (code `duplicate`)"),
        (status = 413, description = "Payload too large"),
        (status = 500, description = "Internal server error"),
        (status = 507, description = "Per-user storage quota exhausted (code `quota_exceeded`)")
    )
)]
pub async fn upload_resume(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    const MAX_UPLOAD_SIZE: i64 = 50 * 1024 * 1024 * 1024; // 50GB

    let hash = headers
        .get("x-upload-hash")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "缺少 x-upload-hash"))?
        .to_string();
    if !is_valid_upload_hash(&hash) {
        return Err(error_response(StatusCode::BAD_REQUEST, "invalid hash"));
    }
    let raw_name = headers
        .get("x-upload-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("video.mp4")
        .to_string();
    let file_name = crate::services::media_service::sanitize_filename(&raw_name);
    let total_size = headers
        .get("x-upload-size")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let category = headers
        .get("x-upload-category")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local")
        .to_string();

    if total_size <= 0 || total_size > MAX_UPLOAD_SIZE {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "x-upload-size 无效或超过 50GB 限制",
        ));
    }

    // 新协议：客户端声明本分片起始偏移；缺省时按旧协议在文件末尾追加
    // （无幂等性，仅为兼容尚未更新的客户端）。
    let offset = match headers.get("x-upload-offset") {
        Some(v) => {
            let raw = v
                .to_str()
                .map_err(|_| error_response(StatusCode::BAD_REQUEST, "x-upload-offset 无效"))?;
            let n = raw
                .parse::<i64>()
                .map_err(|_| error_response(StatusCode::BAD_REQUEST, "x-upload-offset 无效"))?;
            if n < 0 || n > total_size {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "x-upload-offset 超出范围",
                ));
            }
            Some(n)
        }
        None => None,
    };
    if let Some(start) = offset {
        if start + body.len() as i64 > total_size {
            return Err(error_response_code(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "分片超出声明的文件总大小",
            ));
        }
    }

    let outcome = state
        .services
        .media
        .append_upload_chunk(
            auth_user.id,
            &hash,
            &file_name,
            total_size,
            &category,
            offset,
            &body,
        )
        .await
        .map_err(map_upload_append_error)?;

    if let Some(id) = outcome.video_id {
        state.invalidate_caches();
        state.metrics.record_video_upload();
        tracing::info!(
            actor = %auth_user.username,
            video_id = id,
            "admin uploaded video (resumed)"
        );
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id, "received": outcome.received })),
        ));
    }

    if body.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "received": outcome.received })),
        ));
    }

    Ok((
        StatusCode::PARTIAL_CONTENT,
        Json(serde_json::json!({ "received": outcome.received })),
    ))
}

/// POST /admin/videos/check-hashes
#[utoipa::path(
    post,
    path = "/admin/videos/check-hashes",
    tag = "admin",
    description = "Given a list of file hashes, return which ones already exist in the database. Max 1000 hashes per request.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "List of existing hashes", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn check_hashes(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<CheckHashesRequest>,
) -> Result<Json<CheckHashesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.hashes.len() > 1000 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最多检查 1000 个文件",
        ));
    }
    let existing = state
        .services
        .video
        .check_existing_hashes(req.hashes)
        .await
        .map_err(|e| internal_error_log("check_existing_hashes", &e))?;
    Ok(Json(CheckHashesResponse { existing }))
}

/// POST /admin/videos/check-files
#[utoipa::path(
    post,
    path = "/admin/videos/check-files",
    tag = "admin",
    description = "Given a list of (name, size) pairs, return which indices already exist in the database. Max 1000 files per request.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Indices of existing files", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn check_files(
    State(state): State<Arc<AppState>>,
    SafeJson(files): SafeJson<Vec<FileCheckItem>>,
) -> Result<Json<CheckFilesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if files.len() > 1000 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最多检查 1000 个文件",
        ));
    }
    let existing_indices = state
        .services
        .video
        .check_existing_files(&files)
        .await
        .map_err(|e| internal_error_log("check_existing_files", &e))?;
    Ok(Json(CheckFilesResponse {
        existing_indices: existing_indices.into_iter().collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/admin/videos/scan",
    tag = "admin",
    summary = "Scan media directory",
    description = "Scan the configured media directory for video and image files not yet in the database. New files are added with hashes and metadata.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body(content_type = "multipart/form-data", description = "Optional multipart form with a `category` field"),
    responses(
        (status = 200, description = "Scan result", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn scan_media(
    State(state): State<Arc<AppState>>,
    multipart: Option<Multipart>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let category = if let Some(mut mp) = multipart {
        let mut cat = "local".to_string();
        while let Ok(Some(field)) = mp.next_field().await {
            if field.name() == Some("category") {
                cat = field.text().await.unwrap_or_else(|_| "local".to_string());
            }
        }
        cat
    } else {
        "local".to_string()
    };

    tracing::info!(category = %category, "admin started media scan");
    let added = state
        .services
        .video
        .scan_media_directory(&category)
        .await
        .map_err(|e| internal_error_log("scan_media_directory", &e))?;

    tracing::info!(category = %category, added = added, "admin media scan complete");
    state.invalidate_caches();
    Ok(Json(serde_json::json!({"added": added})))
}

/// PUT /admin/videos/{id}
#[utoipa::path(
    put,
    path = "/admin/videos/{id}",
    tag = "admin",
    description = "Update title, description, and/or category for a video",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "Video ID")
    ),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Update result", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<VideoUpdateRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(ref title) = req.title {
        if title.trim().is_empty() || title.len() > 500 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "标题长度需在 1-500 个字符之间",
            ));
        }
    }
    if let Some(ref category) = req.category {
        if category.len() > 100 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "分类名称长度不能超过 100 个字符",
            ));
        }
    }

    let ok = state
        .services
        .video
        .update_video(
            id,
            req.title.as_deref(),
            req.description.as_deref(),
            req.category.as_deref(),
        )
        .await
        .map_err(|e| internal_error_log("update_video failed", &e))?;

    state.invalidate_caches();
    if ok {
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Ok(Json(OkResponse {
            ok: false,
            error: Some("视频不存在".into()),
            deleted: None,
        }))
    }
}

/// DELETE /admin/videos/{id}
#[utoipa::path(
    delete,
    path = "/admin/videos/{id}",
    tag = "admin",
    description = "Delete a video and its associated physical files, playback history, likes, and favorites",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = String, Path, description = "视频 ID（数字或 hashid）")
    ),
    responses(
        (status = 200, description = "Delete result", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let Some(id) = hashid::decode_id_or_numeric(&id) else {
        return Err(error_response(StatusCode::BAD_REQUEST, "无效的视频ID"));
    };
    state.invalidate_caches();
    match state.services.video.delete_video(id).await {
        Ok(true) => {
            state.metrics.record_video_delete();
            tracing::info!(video_id = id, "admin deleted video");
            Ok(Json(OkResponse {
                ok: true,
                error: None,
                deleted: None,
            }))
        }
        Ok(false) => Ok(Json(OkResponse {
            ok: false,
            error: Some("视频不存在".into()),
            deleted: None,
        })),
        Err(e) => {
            tracing::error!("delete_video failed for id={}: {}", id, e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "服务器内部错误",
            ))
        }
    }
}

/// DELETE /admin/videos/batch
#[utoipa::path(
    delete,
    path = "/admin/videos/batch",
    tag = "admin",
    description = "Delete multiple videos and their associated files, playback history, likes, and favorites in a single transaction. Max 500 IDs per request.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Delete result", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_videos(
    State(state): State<Arc<AppState>>,
    SafeJson(ids): SafeJson<Vec<String>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    if ids.len() > 500 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最多批量删除 500 个",
        ));
    }
    let numeric_ids: Vec<i64> = ids
        .iter()
        .filter_map(|id| hashid::decode_id_or_numeric(id))
        .collect();
    state.invalidate_caches();
    let deleted = state
        .services
        .video
        .delete_videos(&numeric_ids)
        .await
        .map_err(|e| {
            tracing::error!("delete_videos failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "批量删除视频失败")
        })?;
    tracing::info!(
        count = numeric_ids.len(),
        deleted = deleted,
        "admin batch deleted videos"
    );

    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: Some(deleted as i64),
    }))
}

/// POST /admin/videos/{id}/cover
#[utoipa::path(
    post,
    path = "/admin/videos/{id}/cover",
    tag = "admin",
    description = "Upload a cover image for a specific video via multipart form",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "Video ID")
    ),
    request_body(content_type = "multipart/form-data", description = "Multipart form with a single `file` field (cover image)"),
    responses(
        (status = 204, description = "Cover uploaded successfully"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn upload_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let file_name = field.file_name().unwrap_or("cover.jpg").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|_| error_response(StatusCode::BAD_REQUEST, "读取封面数据失败"))?;

            state
                .services
                .media
                .update_cover(id, &file_name, data)
                .await
                .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "封面更新失败"))?;

            state.invalidate_caches();
            return Ok(StatusCode::NO_CONTENT);
        }
    }

    Err(error_response(StatusCode::BAD_REQUEST, "缺少文件"))
}

/// POST /admin/videos/backfill-thumbnails
#[utoipa::path(
    post,
    path = "/admin/videos/backfill-thumbnails",
    tag = "admin",
    description = "Scan all local videos without covers and generate thumbnails using ffmpeg. Runs in batches to avoid memory spikes.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "Backfill result", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn backfill_thumbnails(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.services.media.backfill_thumbnails().await {
        Ok((generated, errors)) => {
            state.invalidate_caches();
            Json(serde_json::json!({"ok": true, "generated": generated, "errors": errors}))
        }
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

/// POST /admin/videos/backfill-exif
#[utoipa::path(
    post,
    path = "/admin/videos/backfill-exif",
    tag = "admin",
    description = "Scan local images whose EXIF metadata has not been extracted yet, parse the original image files and backfill the EXIF columns. Runs in batches to avoid memory spikes.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "Backfill result", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn backfill_exif(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.services.media.backfill_image_exif().await {
        Ok((processed, errors)) => {
            state.invalidate_caches();
            Json(serde_json::json!({"ok": true, "processed": processed, "errors": errors}))
        }
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

/// PUT /admin/videos/batch-category — 批量修改分类
#[derive(Deserialize)]
pub struct BatchCategoryRequest {
    pub ids: Vec<i64>,
    pub category: String,
}

#[utoipa::path(
    put,
    path = "/admin/videos/batch-category",
    tag = "admin",
    summary = "Batch update video categories",
    description = "批量修改视频分类（最多 1000 个，分类名最多 100 字符）",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Categories updated", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn batch_update_category(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<BatchCategoryRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.ids.len() > 1000 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最多批量修改 1000 个",
        ));
    }
    if req.category.len() > 100 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "分类名称长度不能超过 100 个字符",
        ));
    }
    let updated = state
        .repos
        .video
        .batch_update_category(&req.ids, &req.category)
        .await
        .map_err(|e| internal_error_log("batch_update_category", &e))?;
    state.invalidate_caches();
    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: Some(updated),
    }))
}
