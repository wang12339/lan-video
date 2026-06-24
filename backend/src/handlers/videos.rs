use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::state::AppState;
use crate::models::video::{VideoItem, PagedVideoResponse, VideoQuery};
use crate::middleware::auth::AuthUser;
use crate::util::response::{error_response, ErrorResponse};

/// GET /videos
pub async fn list_videos(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<VideoQuery>,
) -> Result<Json<PagedVideoResponse>, (StatusCode, Json<ErrorResponse>)> {
    let page = params.page.unwrap_or(0).max(0);
    let size = params.size.unwrap_or(20).clamp(1, 1000);
    let query = params.query.as_deref().unwrap_or("");
    let source_type = params.source_type.as_deref().unwrap_or("");
    let category = params.category.as_deref().unwrap_or("");
    let _username = &auth_user.username; // kept for auth extraction, not used in cached query

    // Build cache key (without username — watch_position is fetched separately per user)
    let cache_key = format!("list_videos:{}:{}:{}:{}:{}", page, size, query, source_type, category);
    if let Some(cached) = state.video_cache.get(&cache_key) {
        if let Ok(resp) = serde_json::from_str::<PagedVideoResponse>(&cached) {
            return Ok(Json(resp));
        }
    }

    // Cache doesn't include per-user watch_position, so don't pass username for the shared query
    let (items, total) = state.video_service
        .list_videos_paged(
            page, size,
            if query.is_empty() { None } else { Some(query) },
            if source_type.is_empty() { None } else { Some(source_type) },
            if category.is_empty() { None } else { Some(category) },
            None, // username omitted for cache sharing
        )
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;

    let resp = PagedVideoResponse { items, total, page, size };

    // Store in cache
    if let Ok(json) = serde_json::to_string(&resp) {
        state.video_cache.insert(cache_key, json);
    }

    Ok(Json(resp))
}

/// GET /videos/{id}
pub async fn get_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<VideoItem>, (StatusCode, Json<ErrorResponse>)> {
    let video = state.video_service
        .get_video(id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    Ok(Json(video))
}

/// POST /videos/{id}/like
pub async fn toggle_like(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let liked = state.video_service.toggle_like(&auth_user.username, id).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    Ok(Json(serde_json::json!({"liked": liked})))
}

/// GET /videos/{id}/like
pub async fn get_like_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let liked = state.video_service.is_liked(&auth_user.username, id).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(serde_json::json!({"liked": liked})))
}

/// POST /videos/{id}/favorite
pub async fn toggle_favorite(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let favorited = state.video_service.toggle_favorite(&auth_user.username, id).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// GET /videos/{id}/favorite
pub async fn get_favorite_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let favorited = state.video_service.is_favorited(&auth_user.username, id).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// POST /videos/{id}/view
pub async fn increment_views(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Rate limit: max 30 views per IP per 60s per video
    let rate_key = format!("view:{}:{}", addr.ip(), id);
    if state.rate_limiter.check_with(&rate_key, 30, 60, 300).await.is_err() {
        return Err(error_response(StatusCode::TOO_MANY_REQUESTS, "请求过于频繁"));
    }
    state.video_service.increment_views(id).await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    Ok(Json(serde_json::json!({"ok": true})))
}
