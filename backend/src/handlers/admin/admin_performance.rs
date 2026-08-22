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

/// 获取性能指标
pub async fn get_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<PerformanceMetricsResponse> {
    let monitor = get_performance_monitor();
    let metrics = monitor.get_metrics().await;

    let success_rate = if metrics.total_queries > 0 {
        metrics.successful_queries as f64 / metrics.total_queries as f64
    } else {
        0.0
    };

    let timeout_rate = if metrics.total_queries > 0 {
        metrics.timeout_queries as f64 / metrics.total_queries as f64
    } else {
        0.0
    };

    let retry_rate = if metrics.total_queries > 0 {
        metrics.retry_queries as f64 / metrics.total_queries as f64
    } else {
        0.0
    };

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
        success_rate,
        timeout_rate,
        retry_rate,
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
