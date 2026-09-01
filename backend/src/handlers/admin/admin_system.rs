use axum::{extract::State, http::StatusCode, Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::admin::{RegistrationToggleRequest, TrackRequest};
use crate::models::video::OkResponse;
use crate::state::AppState;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

/// POST /admin/track — 记录用户操作
pub async fn track_action(
    _state: State<Arc<AppState>>,
    axum::Extension(auth_user): axum::Extension<crate::middleware::auth::AuthUser>,
    SafeJson(req): SafeJson<TrackRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    tracing::info!(
        user = %auth_user.username,
        action = %req.action,
        target = req.target.as_deref().unwrap_or(""),
        page = req.page.as_deref().unwrap_or(""),
        "用户操作"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// GET /admin/stats — 数据统计面板
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let by_type = state
        .repos
        .video
        .count_by_type(auth_user.tenant_id)
        .await
        .map_err(|e| internal_error_log("count_by_type", &e))?;
    let by_category = state
        .repos
        .video
        .count_by_category(auth_user.tenant_id)
        .await
        .map_err(|e| internal_error_log("count_by_category", &e))?;
    let total_views = state
        .repos
        .video
        .total_views(auth_user.tenant_id)
        .await
        .map_err(|e| internal_error_log("total_views", &e))?;
    let total_duration = state
        .repos
        .video
        .total_duration_secs(auth_user.tenant_id)
        .await
        .map_err(|e| internal_error_log("total_duration", &e))?;
    let user_count = state
        .repos
        .user
        .count_users(auth_user.tenant_id)
        .await
        .map_err(|e| internal_error_log("count_users", &e))?;
    let pending_count = state
        .repos
        .user
        .count_pending_users(auth_user.tenant_id)
        .await
        .map_err(|e| internal_error_log("count_pending", &e))?;

    let total_videos: i64 = by_type.iter().map(|(_, c)| c).sum();
    let video_count: i64 = by_type
        .iter()
        .filter(|(t, _)| t.starts_with("local_video") || t == "external")
        .map(|(_, c)| c)
        .sum();
    let image_count: i64 = by_type
        .iter()
        .filter(|(t, _)| t == "local_image")
        .map(|(_, c)| c)
        .sum();

    Ok(Json(serde_json::json!({
        "totalVideos": total_videos,
        "videoCount": video_count,
        "imageCount": image_count,
        "userCount": user_count,
        "pendingCount": pending_count,
        "totalViews": total_views,
        "totalDurationSecs": total_duration,
        "byType": by_type.into_iter().map(|(t, c)| serde_json::json!({"type": t, "count": c})).collect::<Vec<_>>(),
        "byCategory": by_category.into_iter().map(|(cat, c)| serde_json::json!({"category": cat, "count": c})).collect::<Vec<_>>(),
    })))
}

/// GET /admin/config/registration — 查询注册开关状态
pub async fn get_registration_enabled(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "enabled": state.config.registration_enabled(),
    }))
}

pub async fn set_registration_enabled(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<RegistrationToggleRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Persist to DB first; only update in-memory state on success
    state
        .repos
        .registration
        .set_enabled(req.enabled)
        .await
        .map_err(|e| {
            tracing::error!("Failed to persist registration toggle: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "保存注册开关失败")
        })?;
    state.config.set_registration_enabled(req.enabled);
    tracing::info!(
        enabled = req.enabled,
        "registration toggle changed by admin"
    );
    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: None,
    }))
}

pub async fn system_info(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let db_connections = state
        .repos
        .user
        .count_active_connections()
        .await
        .unwrap_or(0);

    let media_root = state.config.media_root.clone();
    let disk_usage = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            const MAX_ENTRIES: u64 = 100_000;
            let mut total: u64 = 0;
            if let Ok(entries) = std::fs::read_dir(&media_root) {
                for (i, entry) in entries.flatten().enumerate() {
                    if i as u64 >= MAX_ENTRIES {
                        break;
                    }
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
            total
        }),
    )
    .await
    .unwrap_or(Ok(0))
    .unwrap_or(0);

    Json(serde_json::json!({
        "mediaSizeBytes": disk_usage,
        "mediaSizeHuman": format_bytes(disk_usage),
        "dbConnections": db_connections,
    }))
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    match bytes {
        0..KB => format!("{} B", bytes),
        KB..MB => format!("{:.1} KB", bytes as f64 / KB as f64),
        MB..GB => format!("{:.1} MB", bytes as f64 / MB as f64),
        _ => format!("{:.2} GB", bytes as f64 / GB as f64),
    }
}
