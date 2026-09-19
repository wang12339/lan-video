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

const MAX_PLAYLIST_NAME_LEN: usize = 100;
const MAX_PLAYLIST_DESC_LEN: usize = 500;
const MAX_BATCH_ADD_VIDEOS: usize = 50;

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
fn decode_id_or_err(id_str: &str, label: &str) -> Result<i64, (StatusCode, Json<ErrorResponse>)> {
    hashid::decode_id_or_numeric(id_str)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, format!("无效的{}ID", label)))
}

/// Convert a `ServiceError` into the handler error tuple.
fn svc_err(e: crate::util::error::ServiceError) -> (StatusCode, Json<ErrorResponse>) {
    e.into_tuple()
}

/// GET /playlists
#[utoipa::path(
    get,
    path = "/playlists",
    tag = "playlists",
    summary = "List current user's playlists",
    description = "返回当前用户创建的所有播放列表（含封面与条目数）",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Playlist list", body = PlaylistListResponse),
        (status = 401, description = "Unauthorized")
    )
)]
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
#[utoipa::path(
    post,
    path = "/playlists",
    tag = "playlists",
    summary = "Create a playlist",
    description = "创建播放列表，名称需为 1-100 字符",
    security(("bearerAuth" = [])),
    request_body = CreatePlaylistRequest,
    responses(
        (status = 201, description = "Playlist created", body = PlaylistResponse),
        (status = 400, description = "Invalid playlist name or description"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(req): SafeJson<CreatePlaylistRequest>,
) -> Result<(StatusCode, Json<PlaylistResponse>), (StatusCode, Json<ErrorResponse>)> {
    if req.name.trim().is_empty() || req.name.len() > MAX_PLAYLIST_NAME_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "播放列表名称长度需在 1-100 个字符之间",
        ));
    }
    if let Some(ref desc) = req.description {
        if desc.len() > MAX_PLAYLIST_DESC_LEN {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "播放列表描述不能超过 500 个字符",
            ));
        }
    }
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
#[utoipa::path(
    get,
    path = "/playlists/{id}",
    tag = "playlists",
    summary = "Get a playlist",
    description = "获取播放列表详情（本人、管理员或公开列表）。非公开的他人列表返回 404 避免泄露存在性",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "播放列表 ID (hashid 或数字)")),
    responses(
        (status = 200, description = "Playlist details", body = PlaylistResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Playlist not found")
    )
)]
pub async fn get_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<PlaylistResponse>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    let (p, count) = state
        .services
        .playlist
        .get_playlist(playlist_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(svc_err)?;

    Ok(Json(to_playlist_response(&p, count)))
}

#[utoipa::path(
    put,
    path = "/playlists/{id}",
    tag = "playlists",
    summary = "Update a playlist",
    description = "修改播放列表名称、描述或公开状态（仅所有者）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "播放列表 ID (hashid 或数字)")),
    request_body = UpdatePlaylistRequest,
    responses(
        (status = 200, description = "Playlist updated", body = serde_json::Value),
        (status = 400, description = "Invalid playlist name or description"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Playlist not found")
    )
)]
pub async fn update_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
    SafeJson(req): SafeJson<UpdatePlaylistRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    if let Some(ref name) = req.name {
        if name.trim().is_empty() || name.len() > MAX_PLAYLIST_NAME_LEN {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "播放列表名称长度需在 1-100 个字符之间",
            ));
        }
    }
    if let Some(ref desc) = req.description {
        if desc.len() > MAX_PLAYLIST_DESC_LEN {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "播放列表描述不能超过 500 个字符",
            ));
        }
    }
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
#[utoipa::path(
    delete,
    path = "/playlists/{id}",
    tag = "playlists",
    summary = "Delete a playlist",
    description = "删除播放列表及其全部条目（仅所有者）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "播放列表 ID (hashid 或数字)")),
    responses(
        (status = 200, description = "Playlist deleted", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Playlist not found")
    )
)]
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
#[utoipa::path(
    get,
    path = "/playlists/{id}/videos",
    tag = "playlists",
    summary = "List videos in a playlist",
    description = "按播放列表顺序返回其中的视频",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "播放列表 ID (hashid 或数字)")),
    responses(
        (status = 200, description = "Playlist videos", body = [PlaylistVideoItem]),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Playlist not found")
    )
)]
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
#[utoipa::path(
    post,
    path = "/playlists/{id}/videos",
    tag = "playlists",
    summary = "Add a video to a playlist",
    description = "向播放列表添加视频（仅所有者）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "播放列表 ID (hashid 或数字)")),
    request_body = AddVideoRequest,
    responses(
        (status = 200, description = "Video added", body = serde_json::Value),
        (status = 400, description = "Invalid video ID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Playlist or video not found")
    )
)]
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
#[utoipa::path(
    delete,
    path = "/playlists/{id}/videos/{video_id}",
    tag = "playlists",
    summary = "Remove a video from a playlist",
    description = "从播放列表中移除指定视频（仅所有者）",
    security(("bearerAuth" = [])),
    params(
        ("id" = String, Path, description = "播放列表 ID (hashid 或数字)"),
        ("video_id" = String, Path, description = "视频 ID (hashid 或数字)")
    ),
    responses(
        (status = 200, description = "Video removed", body = serde_json::Value),
        (status = 400, description = "Invalid playlist or video ID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Playlist or video not found")
    )
)]
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

#[derive(serde::Deserialize)]
pub struct BatchAddVideosRequest {
    pub video_ids: Vec<i64>,
}

pub async fn batch_add_videos_to_playlist(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(playlist_id): Path<String>,
    SafeJson(req): SafeJson<BatchAddVideosRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let playlist_id = decode_id_or_err(&playlist_id, "播放列表")?;
    if req.video_ids.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "视频列表不能为空"));
    }
    if req.video_ids.len() > MAX_BATCH_ADD_VIDEOS {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "单次最多添加 50 个视频",
        ));
    }
    let mut added = 0u64;
    for video_id in &req.video_ids {
        match state
            .services
            .playlist
            .add_video_to_playlist(playlist_id, auth_user.id, *video_id)
            .await
        {
            Ok(()) => added += 1,
            Err(crate::util::error::ServiceError::Duplicate(_)) => {}
            Err(e) => return Err(svc_err(e)),
        }
    }
    Ok(Json(serde_json::json!({ "ok": true, "added": added })))
}
