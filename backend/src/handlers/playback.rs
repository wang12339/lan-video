use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::playback::{PlaybackHistoryRequest, PlaybackHistoryResponse, RecentWatchItem};
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

/// GET /playback/history/{videoId}
pub async fn get_playback_history_for_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<i64>,
) -> Result<Json<PlaybackHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let username = &auth_user.username;

    let (position_ms, duration_ms) = state
        .playback_service
        .get_playback_data(username, video_id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?
        .unwrap_or((0, 0));

    Ok(Json(PlaybackHistoryResponse {
        video_id,
        position_ms,
        duration_ms,
    }))
}

/// GET /playback/history
pub async fn list_playback_history(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<RecentWatchItem>>, (StatusCode, Json<ErrorResponse>)> {
    let history = state
        .playback_service
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
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "播放进度不能为负数",
        ));
    }
    if payload.duration_ms > 86_400_000 * 7 {
        // 7 days max
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "视频时长超出合理范围",
        ));
    }
    if payload.position_ms > payload.duration_ms + 1000 {
        // allow 1s tolerance
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "播放位置不能超过视频时长",
        ));
    }
    state
        .playback_service
        .update_playback(
            &auth_user.username,
            payload.video_id,
            payload.position_ms,
            payload.duration_ms,
        )
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"))?;
    Ok(StatusCode::NO_CONTENT)
}

// --- 播放会话跟踪 ---

#[derive(Deserialize)]
pub struct SessionRequest {
    pub video_id: i64,
}

/// POST /playback/session/start — 开始播放会话
pub async fn start_playback_session(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<SessionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .playback_sessions
        .start(&auth_user.username, payload.video_id);
    tracing::info!(user = %auth_user.username, video_id = payload.video_id, "开始播放视频");
    Ok(StatusCode::NO_CONTENT)
}

/// POST /playback/session/heartbeat — 刷新播放会话
pub async fn playback_session_heartbeat(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<SessionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .playback_sessions
        .heartbeat(&auth_user.username, payload.video_id);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /playback/session/stop — 停止播放会话
pub async fn stop_playback_session(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<SessionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .playback_sessions
        .stop(&auth_user.username, payload.video_id);
    tracing::info!(user = %auth_user.username, video_id = payload.video_id, "停止播放视频");
    Ok(StatusCode::NO_CONTENT)
}
