use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::playback::{
    PagedRecentWatchResponse, PaginationQuery, PlaybackHistoryRequest, PlaybackHistoryResponse,
    SessionRequest,
};
use crate::state::AppState;
use crate::util::hashid;
use crate::util::pagination::PaginationParams;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

/// GET /playback/history/{videoId}
pub async fn get_playback_history_for_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
) -> Result<Json<PlaybackHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    if video_id <= 0 {
        // decode_id_or_numeric also accepts "0" and negatives — no such
        // video exists, so reject before hitting the database.
        return Err(error_response(StatusCode::BAD_REQUEST, "无效的视频ID"));
    }

    let (position_ms, duration_ms) = state
        .services
        .playback
        .get_playback_data(auth_user.tenant_id, &auth_user.username, video_id)
        .await
        .map_err(|e| internal_error_log("get_playback_data", &e))?
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
    Query(params): Query<PaginationQuery>,
) -> Result<Json<PagedRecentWatchResponse>, (StatusCode, Json<ErrorResponse>)> {
    let pagination = PaginationParams::new(params.page, params.size);
    let page = pagination.page;
    let size = pagination.page_size;
    let offset = pagination.offset();

    let (items, total) = state
        .services
        .playback
        .get_playback_history(auth_user.tenant_id, &auth_user.username, size, offset)
        .await
        .map_err(|e| internal_error_log("get_playback_history", &e))?;
    Ok(Json(PagedRecentWatchResponse {
        items,
        total,
        page,
        size,
    }))
}

/// POST /playback/history
pub async fn update_playback_history(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<PlaybackHistoryRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Validate playback values
    if payload.video_id <= 0 {
        return Err(error_response(StatusCode::BAD_REQUEST, "无效的视频ID"));
    }
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
    if payload.position_ms > payload.duration_ms.checked_add(1000).unwrap_or(i64::MIN) {
        // allow 1s tolerance; checked_add guards the arithmetic (the 7-day
        // cap above makes overflow unreachable, but the fallback still
        // rejects rather than silently accepting an uncomputable bound)
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "播放位置不能超过视频时长",
        ));
    }
    // 片尾判定以服务端片长为准（防客户端谎报时长度绕过焚毁判定）。
    // 到达片尾时强制落库（绕过节流，片尾写每用户每视频仅一次），
    // 并由服务端直接焚毁——不依赖前端是否调用焚毁接口。
    let at_end = match state
        .services
        .video
        .get_video(auth_user.tenant_id, payload.video_id)
        .await
    {
        Ok(Some(v)) => crate::services::video_service::VideoService::is_at_end(
            payload.position_ms,
            v.duration,
            payload.duration_ms,
        ),
        _ => false,
    };
    state
        .services
        .playback
        .update_playback_opts(
            auth_user.tenant_id,
            &auth_user.username,
            payload.video_id,
            payload.position_ms,
            payload.duration_ms,
            at_end,
        )
        .await
        .map_err(|e| internal_error_log("update_playback", &e))?;
    if at_end {
        match state
            .services
            .video
            .burn_video_record(auth_user.tenant_id, payload.video_id)
            .await
        {
            Ok(_) => {
                state.invalidate_caches();
                state.playback_sessions.stop(
                    auth_user.tenant_id,
                    &auth_user.username,
                    payload.video_id,
                );
                tracing::info!(
                    user = %auth_user.username,
                    video_id = payload.video_id,
                    "video burned on watch completion (history trigger)"
                );
            }
            Err(crate::util::error::ServiceError::NotFound(_)) => {}
            Err(e) => tracing::warn!(
                video_id = payload.video_id,
                error = %e,
                "auto burn on watch completion failed"
            ),
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

// --- 播放会话跟踪 ---

fn validate_session_request(
    payload: &SessionRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if payload.video_id <= 0 {
        Err(error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))
    } else {
        Ok(())
    }
}

/// POST /playback/session/start — 开始播放会话
pub async fn start_playback_session(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<SessionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    validate_session_request(&payload)?;
    // 会话只对“存在且属于当前租户”的视频授予。未注册/已删除/跨租户
    // 视频一律不建会话 —— 跨租户访问的合法通道只有分享链接(cookie)。
    match state
        .repos
        .video
        .find_by_id(auth_user.tenant_id, payload.video_id)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(error_response(StatusCode::NOT_FOUND, "视频不存在"));
        }
        Err(e) => return Err(internal_error_log("session start: find video", &e)),
    }
    state
        .playback_sessions
        .start(auth_user.tenant_id, &auth_user.username, payload.video_id);
    tracing::info!(
        user = %auth_user.username,
        video_id = payload.video_id,
        tenant_id = auth_user.tenant_id,
        "开始播放视频"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// POST /playback/session/heartbeat — 刷新播放会话
pub async fn playback_session_heartbeat(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<SessionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    validate_session_request(&payload)?;
    state
        .playback_sessions
        .heartbeat(auth_user.tenant_id, &auth_user.username, payload.video_id);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /playback/session/stop — 停止播放会话
pub async fn stop_playback_session(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(payload): SafeJson<SessionRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    validate_session_request(&payload)?;
    state
        .playback_sessions
        .stop(auth_user.tenant_id, &auth_user.username, payload.video_id);
    tracing::info!(user = %auth_user.username, video_id = payload.video_id, "停止播放视频");
    Ok(StatusCode::NO_CONTENT)
}
