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
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse, SafeJson};
use serde::Deserialize;

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
        .video_service
        .add_external_video(
            &req.title,
            req.description.as_deref(),
            req.category.as_deref(),
            &req.stream_url,
            req.cover_url.as_deref(),
            Some(auth_user.id),
        )
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    state.video_cache.invalidate_all();
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
    let mut client_hash: Option<String> = None;
    let mut temp_path: Option<std::path::PathBuf> = None;

    while let Some(mut field) = multipart
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

                // 流式写入临时文件
                let tmp = state
                    .config
                    .media_root
                    .join(format!(".upload_{}", Uuid::new_v4()));
                let mut f = tokio::fs::File::create(&tmp).await.map_err(|_| {
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "创建临时文件失败")
                })?;
                let mut total: u64 = 0;
                loop {
                    let chunk = field
                        .chunk()
                        .await
                        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "读取文件数据失败"))?;
                    match chunk {
                        Some(data) => {
                            total += data.len() as u64;
                            if total > MAX_UPLOAD_SIZE {
                                let _ = tokio::fs::remove_file(&tmp).await;
                                return Err(error_response(
                                    StatusCode::PAYLOAD_TOO_LARGE,
                                    "文件大小超过 50GB 限制",
                                ));
                            }
                            f.write_all(&data).await.map_err(|_| {
                                error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入文件失败")
                            })?;
                        }
                        None => break,
                    }
                }
                f.flush().await.map_err(|_| {
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "保存文件失败")
                })?;
                drop(f);

                if total == 0 {
                    let _ = tokio::fs::remove_file(&tmp).await;
                    return Err(error_response(StatusCode::BAD_REQUEST, "文件为空"));
                }
                temp_path = Some(tmp);
            }
            "category" => {
                category = field.text().await.unwrap_or_default();
            }
            "fileHash" => {
                client_hash = Some(field.text().await.unwrap_or_default());
            }
            _ => {}
        }
    }

    let file_name = file_name.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "缺少文件"))?;
    let tmp_path = temp_path.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "缺少文件"))?;

    let id = match state
        .media_service
        .upload_video_file(
            &file_name,
            &tmp_path,
            &category,
            client_hash.as_deref(),
            auth_user.id,
        )
        .await
    {
        Ok(id) => id,
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            tracing::warn!(actor = %auth_user.username, "upload failed: {}", e);
            return Err(if e.starts_with("重复") || e.starts_with("存储配额") {
                error_response(StatusCode::CONFLICT, e)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "上传失败")
            });
        }
    };

    state.video_cache.invalidate_all();
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
            .media_service
            .upload_video_file(&file_name, &tmp_clone, &category, Some(&hash), auth_user.id)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                if e.starts_with("重复") || e.starts_with("存储配额") {
                    error_response(StatusCode::CONFLICT, e)
                } else {
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "上传失败")
                }
            })?;
        state.upload_locks.remove(&hash);
        state.video_cache.invalidate_all();
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
        .video_service
        .check_existing_hashes(req.hashes)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
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
        .video_service
        .check_existing_files(&files)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
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
        .video_service
        .scan_media_directory(&category)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "扫描失败"))?;

    tracing::info!(category = %category, added = added, "admin media scan complete");
    state.video_cache.invalidate_all();
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
        .video_service
        .update_video(
            id,
            req.title.as_deref(),
            req.description.as_deref(),
            req.category.as_deref(),
        )
        .await
        .map_err(|e| {
            tracing::error!("update_video failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    state.video_cache.invalidate_all();
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
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    state.video_cache.invalidate_all();
    match state.video_service.delete_video(id).await {
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
    SafeJson(ids): SafeJson<Vec<i64>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    if ids.len() > 500 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最多批量删除 500 个",
        ));
    }
    state.video_cache.invalidate_all();
    let deleted = state
        .video_service
        .delete_videos(&ids)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("delete_videos failed: {}", e);
            0
        });
    tracing::info!(
        count = ids.len(),
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
                .media_service
                .update_cover(id, &file_name, data)
                .await
                .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "封面更新失败"))?;

            state.video_cache.invalidate_all();
            return Ok(StatusCode::NO_CONTENT);
        }
    }

    Err(error_response(StatusCode::BAD_REQUEST, "缺少文件"))
}

/// POST /admin/videos/backfill-thumbnails
pub async fn backfill_thumbnails(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.media_service.backfill_thumbnails().await {
        Ok((generated, errors)) => {
            state.video_cache.invalidate_all();
            Json(serde_json::json!({"ok": true, "generated": generated, "errors": errors}))
        }
        Err(e) => Json(serde_json::json!({"ok": false, "error": e})),
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
        .video_repo
        .batch_update_category(&req.ids, &req.category)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    state.video_cache.invalidate_all();
    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: Some(updated),
    }))
}
