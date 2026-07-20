use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::models::video::OkResponse;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

/// POST /admin/track — 记录用户操作
pub async fn track_action(
    _state: State<Arc<AppState>>,
    axum::Extension(auth_user): axum::Extension<crate::middleware::auth::AuthUser>,
    SafeJson(req): SafeJson<TrackRequest>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, Json<ErrorResponse>)> {
    tracing::info!(
        user = %auth_user.username,
        action = %req.action,
        target = req.target.as_deref().unwrap_or(""),
        page = req.page.as_deref().unwrap_or(""),
        "用户操作"
    );
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct TrackRequest {
    pub action: String,
    pub target: Option<String>,
    pub page: Option<String>,
}

/// GET /admin/stats — 数据统计面板
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    use axum::http::StatusCode;

    let by_type = state.video_repo.count_by_type().await.map_err(|e| {
        tracing::error!("count_by_type: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    let by_category = state.video_repo.count_by_category().await.map_err(|e| {
        tracing::error!("count_by_category: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    let total_views = state.video_repo.total_views().await.map_err(|e| {
        tracing::error!("total_views: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    let total_duration = state.video_repo.total_duration_secs().await.map_err(|e| {
        tracing::error!("total_duration: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    let user_count = state.user_repo.count_users().await.map_err(|e| {
        tracing::error!("count_users: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    let pending_count = state.user_repo.count_pending_users().await.map_err(|e| {
        tracing::error!("count_pending: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

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

/// PUT /admin/config/registration — 开关注册功能
#[derive(Deserialize)]
pub struct RegistrationToggleRequest {
    pub enabled: bool,
}

pub async fn set_registration_enabled(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<RegistrationToggleRequest>,
) -> Json<OkResponse> {
    state.config.set_registration_enabled(req.enabled);
    if let Err(e) = state.registration_repo.set_enabled(req.enabled).await {
        tracing::error!("Failed to persist registration toggle: {}", e);
    }
    tracing::info!(
        enabled = req.enabled,
        "registration toggle changed by admin"
    );
    Json(OkResponse {
        ok: true,
        error: None,
        deleted: None,
    })
}

/// GET /admin/system — 系统监控
pub async fn system_info(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let db_stats = sqlx::query_as::<_, (i32,)>("SELECT count(*)::int FROM pg_stat_activity")
        .fetch_one(state.user_repo.pool())
        .await
        .unwrap_or((0,));

    // Media directory disk usage
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
        "dbConnections": db_stats.0,
        "rustLog": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        "mediaRoot": state.config.media_root,
    }))
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    if bytes < 1024 * 1024 * 1024 {
        return format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0);
    }
    format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}
