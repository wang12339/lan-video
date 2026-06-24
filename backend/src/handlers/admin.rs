use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::state::AppState;
use crate::models::video::*;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

/// POST /admin/videos/external
pub async fn add_external_video(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<ExternalVideoRequest>,
) -> Result<(StatusCode, Json<IdResponse>), (StatusCode, Json<ErrorResponse>)> {
    if req.title.trim().is_empty() || req.title.len() > 500 {
        return Err(error_response(StatusCode::BAD_REQUEST, "标题长度需在 1-500 个字符之间"));
    }
    if !req.stream_url.starts_with("http://") && !req.stream_url.starts_with("https://") {
        return Err(error_response(StatusCode::BAD_REQUEST, "stream_url must start with http:// or https://"));
    }
    if let Some(ref cover) = req.cover_url {
        if !cover.starts_with("http://") && !cover.starts_with("https://") {
            return Err(error_response(StatusCode::BAD_REQUEST, "cover_url must start with http:// or https://"));
        }
    }

    let id = state.video_service
        .add_external_video(&req.title, req.description.as_deref(), req.category.as_deref(), &req.stream_url, req.cover_url.as_deref())
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    state.video_cache.invalidate_all();
    Ok((StatusCode::CREATED, Json(IdResponse { id })))
}

/// POST /admin/videos/upload
pub async fn upload_video(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<IdResponse>), (StatusCode, Json<ErrorResponse>)> {
    let mut file_name: Option<String> = None;
    let mut category = "local".to_string();
    let mut client_hash: Option<String> = None;
    let mut temp_path: Option<std::path::PathBuf> = None;

    while let Some(mut field) = multipart.next_field().await.map_err(|_| error_response(StatusCode::BAD_REQUEST, "无效的请求格式"))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let raw_name = field.file_name().unwrap_or("video.mp4").to_string();
                // Sanitize: strip path separators and control characters, keep only the filename
                let fname = std::path::Path::new(&raw_name)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "video.mp4".to_string());
                file_name = Some(fname);

                // 流式写入临时文件
                let tmp = state.config.media_root.join(format!(".upload_{}", Uuid::new_v4()));
                let mut f = tokio::fs::File::create(&tmp).await
                    .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "创建临时文件失败"))?;
                let mut total: u64 = 0;
                loop {
                    let chunk = field.chunk().await
                        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "读取文件数据失败"))?;
                    match chunk {
                        Some(data) => {
                            f.write_all(&data).await
                                .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入文件失败"))?;
                            total += data.len() as u64;
                        }
                        None => break,
                    }
                }
                f.flush().await.map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "保存文件失败"))?;
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

    let id = state.video_service
        .upload_video_file(&file_name, &tmp_path, &category, client_hash.as_deref())
        .await
        .map_err(|e| {
            // Error path — sync removal is acceptable here
            let _ = std::fs::remove_file(&tmp_path);
            if e.starts_with("duplicate:") || e.starts_with("重复") {
                error_response(StatusCode::CONFLICT, &e)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "上传失败")
            }
        })?;

    state.video_cache.invalidate_all();
    Ok((StatusCode::CREATED, Json(IdResponse { id })))
}

#[derive(serde::Deserialize)]
pub struct UploadStatusQuery {
    pub hash: String,
}

fn is_valid_upload_hash(s: &str) -> bool {
    s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
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
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    let hash = headers.get("x-upload-hash")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "缺少 x-upload-hash"))?
        .to_string();
    if !is_valid_upload_hash(&hash) {
        return Err(error_response(StatusCode::BAD_REQUEST, "invalid hash"));
    }
    let raw_name = headers.get("x-upload-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("video.mp4")
        .to_string();
    let file_name = std::path::Path::new(&raw_name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "video.mp4".to_string());
    let total_size = headers.get("x-upload-size")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    if total_size <= 0 {
        return Err(error_response(StatusCode::BAD_REQUEST, "x-upload-size 必须大于 0"));
    }
    let category = headers.get("x-upload-category")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local")
        .to_string();

    let tmp = state.config.media_root.join(format!(".upload_{}", hash));

    if body.is_empty() {
        let received = match tokio::fs::metadata(&tmp).await {
            Ok(m) => m.len() as i64,
            Err(_) => 0,
        };
        return Ok((StatusCode::OK, Json(serde_json::json!({ "received": received }))));
    }

    let mut f = tokio::fs::OpenOptions::new()
        .create(true).append(true).open(&tmp).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "打开临时文件失败"))?;
    f.write_all(&body).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "写入失败"))?;
    f.flush().await.map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "flush失败"))?;
    drop(f);

    let received = tokio::fs::metadata(&tmp).await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    if total_size > 0 && received >= total_size {
        let tmp_clone = tmp.clone();
        let id = state.video_service
            .upload_video_file(&file_name, &tmp_clone, &category, Some(&hash))
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                if e.starts_with("重复") {
                    error_response(StatusCode::CONFLICT, &e)
                } else {
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "上传失败")
                }
            })?;
        state.video_cache.invalidate_all();
        return Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id, "received": received }))));
    }

    Ok((StatusCode::PARTIAL_CONTENT, Json(serde_json::json!({ "received": received }))))
}

/// POST /admin/videos/check-hashes
pub async fn check_hashes(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<CheckHashesRequest>,
) -> Result<Json<CheckHashesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.hashes.len() > 1000 {
        return Err(error_response(StatusCode::BAD_REQUEST, "最多检查 1000 个文件"));
    }
    let existing = state.video_service
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
        return Err(error_response(StatusCode::BAD_REQUEST, "最多检查 1000 个文件"));
    }
    let existing_indices = state.video_service
        .check_existing_files(&files)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(CheckFilesResponse { existing_indices: existing_indices.into_iter().collect() }))
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

    let added = state.video_service
        .scan_media_directory(&category)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "扫描失败"))?;

    state.video_cache.invalidate_all();
    Ok(Json(serde_json::json!({"added": added})))
}

/// PUT /admin/videos/{id}
pub async fn update_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<VideoUpdateRequest>,
) -> Json<OkResponse> {
    let ok = state.video_service
        .update_video(id, req.title.as_deref(), req.description.as_deref(), req.category.as_deref())
        .await
        .unwrap_or(false);

    state.video_cache.invalidate_all();
    if ok {
        Json(OkResponse { ok: true, error: None, deleted: None })
    } else {
        Json(OkResponse { ok: false, error: Some("视频不存在".into()), deleted: None })
    }
}

/// DELETE /admin/videos/{id}
pub async fn delete_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<OkResponse> {
    state.video_cache.invalidate_all();
    match state.video_service.delete_video(id).await {
        Ok(true) => Json(OkResponse { ok: true, error: None, deleted: None }),
        Ok(false) => Json(OkResponse { ok: false, error: Some("视频不存在".into()), deleted: None }),
        Err(e) => {
            tracing::error!("delete_video failed for id={}: {}", id, e);
            Json(OkResponse { ok: false, error: Some("删除失败".into()), deleted: None })
        },
    }
}

/// DELETE /admin/videos/batch
pub async fn delete_videos(
    State(state): State<Arc<AppState>>,
    SafeJson(ids): SafeJson<Vec<i64>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    if ids.len() > 500 {
        return Err(error_response(StatusCode::BAD_REQUEST, "最多批量删除 500 个"));
    }
    state.video_cache.invalidate_all();
    let deleted = state.video_service
        .delete_videos(ids)
        .await
        .unwrap_or(0);

    Ok(Json(OkResponse { ok: true, error: None, deleted: Some(deleted as i64) }))
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
            let data = field.bytes().await.map_err(|_| error_response(StatusCode::BAD_REQUEST, "读取封面数据失败"))?;

            state.video_service
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
pub async fn backfill_thumbnails(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match state.video_service.backfill_thumbnails().await {
        Ok((generated, errors)) => {
            state.video_cache.invalidate_all();
            Json(serde_json::json!({"ok": true, "generated": generated, "errors": errors}))
        }
        Err(e) => {
            Json(serde_json::json!({"ok": false, "error": e}))
        }
    }
}
