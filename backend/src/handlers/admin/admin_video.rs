use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::middleware::auth::AuthUser;
use crate::models::video::*;
use crate::services::media_service::is_safe_external_url;
use crate::services::media_service::upload::stream_multipart_to_file;
use crate::state::AppState;
use crate::util::hashid;
use crate::util::error::ServiceError;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};
use serde::Deserialize;

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

/// POST /admin/videos/external
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
    // SECURITY (A04 M4): reject external URLs pointing at loopback, link-local,
    // or RFC1918 addresses. The browser still fetches the URL, but blocking
    // these on input stops an admin from accidentally registering an
    // internal-network address that every viewer would then resolve.
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
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;

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

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "无效的请求格式"))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let raw_name = field.file_name().unwrap_or("video.mp4").to_string();
                // Sanitize: strip path separators and control characters, keep only the filename
                let fname = crate::services::media_service::sanitize_filename(&raw_name);
                file_name = Some(fname);

                // 流式写入临时文件（复用公共函数）
                let tmp = state
                    .config
                    .media_root
                    .join(format!(".upload_{}", Uuid::new_v4()));
                stream_multipart_to_file(field, &tmp, MAX_UPLOAD_SIZE)
                    .await
                    .map_err(|e| {
                        let (status, body) = map_upload_service_error(&e);
                        error_response(status, body)
                    })?;
                temp_path = Some(tmp);
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
        .upload_video_file(&file_name, &tmp_path, &category, auth_user.id)
        .await
    {
        Ok(id) => id,
        Err(ServiceError::Duplicate(_) | ServiceError::QuotaExceeded(_)) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            tracing::warn!(actor = %auth_user.username, "upload conflict");
            return Err(error_response(StatusCode::CONFLICT, "文件重复或存储配额已用尽"));
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            tracing::warn!(actor = %auth_user.username, "upload failed: {}", e);
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, "上传失败"));
        }
    };

    state.invalidate_caches();
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
}

fn is_valid_upload_hash(s: &str) -> bool {
    s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// GET /admin/videos/upload-status?hash=xxx
pub async fn upload_status(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UploadStatusQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_upload_hash(&q.hash) {
        return Err(error_response(StatusCode::BAD_REQUEST, "invalid hash"));
    }
    let tmp = state.config.media_root.join(format!(".upload_{}", q.hash));
    let received = match tokio::fs::metadata(&tmp).await {
        Ok(m) => m.len() as i64,
        Err(_) => 0,
    };
    Ok(Json(serde_json::json!({ "received": received })))
}

/// POST /admin/videos/upload-resume
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

    let tmp = state.config.media_root.join(format!(".upload_{}", hash));

    // Empty body = check progress (no lock needed for read-only check)
    if body.is_empty() {
        let received = match tokio::fs::metadata(&tmp).await {
            Ok(m) => m.len() as i64,
            Err(_) => 0,
        };
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "received": received })),
        ));
    }

    // Per-hash mutex to prevent concurrent writes from corrupting the temp file
    let lock = state
        .upload_locks
        .entry(hash.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .value()
        .clone();
    let _guard = lock.lock().await;

    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "打开临时文件失败"))?;
    f.write_all(&body)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入失败"))?;
    f.flush()
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "flush失败"))?;
    drop(f);

    let received = tokio::fs::metadata(&tmp)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    if received >= total_size {
        let tmp_clone = tmp.clone();
        let id = state
            .services
            .media
            .upload_video_file(&file_name, &tmp_clone, &category, auth_user.id)
            .await
            .map_err(|e| {
                let tmp_for_cleanup = tmp.clone();
                tokio::spawn(async move {
                    let _ = tokio::fs::remove_file(&tmp_for_cleanup).await;
                });
                match &e {
                    ServiceError::Duplicate(_) | ServiceError::QuotaExceeded(_) => {
                        error_response(StatusCode::CONFLICT, "文件重复或存储配额已用尽")
                    }
                    _ => {
                        tracing::warn!("upload_resume failed: {}", e);
                        error_response(StatusCode::INTERNAL_SERVER_ERROR, "上传失败")
                    }
                }
            })?;
        state.upload_locks.remove(&hash);
        state.invalidate_caches();
        tracing::info!(
            actor = %auth_user.username,
            video_id = id,
            "admin uploaded video (resumed)"
        );
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id, "received": received })),
        ));
    }

    Ok((
        StatusCode::PARTIAL_CONTENT,
        Json(serde_json::json!({ "received": received })),
    ))
}

/// POST /admin/videos/check-hashes
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
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(Json(CheckHashesResponse { existing }))
}

/// POST /admin/videos/check-files
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
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(Json(CheckFilesResponse {
        existing_indices: existing_indices.into_iter().collect(),
    }))
}

/// POST /admin/videos/scan
pub async fn scan_media(
    State(state): State<Arc<AppState>>,
    multipart: Option<Multipart>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let category = if let Some(mut mp) = multipart {
        let mut cat = "local".to_string();
        while let Ok(Some(field)) = mp.next_field().await {
            if field.name() == Some("category") {
                cat = field.text().await.unwrap_or("local".to_string());
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
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "扫描失败"))?;

    tracing::info!(category = %category, added = added, "admin media scan complete");
    state.invalidate_caches();
    Ok(Json(serde_json::json!({"added": added})))
}

/// PUT /admin/videos/{id}
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
pub async fn backfill_thumbnails(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.services.media.backfill_thumbnails().await {
        Ok((generated, errors)) => {
            state.invalidate_caches();
            Json(serde_json::json!({"ok": true, "generated": generated, "errors": errors}))
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
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    state.invalidate_caches();
    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: Some(updated),
    }))
}
