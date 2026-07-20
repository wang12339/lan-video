use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::services::recommendation_service::VideoRecommendation;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

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

#[derive(serde::Serialize)]
pub struct RecommendationResponse {
    pub items: Vec<RecommendationItem>,
    pub total: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationItem {
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
    pub score: f32,
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
        .recommendation_service
        .get_recommendations(&auth_user.username, 0, 20)
        .await
        .map_err(|e| {
            tracing::error!("get_recommendations failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

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
    Path(video_id): Path<i64>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let recommendations = state
        .recommendation_service
        .get_similar_videos(video_id, 10)
        .await
        .map_err(|e| {
            tracing::error!("get_similar_videos failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

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
/// Get trending/popular videos
pub async fn get_trending_videos(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let recommendations = state
        .recommendation_service
        .get_trending_videos(20)
        .await
        .map_err(|e| {
            tracing::error!("get_trending_videos failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

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
/// Get recently uploaded videos
pub async fn get_recent_videos(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let recommendations = state
        .recommendation_service
        .get_recent_videos(20)
        .await
        .map_err(|e| {
            tracing::error!("get_recent_videos failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    let items: Vec<RecommendationItem> = recommendations
        .into_iter()
        .map(map_to_recommendation)
        .collect();

    Ok(Json(RecommendationResponse {
        total: items.len(),
        items,
    }))
}
