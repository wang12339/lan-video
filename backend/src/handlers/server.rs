use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::Json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::models::server::{
    CheckStatus, DiskUsage, HealthCheckResponse, MetricsResponse, ServerInfo,
};
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/server/info",
    tag = "server",
    summary = "Server version info",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "Server version info", body = ServerInfo))
)]
pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /health — comprehensive health check endpoint
///
/// Returns detailed health status including:
/// - Database connectivity
/// - Redis connectivity (if configured)
/// - Disk space usage
/// - System information
/// - Version information
///
/// Returns 200 if all checks pass, 503 if any critical check fails.
#[utoipa::path(
    get,
    path = "/health",
    tag = "server",
    responses(
        (status = 200, description = "All checks passed", body = HealthCheckResponse),
        (status = 503, description = "One or more checks failed", body = HealthCheckResponse)
    )
)]
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let start = Instant::now();
    let mut checks = HashMap::new();
    let mut all_ok = true;

    // Database check
    let db_start = Instant::now();
    let db_ok = state.services.admin.count_users().await.is_ok();
    let db_duration = db_start.elapsed();
    checks.insert(
        "database".to_string(),
        CheckStatus {
            status: if db_ok { "healthy" } else { "unhealthy" }.to_string(),
            message: if db_ok {
                None
            } else {
                Some("Database connection failed".to_string())
            },
            response_time_ms: Some(db_duration.as_millis() as u64),
        },
    );
    if !db_ok {
        all_ok = false;
    }

    // Redis check (if configured). A configured-but-disconnected Redis is still
    // reported, so health output distinguishes "not configured" from "down".
    if state.redis.is_configured() {
        let redis_start = Instant::now();
        let (redis_ok, redis_message) = match state.redis.resolve() {
            Some(conn) => {
                let mut conn = conn.as_ref().clone();
                match redis::cmd("PING").query_async::<String>(&mut conn).await {
                    Ok(_) => (true, None),
                    Err(e) => (false, Some(format!("Redis PING failed: {e}"))),
                }
            }
            // Configured but not connected yet: the backend retries in the
            // background, so this is a degraded (not misconfigured) state.
            None => (
                false,
                Some("Redis configured but not connected (retrying in background)".to_string()),
            ),
        };
        let redis_duration = redis_start.elapsed();
        checks.insert(
            "redis".to_string(),
            CheckStatus {
                status: if redis_ok { "healthy" } else { "unhealthy" }.to_string(),
                message: redis_message,
                response_time_ms: Some(redis_duration.as_millis() as u64),
            },
        );
        if !redis_ok {
            all_ok = false;
        }
    }

    // Disk space check
    let disk_start = Instant::now();
    let disk_info = check_disk_space(&state.config.media_root);
    let disk_duration = disk_start.elapsed();
    match disk_info {
        Ok(usage) => {
            // Consider unhealthy if disk usage > 95%
            let disk_ok = usage.usage_percent < 95.0;
            checks.insert(
                "disk".to_string(),
                CheckStatus {
                    status: if disk_ok { "healthy" } else { "warning" }.to_string(),
                    message: if disk_ok {
                        None
                    } else {
                        Some(format!("Disk usage critical: {:.1}%", usage.usage_percent))
                    },
                    response_time_ms: Some(disk_duration.as_millis() as u64),
                },
            );
            if !disk_ok {
                all_ok = false;
            }
        }
        Err(e) => {
            checks.insert(
                "disk".to_string(),
                CheckStatus {
                    status: "unhealthy".to_string(),
                    message: Some(format!("Disk check failed: {}", e)),
                    response_time_ms: Some(disk_duration.as_millis() as u64),
                },
            );
            all_ok = false;
        }
    }

    // System info
    // 侦察面收敛（渗透报告建议）：/health 是公开端点，只回最简状态。
    // 详细的 checks/system_info（内存、磁盘、版本）仅对管理员开放
    // （/admin/system-info），这里不再输出，避免公网指纹泄露。
    // 503 语义不变：任一关键检查失败仍返回 503 供 CF/监控判活。
    let _ = &checks;
    let response = HealthCheckResponse {
        status: if all_ok { "healthy" } else { "unhealthy" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        checks: HashMap::new(),
        system_info: Default::default(),
    };

    let mut headers = HeaderMap::new();
    if !state.config.public_url.is_empty() {
        if let Ok(val) = HeaderValue::try_from(state.config.public_url.as_str()) {
            headers.insert("X-Public-Url", val);
        } else {
            tracing::warn!("Invalid PUBLIC_URL value, skipping X-Public-Url header");
        }
    }
    headers.insert(
        "X-Response-Time",
        HeaderValue::from(start.elapsed().as_millis() as u64),
    );

    if all_ok {
        (StatusCode::OK, headers, Json(response)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, headers, Json(response)).into_response()
    }
}

/// Check disk space for a given path
fn check_disk_space(path: &std::path::Path) -> Result<DiskUsage, String> {
    use sysinfo::Disks;

    let disks = Disks::new_with_refreshed_list();
    // sysinfo 只列出绝对挂载点; media_root 等配置常为相对路径("./media"),
    // 必须先 canonicalize 成绝对路径再匹配, 否则永远找不到磁盘, /health 503。
    let path_str = std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();

    // 嵌套挂载时取最长前缀的挂载点, 统计最贴近该目录的文件系统
    let disk = disks
        .iter()
        .filter(|d| path_str.starts_with(d.mount_point().to_str().unwrap_or("")))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .ok_or_else(|| "Could not find disk for path".to_string())?;

    let total = disk.total_space();
    let available = disk.available_space();
    let used = total - available;
    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(DiskUsage {
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        usage_percent,
    })
}

// get_system_info 已移除：/health 不再泄露系统信息（渗透报告建议），
// 管理端由 handlers::admin::system_info 提供。

/// GET /metrics — Prometheus metrics endpoint
///
/// Returns JSON with current metrics for monitoring and alerting.
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "server",
    security(("metricsToken" = [])),
    responses((status = 200, description = "JSON metrics", body = MetricsResponse))
)]
pub async fn metrics(State(state): State<Arc<AppState>>) -> Json<MetricsResponse> {
    let metrics = &state.metrics;

    Json(MetricsResponse {
        uptime_secs: metrics.get_uptime_seconds(),
        http_requests_total: metrics.http_requests_total.get(),
        http_request_duration_seconds: metrics.http_request_duration_seconds.get_sample_sum(),
        video_views_total: metrics.video_views_total.get(),
        video_uploads_total: metrics.video_uploads_total.get(),
        video_deletes_total: metrics.video_deletes_total.get(),
        auth_login_total: metrics.auth_login_total.get(),
        auth_login_failed_total: metrics.auth_login_failed_total.get(),
        auth_register_total: metrics.auth_register_total.get(),
        auth_password_reset_total: metrics.auth_password_reset_total.get(),
        cache_hits_total: metrics.cache_hits_total.get(),
        cache_misses_total: metrics.cache_misses_total.get(),
        active_connections: metrics.http_requests_in_flight.get(),
        database_pool_size: metrics.database_pool_size.get(),
        database_pool_active: metrics.database_pool_active.get(),
    })
}

/// GET /metrics/prometheus — Prometheus text format metrics
///
/// Returns metrics in Prometheus text format for scraping.
#[utoipa::path(
    get,
    path = "/metrics/prometheus",
    tag = "server",
    security(("metricsToken" = [])),
    responses((status = 200, description = "Prometheus text exposition", content_type = "text/plain", body = String))
)]
pub async fn metrics_prometheus(State(state): State<Arc<AppState>>) -> String {
    state.metrics.encode_metrics()
}

/// GET /docs/openapi.json — OpenAPI specification
///
/// 直接序列化 `openapi::spec()` 返回的静态引用（首次调用后不再重建）。
#[utoipa::path(
    get,
    path = "/docs/openapi.json",
    tag = "server",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "OpenAPI 3.1 document", body = serde_json::Value))
)]
pub async fn openapi_spec() -> Json<&'static serde_json::Value> {
    Json(crate::openapi::spec())
}

/// GET /docs — redirect to OpenAPI spec
#[utoipa::path(
    get,
    path = "/docs",
    tag = "server",
    summary = "Redirect to the OpenAPI document",
    security(("bearerAuth" = [])),
    responses((status = 308, description = "Permanent redirect to /docs/openapi.json"))
)]
pub async fn docs_redirect() -> Redirect {
    Redirect::permanent("/docs/openapi.json")
}
