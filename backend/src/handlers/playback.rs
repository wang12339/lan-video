use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;

use crate::state::AppState;
use crate::middleware::auth::AuthUser;
use crate::models::playback::{PlaybackHistoryRequest, PlaybackHistoryResponse, RecentWatchItem};
use crate::util::response::{error_response, ErrorResponse, SafeJson};

/// GET /playback/history/{videoId}
pub async fn get_playback_history_for_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<i64>,
) -> Result<Json<PlaybackHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let username = &auth_user.username;

    let (position_ms, duration_ms) = tokio::join!(
        state.video_service.get_playback_position(username, video_id),
        state.video_service.get_playback_duration(username, video_id),
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
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<RecentWatchItem>>, (StatusCode, Json<ErrorResponse>)> {
    let history = state.video_service
        .get_playback_history(&auth_user.username)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(Json(history))
}

/// POST /playback/history
pub async fn update_playback_history(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<PlaybackHistoryRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Validate playback values
    if payload.position_ms < 0 || payload.duration_ms < 0 {
        return Err(error_response(StatusCode::BAD_REQUEST, "播放进度不能为负数"));
    }
    if payload.duration_ms > 86_400_000 * 7 { // 7 days max
        return Err(error_response(StatusCode::BAD_REQUEST, "视频时长超出合理范围"));
    }
    if payload.position_ms > payload.duration_ms + 1000 { // allow 1s tolerance
        return Err(error_response(StatusCode::BAD_REQUEST, "播放位置不能超过视频时长"));
    }
    state.video_service
        .update_playback(&auth_user.username, payload.video_id, payload.position_ms, payload.duration_ms)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(StatusCode::NO_CONTENT)
}
