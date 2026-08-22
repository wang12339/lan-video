use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::playlist::{
    AddVideoRequest, CreatePlaylistRequest, PlaylistListResponse, PlaylistResponse,
    PlaylistVideoItem, ReorderRequest, UpdatePlaylistRequest,
};
use crate::repositories::playlist_repo::PlaylistRow;
use crate::state::AppState;
use crate::util::hashid;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

/// Build the API response for a playlist row, consistent across all endpoints.
fn to_playlist_response(p: &PlaylistRow, count: i64) -> PlaylistResponse {
    PlaylistResponse {
        id: p.id,
        name: p.name.clone(),
        description: p.description.clone(),
        is_public: p.is_public,
        cover_url: p.cover_url.clone(),
        item_count: count,
        created_at: p.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        updated_at: p.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

/// Decode a hashid or numeric string to an i64, returning a BAD_REQUEST error on failure.
fn decode_id_or_err(
    id_str: &str,
    label: &str,
) -> Result<i64, (StatusCode, Json<ErrorResponse>)> {
    hashid::decode_id_or_numeric(id_str)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, format!("无效的{}ID", label)))
}

/// Convert a `ServiceError` into the handler error tuple.
fn svc_err(e: crate::util::error::ServiceError) -> (StatusCode, Json<ErrorResponse>) {
    e.into_tuple()
}

/// GET /playlists
pub async fn list_my_playlists(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<PlaylistListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let playlists = state
        .services
        .playlist
        .list_user_playlists(auth_user.id)
        .await
        .map_err(svc_err)?;

    let result: Vec<PlaylistResponse> = playlists
        .into_iter()
        .map(|(p, count)| to_playlist_response(&p, count))
        .collect();
    Ok(Json(PlaylistListResponse { playlists: result }))
}

/// POST /playlists
pub async fn create_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(req): SafeJson<CreatePlaylistRequest>,
) -> Result<(StatusCode, Json<PlaylistResponse>), (StatusCode, Json<ErrorResponse>)> {
    let p = state
        .services
        .playlist
        .create_playlist(
            auth_user.id,
            &req.name,
            req.description.as_deref(),
            req.is_public,
        )
        .await
        .map_err(svc_err)?;

    Ok((StatusCode::CREATED, Json(to_playlist_response(&p, 0))))
}

/// GET /playlists/{id}
pub async fn get_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    let (p, count) = state
        .services
        .playlist
        .get_playlist(playlist_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(svc_err)?;

    let value = serde_json::to_value(to_playlist_response(&p, count))
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(Json(value))
}

/// PUT /playlists/{id}
pub async fn update_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
    SafeJson(req): SafeJson<UpdatePlaylistRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    state
        .services
        .playlist
        .update_playlist(
            playlist_id,
            auth_user.id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.is_public,
        )
        .await
        .map_err(svc_err)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /playlists/{id}
pub async fn delete_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    state
        .services
        .playlist
        .delete_playlist(playlist_id, auth_user.id)
        .await
        .map_err(svc_err)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /playlists/{id}/videos — videos inside a playlist, in playlist order
pub async fn list_playlist_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<Vec<PlaylistVideoItem>>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    let videos = state
        .services
        .playlist
        .list_playlist_videos(playlist_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(svc_err)?;

    Ok(Json(
        videos.into_iter().map(PlaylistVideoItem::from).collect(),
    ))
}

/// POST /playlists/{id}/videos
pub async fn add_video_to_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
    SafeJson(req): SafeJson<AddVideoRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    state
        .services
        .playlist
        .add_video_to_playlist(playlist_id, auth_user.id, req.video_id)
        .await
        .map_err(svc_err)?;

    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

/// DELETE /playlists/{id}/videos/{video_id}
pub async fn remove_video_from_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((playlist_id, video_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    let video_id = decode_id_or_err(&video_id, "视频")?;
    state
        .services
        .playlist
        .remove_video_from_playlist(playlist_id, auth_user.id, video_id)
        .await
        .map_err(svc_err)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// PUT /playlists/{id}/reorder
pub async fn reorder_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
    SafeJson(req): SafeJson<ReorderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    state
        .services
        .playlist
        .reorder_playlist(playlist_id, auth_user.id, &req.video_ids)
        .await
        .map_err(svc_err)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
