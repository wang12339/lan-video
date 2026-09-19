use axum::{extract::State, http::StatusCode, Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::admin::{RegistrationToggleRequest, TrackRequest};
use crate::models::video::OkResponse;
use crate::state::AppState;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

/// 清洗用户可控的追踪字段：剔除控制字符（换行/回车/制表符/ANSI 转义引导符
/// 等，否则可伪造日志行或注入终端控制序列），并截断到 200 字符。
fn sanitize_log_field(s: &str) -> String {
    const MAX_CHARS: usize = 200;
    let mut out: String = s
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_CHARS)
        .collect();
    if s.chars().count() > MAX_CHARS {
        out.push('…');
    }
    out
}

/// 共享埋点写入：按用户限速 + 字段清洗 + tracing。
///
/// 每用户 120 次/60s，超限静默丢弃（返回 204，避免前端重试风暴）。
async fn record_track(state: &AppState, auth_user: &AuthUser, req: &TrackRequest) -> StatusCode {
    if state
        .rate_limiter
        .check_with(&format!("track:{}", auth_user.id), 120, 60, 0)
        .await
        .is_err()
    {
        return StatusCode::NO_CONTENT;
    }
    tracing::info!(
        user = %auth_user.username,
        action = %sanitize_log_field(&req.action),
        target = %sanitize_log_field(req.target.as_deref().unwrap_or("")),
        page = %sanitize_log_field(req.page.as_deref().unwrap_or("")),
        "用户操作"
    );
    StatusCode::NO_CONTENT
}

/// POST /admin/track — 记录用户操作（历史路径，保留兼容旧客户端）
#[utoipa::path(
    post,
    path = "/admin/track",
    tag = "admin",
    description = "记录用户操作日志（页面、动作、目标），仅返回 204",
    security(("bearerAuth" = [])),
    request_body = TrackRequest,
    responses(
        (status = 204, description = "Action recorded"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn track_action(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(req): SafeJson<TrackRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    Ok(record_track(&state, &auth_user, &req).await)
}

/// POST /track — 记录用户操作（普通用户埋点路径，避免 /admin 命名空间混淆）
#[utoipa::path(
    post,
    path = "/track",
    tag = "user",
    description = "记录用户操作日志（页面、动作、目标），仅返回 204；按用户限速",
    security(("bearerAuth" = [])),
    request_body = TrackRequest,
    responses(
        (status = 204, description = "Action recorded"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn track_action_public(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    SafeJson(req): SafeJson<TrackRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    Ok(record_track(&state, &auth_user, &req).await)
}

/// GET /admin/stats — 数据统计面板
#[utoipa::path(
    get,
    path = "/admin/stats",
    tag = "admin",
    description = "返回视频/图片/用户数量、总播放量与观看时长、类型与分类分布等统计数据",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "Dashboard stats", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let by_type = state
        .repos
        .video
        .count_by_type()
        .await
        .map_err(|e| internal_error_log("count_by_type", &e))?;
    let by_category = state
        .repos
        .video
        .count_by_category()
        .await
        .map_err(|e| internal_error_log("count_by_category", &e))?;
    let total_views = state
        .repos
        .video
        .total_views()
        .await
        .map_err(|e| internal_error_log("total_views", &e))?;
    let total_duration = state
        .repos
        .video
        .total_duration_secs()
        .await
        .map_err(|e| internal_error_log("total_duration", &e))?;
    let user_count = state
        .repos
        .user
        .count_users()
        .await
        .map_err(|e| internal_error_log("count_users", &e))?;
    let pending_count = state
        .repos
        .user
        .count_pending_users()
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
#[utoipa::path(
    get,
    path = "/admin/config/registration",
    tag = "admin",
    description = "查询注册开关的当前状态",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "Registration state", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_registration_enabled(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "enabled": state.config.registration_enabled(),
    }))
}

#[utoipa::path(
    put,
    path = "/admin/config/registration",
    tag = "admin",
    summary = "Toggle public registration",
    description = "开启/关闭公开注册（持久化到数据库）",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = RegistrationToggleRequest,
    responses(
        (status = 200, description = "Registration toggle updated", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
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

#[utoipa::path(
    get,
    path = "/admin/system",
    tag = "admin",
    summary = "System monitoring info",
    description = "返回媒体目录磁盘占用与数据库连接数等系统监控信息",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "System info", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
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

#[cfg(test)]
mod tests {
    use super::sanitize_log_field;

    #[test]
    fn sanitize_log_field_strips_control_chars() {
        assert_eq!(sanitize_log_field("view\nvideo"), "viewvideo");
        assert_eq!(sanitize_log_field("a\r\tb"), "ab");
        // ANSI 转义由 ESC 控制字符驱动，去掉 ESC 后序列失效
        assert_eq!(sanitize_log_field("\u{1b}[31mred"), "[31mred");
        assert_eq!(sanitize_log_field("用户"), "用户");
    }

    #[test]
    fn sanitize_log_field_truncates_to_200_chars() {
        let long = "x".repeat(250);
        let sanitized = sanitize_log_field(&long);
        assert_eq!(sanitized.chars().count(), 201);
        assert!(sanitized.ends_with('…'));

        let exact = "x".repeat(200);
        assert_eq!(sanitize_log_field(&exact).chars().count(), 200);
    }
}
