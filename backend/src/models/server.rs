use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct ServerInfo {
    pub version: String,
}

#[derive(Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
    pub checks: HashMap<String, CheckStatus>,
    pub system_info: SystemInfo,
}

#[derive(Serialize)]
pub struct CheckStatus {
    pub status: String,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct SystemInfo {
    pub uptime_secs: u64,
    pub disk_usage: DiskUsage,
    pub memory_usage: Option<MemoryUsage>,
}

#[derive(Serialize)]
pub struct DiskUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
}

#[derive(Serialize)]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
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
    pub auth_password_reset_total: u64,
    pub cache_hits_total: u64,
    pub cache_misses_total: u64,
    pub active_connections: f64,
    pub database_pool_size: f64,
    pub database_pool_active: f64,
}
