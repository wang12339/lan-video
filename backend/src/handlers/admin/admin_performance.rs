use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;
use crate::util::performance_monitor::get_performance_monitor;

/// 性能指标响应
#[derive(Debug, Serialize)]
pub struct PerformanceMetricsResponse {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub timeout_queries: u64,
    pub retry_queries: u64,
    pub avg_query_duration_ms: f64,
    pub p95_query_duration_ms: f64,
    pub p99_query_duration_ms: f64,
    pub cache_hit_rate: f64,
    pub success_rate: f64,
    pub timeout_rate: f64,
    pub retry_rate: f64,
}

pub async fn get_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<PerformanceMetricsResponse> {
    let monitor = get_performance_monitor();
    let metrics = monitor.get_metrics().await;

    let total = metrics.total_queries as f64;
    let safe_div = |n: u64| if total > 0.0 { n as f64 / total } else { 0.0 };

    Json(PerformanceMetricsResponse {
        total_queries: metrics.total_queries,
        successful_queries: metrics.successful_queries,
        failed_queries: metrics.failed_queries,
        timeout_queries: metrics.timeout_queries,
        retry_queries: metrics.retry_queries,
        avg_query_duration_ms: metrics.avg_query_duration_ms,
        p95_query_duration_ms: metrics.p95_query_duration_ms,
        p99_query_duration_ms: metrics.p99_query_duration_ms,
        cache_hit_rate: metrics.cache_hit_rate,
        success_rate: safe_div(metrics.successful_queries),
        timeout_rate: safe_div(metrics.timeout_queries),
        retry_rate: safe_div(metrics.retry_queries),
    })
}

/// 重置性能指标
pub async fn reset_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let monitor = get_performance_monitor();
    monitor.reset();

    Json(serde_json::json!({
        "success": true,
        "message": "Performance metrics reset successfully"
    }))
}
