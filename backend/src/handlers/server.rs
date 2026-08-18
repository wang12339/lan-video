use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::Json;
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ServerInfo {
    pub version: String,
}

#[derive(Serialize)]
pub struct MetricsResponse {
    pub uptime_secs: u64,
    pub http_requests_total: u64,
    pub http_request_duration_seconds: f64,
    pub video_views_total: u64,
    pub video_uploads_total: u64,
    pub video_deletes_total: u64,
    pub auth_login_total: u64,
    pub auth_login_failed_total: u64,
    pub auth_register_total: u64,
    pub cache_hits_total: u64,
    pub cache_misses_total: u64,
    pub active_connections: f64,
    pub database_pool_size: f64,
    pub database_pool_active: f64,
}

pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /health — liveness + readiness probe
///
/// Minimal health endpoint for load balancer / k8s probes.
/// Returns 200 on success, 503 on failure. No sensitive info leaked.
/// Returns X-Public-Url header so clients can discover the server's public address.
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db_ok = state.repos.user.count_users(1).await.is_ok();

    let mut headers = HeaderMap::new();
    if !state.config.public_url.is_empty() {
        headers.insert(
            "X-Public-Url",
            HeaderValue::try_from(state.config.public_url.as_str()).unwrap(),
        );
    }

    if db_ok {
        (StatusCode::OK, headers, "ok").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, headers, "unavailable").into_response()
    }
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
