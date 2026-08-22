use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::Json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::models::server::{
    CheckStatus, DiskUsage, HealthCheckResponse, MemoryUsage, MetricsResponse, ServerInfo,
    SystemInfo,
};
use crate::state::AppState;

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
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let start = Instant::now();
    let mut checks = HashMap::new();
    let mut all_ok = true;

    // Database check
    let db_start = Instant::now();
    let db_ok = state.repos.user.count_users(1).await.is_ok();
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

    // Redis check (if configured)
    if let Some(ref redis) = state.redis {
        let redis_start = Instant::now();
        let redis_ok = redis::cmd("PING")
            .query_async::<String>(&mut redis.clone())
            .await
            .is_ok();
        let redis_duration = redis_start.elapsed();
        checks.insert(
            "redis".to_string(),
            CheckStatus {
                status: if redis_ok { "healthy" } else { "unhealthy" }.to_string(),
                message: if redis_ok {
                    None
                } else {
                    Some("Redis connection failed".to_string())
                },
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
    let system_info = get_system_info();

    // Build response
    let response = HealthCheckResponse {
        status: if all_ok { "healthy" } else { "unhealthy" }.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        checks,
        system_info,
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
    let path_str = path.to_str().unwrap_or("/");

    // Find the disk that contains the path
    let disk = disks
        .iter()
        .find(|d| path_str.starts_with(d.mount_point().to_str().unwrap_or("")))
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

/// Get system information
fn get_system_info() -> SystemInfo {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_memory();

    let memory_usage = MemoryUsage {
        total_bytes: sys.total_memory(),
        used_bytes: sys.used_memory(),
        available_bytes: sys.available_memory(),
        usage_percent: if sys.total_memory() > 0 {
            (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0
        } else {
            0.0
        },
    };

    SystemInfo {
        uptime_secs: get_uptime_seconds(),
        disk_usage: DiskUsage {
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            usage_percent: 0.0,
        },
        memory_usage: Some(memory_usage),
    }
}

/// Get system uptime in seconds
fn get_uptime_seconds() -> u64 {
    // This is a simplified version - in production you might want to use a proper uptime crate
    // For now, we'll return 0 or use process uptime
    0
}

/// GET /metrics — Prometheus metrics endpoint
///
/// Returns JSON with current metrics for monitoring and alerting.
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
        cache_hits_total: metrics.cache_hits_total.get(),
        cache_misses_total: metrics.cache_misses_total.get(),
        active_connections: metrics.active_connections.get(),
        database_pool_size: metrics.database_pool_size.get(),
        database_pool_active: metrics.database_pool_active.get(),
    })
}

/// GET /metrics/prometheus — Prometheus text format metrics
///
/// Returns metrics in Prometheus text format for scraping.
pub async fn metrics_prometheus(State(state): State<Arc<AppState>>) -> String {
    state.metrics.encode_metrics()
}

/// GET /docs/openapi.json — OpenAPI specification
pub async fn openapi_spec() -> Json<serde_json::Value> {
    Json(crate::openapi::spec())
}

/// GET /docs — redirect to OpenAPI spec
pub async fn docs_redirect() -> Redirect {
    Redirect::permanent("/docs/openapi.json")
}
