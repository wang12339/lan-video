use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::middleware::auth::AppState;
use crate::models::video::*;
use crate::util::response::{error_response, ErrorResponse};

/// POST /admin/videos/external
pub async fn add_external_video(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExternalVideoRequest>,
) -> Result<(StatusCode, Json<IdResponse>), (StatusCode, Json<ErrorResponse>)> {
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
    let mut file_data: Option<(String, Vec<u8>, String, Option<String>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|_| error_response(StatusCode::BAD_REQUEST, "invalid multipart"))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let file_name = field.file_name().unwrap_or("video.mp4").to_string();
                let data = field.bytes().await.map_err(|_| error_response(StatusCode::BAD_REQUEST, "failed to read file data"))?.to_vec();
                if let Some(ref mut entry) = file_data {
                    entry.0 = file_name;
                    entry.1 = data;
                } else {
                    file_data = Some((file_name, data, "local".to_string(), None));
                }
            }
            "category" => {
                let val = field.text().await.unwrap_or_default();
                if let Some(ref mut entry) = file_data {
                    entry.2 = val;
                } else {
                    file_data = Some(("video.mp4".into(), vec![], val, None));
                }
            }
            "fileHash" => {
                let val = field.text().await.unwrap_or_default();
                if let Some(ref mut entry) = file_data {
                    entry.3 = Some(val);
                } else {
                    file_data = Some(("video.mp4".into(), vec![], "local".to_string(), Some(val)));
                }
            }
            _ => {}
        }
    }

    let (file_name, data, category, client_hash) = file_data.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "missing file"))?;

    if data.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "empty file"));
    }

    let id = state.video_service
        .upload_video(&file_name, data.into(), &category, client_hash.as_deref())
        .await
        .map_err(|e| {
            if e.starts_with("duplicate:") {
                error_response(StatusCode::CONFLICT, &e)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "upload failed")
            }
        })?;

    state.video_cache.invalidate_all();
    Ok((StatusCode::CREATED, Json(IdResponse { id })))
}

/// POST /admin/videos/check-hashes
pub async fn check_hashes(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckHashesRequest>,
) -> Result<Json<CheckHashesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let existing = state.video_service
        .check_existing_hashes(req.hashes)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(CheckHashesResponse { existing }))
}

/// POST /admin/videos/check-files
pub async fn check_files(
    State(state): State<Arc<AppState>>,
    Json(files): Json<Vec<FileCheckItem>>,
) -> Result<Json<CheckFilesResponse>, (StatusCode, Json<ErrorResponse>)> {
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
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "scan failed"))?;

    state.video_cache.invalidate_all();
    Ok(Json(serde_json::json!({"added": added})))
}

/// PUT /admin/videos/{id}
pub async fn update_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<VideoUpdateRequest>,
) -> Json<OkResponse> {
    let ok = state.video_service
        .update_video(id, req.title.as_deref(), req.description.as_deref(), req.category.as_deref())
        .await
        .unwrap_or(false);

    state.video_cache.invalidate_all();
    if ok {
        Json(OkResponse { ok: true, error: None, deleted: None })
    } else {
        Json(OkResponse { ok: false, error: Some("not found".into()), deleted: None })
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
        Ok(false) => Json(OkResponse { ok: false, error: Some("not found".into()), deleted: None }),
        Err(e) => Json(OkResponse { ok: false, error: Some(e), deleted: None }),
    }
}

/// DELETE /admin/videos/batch
pub async fn delete_videos(
    State(state): State<Arc<AppState>>,
    Json(ids): Json<Vec<i64>>,
) -> Json<OkResponse> {
    state.video_cache.invalidate_all();
    let deleted = state.video_service
        .delete_videos(ids)
        .await
        .unwrap_or(0);

    Json(OkResponse { ok: true, error: None, deleted: Some(deleted as i64) })
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
            let data = field.bytes().await.map_err(|_| error_response(StatusCode::BAD_REQUEST, "failed to read cover data"))?;

            state.video_service
                .update_cover(id, &file_name, data)
                .await
                .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "cover update failed"))?;

            state.video_cache.invalidate_all();
            return Ok(StatusCode::NO_CONTENT);
        }
    }

    Err(error_response(StatusCode::BAD_REQUEST, "missing file in request"))
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
