use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::repositories::playlist_repo::{PlaylistRow, PlaylistVideoRow};
use crate::state::AppState;
use crate::util::hashid;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

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
    #[serde(deserialize_with = "crate::util::hashid_serde::deserialize_id")]
    pub video_id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistResponse {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
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
#[serde(rename_all = "camelCase")]
pub struct PlaylistListResponse {
    pub playlists: Vec<PlaylistResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistVideoItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub description: String,
    pub source_type: String,
    pub cover_url: Option<String>,
    pub stream_url: String,
    pub category: String,
    pub views: i64,
    pub duration: i64,
}

/// Validate a playlist name: non-blank and at most 200 characters.
/// Postgres VARCHAR(200) counts characters, not bytes, so use char count
/// (byte length would wrongly reject multi-byte names such as Chinese).
fn is_valid_playlist_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && trimmed.chars().count() <= 200
}

/// Check whether a sqlx error is a foreign-key violation on a named constraint.
fn is_fk_violation(e: &sqlx::Error, constraint: &str) -> bool {
    matches!(
        e,
        sqlx::Error::Database(db)
            if db.code().as_deref() == Some("23503")
                && db.constraint() == Some(constraint)
    )
}

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

async fn get_playlist_or_404(
    state: &AppState,
    playlist_id: i64,
) -> Result<PlaylistRow, (StatusCode, Json<ErrorResponse>)> {
    state
        .repos
        .playlist
        .get_playlist(playlist_id)
        .await
        .map_err(|e| internal_error_log("playlist operation failed", &e))?
        .ok_or(error_response(StatusCode::NOT_FOUND, "播放列表不存在"))
}

/// Load a playlist the user is allowed to read: owner, admin, or public.
/// Non-public playlists of other users are reported as NOT_FOUND so their
/// existence is not leaked (same behavior as GET /playlists/{id}).
async fn load_visible_playlist(
    state: &AppState,
    playlist_id: i64,
    auth_user: &AuthUser,
) -> Result<PlaylistRow, (StatusCode, Json<ErrorResponse>)> {
    let p = get_playlist_or_404(state, playlist_id).await?;
    if p.is_public || p.user_id == auth_user.id || auth_user.is_admin {
        Ok(p)
    } else {
        Err(error_response(StatusCode::NOT_FOUND, "播放列表不存在"))
    }
}

async fn verify_playlist_ownership(
    state: &AppState,
    playlist_id: i64,
    user_id: i64,
) -> Result<PlaylistRow, (StatusCode, Json<ErrorResponse>)> {
    let p = get_playlist_or_404(state, playlist_id).await?;
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
        .repos
        .playlist
        .list_user_playlists_with_counts(auth_user.id)
        .await
        .map_err(|e| internal_error_log("playlist operation failed", &e))?;

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
    if !is_valid_playlist_name(&req.name) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "名称长度 1-200 字符",
        ));
    }

    let p = state
        .repos
        .playlist
        .create_playlist(
            auth_user.id,
            &req.name,
            req.description.as_deref(),
            req.is_public.unwrap_or(false),
        )
        .await
        .map_err(|e| internal_error_log("playlist operation failed", &e))?;

    Ok((StatusCode::CREATED, Json(to_playlist_response(&p, 0))))
}

/// GET /playlists/{id}
pub async fn get_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = hashid::decode_id_or_numeric(&playlist_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的播放列表ID"))?;
    let p = load_visible_playlist(&state, playlist_id, &auth_user).await?;

    let count = state
        .repos
        .playlist
        .count_playlist_items(playlist_id)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(playlist_id, "count_playlist_items failed: {}", e);
            0
        });

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
    let playlist_id = hashid::decode_id_or_numeric(&playlist_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的播放列表ID"))?;
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    if let Some(name) = &req.name {
        if !is_valid_playlist_name(name) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "名称长度 1-200 字符",
            ));
        }
    }

    state
        .repos
        .playlist
        .update_playlist(
            playlist_id,
            req.name.as_deref(),
            req.description.as_deref(),
            req.is_public,
        )
        .await
        .map_err(|e| internal_error_log("playlist operation failed", &e))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /playlists/{id}
pub async fn delete_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = hashid::decode_id_or_numeric(&playlist_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的播放列表ID"))?;
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .repos
        .playlist
        .delete_playlist(playlist_id)
        .await
        .map_err(|e| internal_error_log("playlist operation failed", &e))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /playlists/{id}/videos — videos inside a playlist, in playlist order
pub async fn list_playlist_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<Vec<PlaylistVideoItem>>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = hashid::decode_id_or_numeric(&playlist_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的播放列表ID"))?;
    // Consistent with GET /playlists/{id}: owner, admin, or the public can
    // read a playlist's videos. Only the owner may modify it.
    load_visible_playlist(&state, playlist_id, &auth_user).await?;

    let videos = state
        .repos
        .playlist
        .list_playlist_videos(playlist_id)
        .await
        .map_err(|e| internal_error_log("list_playlist_videos failed", &e))?;

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
    let playlist_id = hashid::decode_id_or_numeric(&playlist_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的播放列表ID"))?;
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .repos
        .playlist
        .add_video(playlist_id, req.video_id)
        .await
        .map_err(|e| {
            if is_fk_violation(&e, "playlist_items_video_id_fkey") {
                return error_response(StatusCode::NOT_FOUND, "视频不存在");
            }
            if is_fk_violation(&e, "playlist_items_playlist_id_fkey") {
                return error_response(StatusCode::NOT_FOUND, "播放列表不存在");
            }
            internal_error_log("playlist operation failed", &e)
        })?;

    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

/// DELETE /playlists/{id}/videos/{video_id}
pub async fn remove_video_from_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((playlist_id, video_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = hashid::decode_id_or_numeric(&playlist_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的播放列表ID"))?;
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    verify_playlist_ownership(&state, playlist_id, auth_user.id).await?;

    state
        .repos
        .playlist
        .remove_video(playlist_id, video_id)
        .await
        .map_err(|e| internal_error_log("playlist operation failed", &e))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

impl From<PlaylistVideoRow> for PlaylistVideoItem {
    fn from(v: PlaylistVideoRow) -> Self {
        Self {
            id: v.id,
            title: v.title,
            description: v.description,
            source_type: v.source_type,
            cover_url: v.cover_url,
            stream_url: v.stream_url,
            category: v.category,
            views: v.views,
            duration: v.duration,
        }
    }
}
