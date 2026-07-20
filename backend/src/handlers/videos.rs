use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::video::{PagedVideoResponse, VideoItem, VideoQuery};
use crate::state::AppState;
use crate::util::response::{error_response, internal_error_log, ErrorResponse};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<i64>,
    pub size: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct SearchResponse {
    pub items: Vec<SearchResultItem>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(serde::Serialize)]
pub struct SearchResultItem {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rank: f32,
    pub headline: Option<String>,
}

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
    let uploader_id = params.uploader_id;
    let _auth_user = auth_user; // kept for auth extraction, not used in cached query

    // Build cache key (without username — watch_position is fetched separately per user)
    let cache_key = format!(
        "list_videos:{}:{}:{}:{}:{}:{}",
        page, size, query, source_type, category,
        uploader_id.unwrap_or(0)
    );
    if let Some(resp) = state.video_cache.get(&cache_key) {
        return Ok(Json(resp));
    }

    // Cache doesn't include per-user watch_position, so don't pass username for the shared query
    let (items, total) = state
        .video_service
        .list_videos_paged(
            page,
            size,
            if query.is_empty() { None } else { Some(query) },
            if source_type.is_empty() {
                None
            } else {
                Some(source_type)
            },
            if category.is_empty() {
                None
            } else {
                Some(category)
            },
            None, // username omitted for cache sharing
            uploader_id,
        )
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;

    let resp = PagedVideoResponse {
        items,
        total,
        page,
        size,
    };

    // Store in cache
    state.video_cache.insert(cache_key, resp.clone());

    Ok(Json(resp))
}

/// GET /videos/{id}
pub async fn get_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<VideoItem>, (StatusCode, Json<ErrorResponse>)> {
    let video = state
        .video_service
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
    let liked = state
        .playback_service
        .toggle_like(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    tracing::info!(user = %auth_user.username, video_id = id, liked = liked, "toggle like");
    Ok(Json(serde_json::json!({"liked": liked})))
}

/// GET /videos/{id}/like
pub async fn get_like_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let liked = state
        .playback_service
        .is_liked(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(serde_json::json!({"liked": liked})))
}

/// POST /videos/{id}/favorite
pub async fn toggle_favorite(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let favorited = state
        .playback_service
        .toggle_favorite(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    tracing::info!(user = %auth_user.username, video_id = id, favorited = favorited, "toggle favorite");
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// GET /videos/{id}/favorite
pub async fn get_favorite_status(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let favorited = state
        .playback_service
        .is_favorited(&auth_user.username, id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(serde_json::json!({"favorited": favorited})))
}

/// GET /videos/favorites
pub async fn list_favorites(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<crate::models::playback::RecentWatchItem>>, (StatusCode, Json<ErrorResponse>)>
{
    let favorites = state
        .playback_service
        .get_favorites(&auth_user.username)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "查询失败"))?;
    Ok(Json(favorites))
}

/// POST /videos/{id}/view
pub async fn increment_views(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Rate limit: max 30 views per IP per 60s per video
    let rate_key = format!("view:{}:{}", addr.ip(), id);
    if state
        .ip_rate_limiter
        .check_with(&rate_key, 30, 60, 300)
        .await
        .is_err()
    {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "请求过于频繁",
        ));
    }
    state
        .video_service
        .increment_views(id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "操作失败"))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

const MAX_SEARCH_QUERY_LEN: usize = 200;

/// GET /videos/search
pub async fn search_videos(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let page = params.page.unwrap_or(0).max(0);
    let size = params.size.unwrap_or(20).clamp(1, 100);

    if params.q.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("搜索关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

    let (results, total) = state
        .search_service
        .full_text_search(&params.q, page, size)
        .await
        .map_err(|e| internal_error_log("search_videos", &e))?;

    let items: Vec<SearchResultItem> = results
        .into_iter()
        .map(|r| SearchResultItem {
            id: r.video_id,
            title: r.title,
            description: r.description,
            category: r.category,
            rank: r.rank,
            headline: r.headline,
        })
        .collect();

    Ok(Json(SearchResponse {
        items,
        total,
        page,
        size,
    }))
}

/// GET /videos/search/suggest
pub async fn search_suggest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    if params.q.len() > MAX_SEARCH_QUERY_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("搜索关键词不能超过 {} 个字符", MAX_SEARCH_QUERY_LEN),
        ));
    }

    let suggestions = state
        .search_service
        .search_suggest(&params.q, 10)
        .await
        .map_err(|e| internal_error_log("search_suggest", &e))?;

    Ok(Json(suggestions))
}
