use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use moka::sync::Cache;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex;

use crate::middleware::auth::AuthUser;
use crate::services::recommendation_service::VideoRecommendation;
use crate::state::AppState;
use crate::util::hashid;
use crate::util::response::{error_response, internal_error_log, ErrorResponse};

fn map_to_recommendation(r: VideoRecommendation) -> RecommendationItem {
    RecommendationItem {
        id: r.id,
        title: r.title,
        category: r.category,
        thumb_url: r.thumb_url,
        score: r.score,
        reason: r.reason,
    }
}

/// Per-key in-flight locks used to prevent cache stampede (cache breakdown):
/// when a key expires, concurrent requests serialize on this lock instead of
/// all hitting the database at once. TTL-bounded so the lock table cannot grow
/// unboundedly — waiters hold an `Arc` clone which keeps the lock alive until
/// the load finishes.
static INFLIGHT_LOCKS: OnceLock<Cache<String, Arc<Mutex<()>>>> = OnceLock::new();

fn inflight_locks() -> &'static Cache<String, Arc<Mutex<()>>> {
    INFLIGHT_LOCKS.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(60))
            .max_capacity(4096)
            .build()
    })
}

/// Return the cached recommendation list for `key`, or load it via `loader`
/// and populate the cache. Concurrent misses for the same key are coalesced
/// via `inflight_locks` (double-checked after acquiring the lock).
async fn get_cached_recommendations<F, Fut>(
    state: &AppState,
    key: &str,
    loader: F,
) -> Result<Vec<VideoRecommendation>, (StatusCode, Json<ErrorResponse>)>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<VideoRecommendation>, String>>,
{
    if let Some(cached) = state.recommendation_cache.get(key) {
        return Ok(cached);
    }
    let lock = inflight_locks().get_with(key.to_string(), || Arc::new(Mutex::new(())));
    let _guard = lock.lock().await;
    if let Some(cached) = state.recommendation_cache.get(key) {
        return Ok(cached);
    }
    let items = loader().await.map_err(|e| {
        tracing::error!("recommendation load failed ({}): {}", key, e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
    })?;
    state
        .recommendation_cache
        .insert(key.to_string(), items.clone());
    Ok(items)
}

#[derive(serde::Serialize)]
pub struct RecommendationResponse {
    pub items: Vec<RecommendationItem>,
    pub total: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationItem {
    #[serde(serialize_with = "crate::util::hashid_serde::serialize_id")]
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
    pub score: f64,
    pub reason: String,
}

/// GET /recommendations
///
/// Get personalized recommendations based on user's viewing history
pub async fn get_recommendations(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let recommendations = state
        .services
        .recommendation
        .get_recommendations(&auth_user.username, 0, 20)
        .await
        .map_err(|e| internal_error_log("get_recommendations failed", &e))?;

    let items: Vec<RecommendationItem> = recommendations
        .into_iter()
        .map(map_to_recommendation)
        .collect();

    Ok(Json(RecommendationResponse {
        total: items.len(),
        items,
    }))
}

/// GET /recommendations/similar/{video_id}
///
/// Get videos similar to a specific video
pub async fn get_similar_videos(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<String>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;

    let cache_key = format!("similar:{}", video_id);
    let recommendations = get_cached_recommendations(&state, &cache_key, || {
        state
            .services
            .recommendation
            .get_similar_videos(video_id, 10)
    })
    .await?;

    let items: Vec<RecommendationItem> = recommendations
        .into_iter()
        .map(map_to_recommendation)
        .collect();

    Ok(Json(RecommendationResponse {
        total: items.len(),
        items,
    }))
}

/// GET /recommendations/trending
///
/// Get trending/popular videos (cached for 2 minutes)
pub async fn get_trending_videos(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let recommendations = get_cached_recommendations(&state, "trending", || {
        state.services.recommendation.get_trending_videos(20)
    })
    .await?;

    let items: Vec<RecommendationItem> = recommendations
        .into_iter()
        .map(map_to_recommendation)
        .collect();

    Ok(Json(RecommendationResponse {
        total: items.len(),
        items,
    }))
}

/// GET /recommendations/recent
///
/// Get recently uploaded videos (cached for 2 minutes)
pub async fn get_recent_videos(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let recommendations = get_cached_recommendations(&state, "recent", || {
        state.services.recommendation.get_recent_videos(20)
    })
    .await?;

    let items: Vec<RecommendationItem> = recommendations
        .into_iter()
        .map(map_to_recommendation)
        .collect();

    Ok(Json(RecommendationResponse {
        total: items.len(),
        items,
    }))
}
