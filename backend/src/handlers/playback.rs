use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use std::sync::Arc;

use crate::middleware::auth::{AppState, AuthUser};
use crate::models::playback::{PlaybackHistoryRequest, PlaybackHistoryResponse};
use crate::util::response::{error_response, ErrorResponse};

fn get_username(ext: Option<&Extension<AuthUser>>, headers: &HeaderMap) -> String {
    if let Some(ext) = ext {
        return ext.username.clone();
    }
    headers
        .get("X-Username")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "demo".to_string())
}

/// GET /playback/history/{videoId}
pub async fn get_playback_history_for_video(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(video_id): Path<i64>,
) -> Result<Json<PlaybackHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let username = get_username(None, &headers);

    let (position_ms, duration_ms) = tokio::join!(
        state.video_service.get_playback_position(&username, video_id),
        state.video_service.get_playback_duration(&username, video_id),
    );

    Ok(Json(PlaybackHistoryResponse {
        video_id,
        position_ms: position_ms.unwrap_or(Some(0)).unwrap_or(0),
        duration_ms: duration_ms.unwrap_or(Some(0)).unwrap_or(0),
    }))
}

/// GET /playback/history
pub async fn list_playback_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<PlaybackHistoryResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let username = get_username(None, &headers);
    let history = state.video_service
        .get_playback_history(&username)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(Json(history))
}

/// POST /playback/history
pub async fn update_playback_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<PlaybackHistoryRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let username = get_username(None, &headers);
    state.video_service
        .update_playback(&username, payload.video_id, payload.position_ms, payload.duration_ms)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    Ok(StatusCode::NO_CONTENT)
}
