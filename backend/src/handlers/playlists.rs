use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::repositories::playlist_repo::PlaylistRow;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

#[derive(Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdatePlaylistRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Deserialize)]
pub struct AddVideoRequest {
    pub video_id: i64,
}

#[derive(Serialize)]
pub struct PlaylistResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub cover_url: Option<String>,
    pub item_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct PlaylistListResponse {
    pub playlists: Vec<PlaylistResponse>,
}

async fn verify_playlist_ownership(
    state: &AppState,
    playlist_id: i64,
    user_id: i64,
) -> Result<PlaylistRow, (StatusCode, Json<ErrorResponse>)> {
    let p = state
        .playlist_repo
        .get_playlist(playlist_id)
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?
        .ok_or(error_response(StatusCode::NOT_FOUND, "播放列表不存在"))?;
    if p.user_id != user_id {
        return Err(error_response(StatusCode::FORBIDDEN, "无权修改此播放列表"));
    }
    Ok(p)
}

/// GET /playlists
pub async fn list_my_playlists(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<PlaylistListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let playlists = state
        .playlist_repo
        .list_user_playlists_with_counts(auth_user.id)
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    let result: Vec<PlaylistResponse> = playlists
        .into_iter()
        .map(|(p, count)| PlaylistResponse {
            id: p.id,
            name: p.name,
            description: p.description,
            is_public: p.is_public,
            cover_url: p.cover_url,
            item_count: count,
            created_at: p.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            updated_at: p.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();
    Ok(Json(PlaylistListResponse { playlists: result }))
}

/// POST /playlists
pub async fn create_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<CreatePlaylistRequest>,
) -> Result<(StatusCode, Json<PlaylistResponse>), (StatusCode, Json<ErrorResponse>)> {
    if req.name.is_empty() || req.name.len() > 200 {
        return Err(error_response(StatusCode::BAD_REQUEST, "名称长度 1-200 字符"));
    }

    let p = state
        .playlist_repo
        .create_playlist(
            auth_user.id,
            &req.name,
            req.description.as_deref(),
            req.is_public.unwrap_or(false),
        )
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    Ok((
        StatusCode::CREATED,
        Json(PlaylistResponse {
            id: p.id,
            name: p.name,
            description: p.description,
            is_public: p.is_public,
            cover_url: p.cover_url,
            item_count: 0,
            created_at: p.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            updated_at: p.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }),
    ))
}

/// GET /playlists/{id}
pub async fn get_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let p = state
        .playlist_repo
        .get_playlist(playlist_id)
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?
        .ok_or(error_response(StatusCode::NOT_FOUND, "播放列表不存在"))?;

    // Only owner or admin can view non-public playlists
    if !p.is_public && p.user_id != auth_user.id && !auth_user.is_admin {
        return Err(error_response(StatusCode::NOT_FOUND, "播放列表不存在"));
    }

    let count = state
        .playlist_repo
        .count_playlist_items(playlist_id)
        .await
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "id": p.id,
        "name": p.name,
        "description": p.description,
        "isPublic": p.is_public,
        "coverUrl": p.cover_url,
        "itemCount": count,
        "createdAt": p.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        "updatedAt": p.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    })))
}

/// PUT /playlists/{id}
pub async fn update_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<i64>,
    Json(req): Json<UpdatePlaylistRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .playlist_repo
        .update_playlist(
            playlist_id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.is_public,
        )
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /playlists/{id}
pub async fn delete_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .playlist_repo
        .delete_playlist(playlist_id)
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /playlists/{id}/videos
pub async fn add_video_to_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<i64>,
    Json(req): Json<AddVideoRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<ErrorResponse>)> {
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .playlist_repo
        .add_video(playlist_id, req.video_id)
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    // Update updated_at
    let _ = state
        .playlist_repo
        .update_playlist(playlist_id, None, None, None)
        .await;

    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

/// DELETE /playlists/{id}/videos/{video_id}
pub async fn remove_video_from_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((playlist_id, video_id)): Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .playlist_repo
        .remove_video(playlist_id, video_id)
        .await
        .map_err(|e| { tracing::error!("playlist operation failed: {}", e); error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误") })?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
