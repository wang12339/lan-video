use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
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
    let size = params.size.unwrap_or(20).max(1).min(1000);
    let query = params.query.as_deref().unwrap_or("");
    let source_type = params.source_type.as_deref().unwrap_or("");
    let category = params.category.as_deref().unwrap_or("");
    let username = &auth_user.username;

    // Build cache key and check cache
    let cache_key = format!("list_videos:{}:{}:{}:{}:{}:{}", page, size, query, source_type, category, username);
    if let Some(cached) = state.video_cache.get(&cache_key) {
        if let Ok(resp) = serde_json::from_str::<PagedVideoResponse>(&cached) {
            return Ok(Json(resp));
        }
    }

    let (items, total) = state.video_service
        .list_videos_paged(
            page, size,
            if query.is_empty() { None } else { Some(query) },
            if source_type.is_empty() { None } else { Some(source_type) },
            if category.is_empty() { None } else { Some(category) },
            if username.is_empty() { None } else { Some(&username) },
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
