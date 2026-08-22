use prometheus::{
    Encoder, Gauge, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Registry,
    TextEncoder,
};
use std::time::Instant;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,

    // HTTP请求指标
    pub http_requests_total: IntCounter,
    pub http_request_duration_seconds: Histogram,
    pub http_requests_in_flight: Gauge,

    // 请求延迟直方图（按 method / status 细分）
    pub http_request_duration_by_route: HistogramVec,

    // 错误计数器（按 method / status_code / path 分类）
    pub http_errors_total: IntCounterVec,

    // 业务指标
    pub video_views_total: IntCounter,
    pub video_uploads_total: IntCounter,
    pub video_deletes_total: IntCounter,

    // 认证指标
    pub auth_login_total: IntCounter,
    pub auth_login_failed_total: IntCounter,
    pub auth_register_total: IntCounter,

    // 缓存指标
    pub cache_hits_total: IntCounter,
    pub cache_misses_total: IntCounter,

    // 活跃连接数指标
    pub active_connections: Gauge,
    pub active_connections_total: IntCounter, // 累计连接数
    pub idle_connections: Gauge,              // 空闲连接数

    // 数据库连接池指标
    pub database_pool_size: Gauge,
    pub database_pool_active: Gauge,
    pub database_pool_idle: Gauge,
    pub database_pool_waiting: Gauge,
    pub database_query_duration_seconds: HistogramVec, // 查询延迟（按操作分类）
    pub database_errors_total: IntCounterVec,          // 数据库错误计数

    // 开始时间
    pub start_time: Instant,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        // ── 基础 HTTP 指标 ──────────────────────────────────────────

        let http_requests_total =
            IntCounter::new("http_requests_total", "Total number of HTTP requests")
                .expect("metrics: http_requests_total name collision");

        let http_request_duration_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("metrics: http_request_duration_seconds name collision");

        let http_requests_in_flight = Gauge::new(
            "http_requests_in_flight",
            "Number of HTTP requests currently in progress",
        )
        .expect("metrics: http_requests_in_flight name collision");

        // ── 请求延迟直方图（按 method / status 细分） ────────────────

        let http_request_duration_by_route = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_by_route_seconds",
                "HTTP request duration in seconds, labeled by method and status",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
            ]),
            &["method", "status"],
        )
        .expect("metrics: http_request_duration_by_route_seconds name collision");

        // ── 错误计数器（按 method / status_code / path 分类） ────────

        let http_errors_total = IntCounterVec::new(
            prometheus::Opts::new(
                "http_errors_total",
                "Total number of HTTP error responses (status >= 400), labeled by method, status, and path",
            ),
            &["method", "status", "path"],
        )
        .expect("metrics: http_errors_total name collision");

        // ── 业务指标 ────────────────────────────────────────────────

        let video_views_total = IntCounter::new("video_views_total", "Total number of video views")
            .expect("metrics: video_views_total name collision");

        let video_uploads_total =
            IntCounter::new("video_uploads_total", "Total number of video uploads")
                .expect("metrics: video_uploads_total name collision");

        let video_deletes_total =
            IntCounter::new("video_deletes_total", "Total number of video deletions")
                .expect("metrics: video_deletes_total name collision");

        // ── 认证指标 ────────────────────────────────────────────────

        let auth_login_total =
            IntCounter::new("auth_login_total", "Total number of login attempts")
                .expect("metrics: auth_login_total name collision");

        let auth_login_failed_total = IntCounter::new(
            "auth_login_failed_total",
            "Total number of failed login attempts",
        )
        .expect("metrics: auth_login_failed_total name collision");

        let auth_register_total =
            IntCounter::new("auth_register_total", "Total number of user registrations")
                .expect("metrics: auth_register_total name collision");

        // ── 缓存指标 ────────────────────────────────────────────────

        let cache_hits_total = IntCounter::new("cache_hits_total", "Total number of cache hits")
            .expect("metrics: cache_hits_total name collision");

        let cache_misses_total =
            IntCounter::new("cache_misses_total", "Total number of cache misses")
                .expect("metrics: cache_misses_total name collision");

        // ── 连接指标 ────────────────────────────────────────────────

        let active_connections = Gauge::new(
            "active_connections",
            "Number of currently active connections",
        )
        .expect("metrics: active_connections name collision");

        let active_connections_total = IntCounter::new(
            "active_connections_total",
            "Total number of connections ever established",
        )
        .expect("metrics: active_connections_total name collision");

        let idle_connections = Gauge::new(
            "idle_connections",
            "Number of idle connections (no active request)",
        )
        .expect("metrics: idle_connections name collision");

        // ── 数据库连接池指标 ────────────────────────────────────────

        let database_pool_size = Gauge::new(
            "database_pool_size",
            "Database connection pool total size",
        )
        .expect("metrics: database_pool_size name collision");

        let database_pool_active = Gauge::new(
            "database_pool_active",
            "Number of active database connections in use",
        )
        .expect("metrics: database_pool_active name collision");

        let database_pool_idle = Gauge::new(
            "database_pool_idle",
            "Number of idle database connections in the pool",
        )
        .expect("metrics: database_pool_idle name collision");

        let database_pool_waiting = Gauge::new(
            "database_pool_waiting",
            "Number of requests waiting for a database connection",
        )
        .expect("metrics: database_pool_waiting name collision");

        let database_query_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "database_query_duration_seconds",
                "Database query duration in seconds, labeled by operation type",
            )
            .buckets(vec![
                0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0,
            ]),
            &["operation"],
        )
        .expect("metrics: database_query_duration_seconds name collision");

        let database_errors_total = IntCounterVec::new(
            prometheus::Opts::new(
                "database_errors_total",
                "Total number of database errors, labeled by operation and error type",
            ),
            &["operation", "error_type"],
        )
        .expect("metrics: database_errors_total name collision");

        // ── 注册所有指标 ────────────────────────────────────────────

        fn register_metric<T: prometheus::core::Collector + Clone + 'static>(
            registry: &Registry,
            metric: T,
            name: &str,
        ) {
            registry.register(Box::new(metric)).unwrap_or_else(|e| {
                tracing::error!("metrics: failed to register {}: {}", name, e);
            });
        }

        register_metric(&registry, http_requests_total.clone(), "http_requests_total");
        register_metric(
            &registry,
            http_request_duration_seconds.clone(),
            "http_request_duration_seconds",
        );
        register_metric(
            &registry,
            http_requests_in_flight.clone(),
            "http_requests_in_flight",
        );
        register_metric(
            &registry,
            http_request_duration_by_route.clone(),
            "http_request_duration_by_route_seconds",
        );
        register_metric(&registry, http_errors_total.clone(), "http_errors_total");
        register_metric(&registry, video_views_total.clone(), "video_views_total");
        register_metric(&registry, video_uploads_total.clone(), "video_uploads_total");
        register_metric(&registry, video_deletes_total.clone(), "video_deletes_total");
        register_metric(&registry, auth_login_total.clone(), "auth_login_total");
        register_metric(&registry, auth_login_failed_total.clone(), "auth_login_failed_total");
        register_metric(&registry, auth_register_total.clone(), "auth_register_total");
        register_metric(&registry, cache_hits_total.clone(), "cache_hits_total");
        register_metric(&registry, cache_misses_total.clone(), "cache_misses_total");
        register_metric(&registry, active_connections.clone(), "active_connections");
        register_metric(&registry, active_connections_total.clone(), "active_connections_total");
        register_metric(&registry, idle_connections.clone(), "idle_connections");
        register_metric(&registry, database_pool_size.clone(), "database_pool_size");
        register_metric(&registry, database_pool_active.clone(), "database_pool_active");
        register_metric(&registry, database_pool_idle.clone(), "database_pool_idle");
        register_metric(&registry, database_pool_waiting.clone(), "database_pool_waiting");
        register_metric(
            &registry,
            database_query_duration_seconds.clone(),
            "database_query_duration_seconds",
        );
        register_metric(&registry, database_errors_total.clone(), "database_errors_total");

        Metrics {
            registry,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_flight,
            http_request_duration_by_route,
            http_errors_total,
            video_views_total,
            video_uploads_total,
            video_deletes_total,
            auth_login_total,
            auth_login_failed_total,
            auth_register_total,
            cache_hits_total,
            cache_misses_total,
            active_connections,
            active_connections_total,
            idle_connections,
            database_pool_size,
            database_pool_active,
            database_pool_idle,
            database_pool_waiting,
            database_query_duration_seconds,
            database_errors_total,
            start_time: Instant::now(),
        }
    }

    pub fn encode_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
            tracing::error!("metrics: failed to encode: {}", e);
            return String::new();
        }
        String::from_utf8(buffer).unwrap_or_default()
    }

    /// 记录一次 HTTP 请求的延迟
    pub fn record_request(&self, duration: std::time::Duration) {
        self.http_requests_total.inc();
        self.http_request_duration_seconds
            .observe(duration.as_secs_f64());
    }

    /// 记录按 method/status 细分的请求延迟
    pub fn record_request_with_labels(&self, method: &str, status: u16, duration: std::time::Duration) {
        let status_str = status.to_string();
        self.http_request_duration_by_route
            .with_label_values(&[method, &status_str])
            .observe(duration.as_secs_f64());
    }

    /// 记录 HTTP 错误（status >= 400）
    pub fn record_error(&self, method: &str, status: u16, path: &str) {
        let status_str = status.to_string();
        // 归一化路径，去掉 ID 等动态片段，避免基数爆炸
        let normalized_path = normalize_path(path);
        self.http_errors_total
            .with_label_values(&[method, &status_str, &normalized_path])
            .inc();
    }

    /// 记录数据库查询延迟
    pub fn record_db_query(&self, operation: &str, duration: std::time::Duration) {
        self.database_query_duration_seconds
            .with_label_values(&[operation])
            .observe(duration.as_secs_f64());
    }

    /// 记录数据库错误
    pub fn record_db_error(&self, operation: &str, error_type: &str) {
        self.database_errors_total
            .with_label_values(&[operation, error_type])
            .inc();
    }

    pub fn record_video_view(&self) {
        self.video_views_total.inc();
    }

    pub fn record_video_upload(&self) {
        self.video_uploads_total.inc();
    }

    pub fn record_video_delete(&self) {
        self.video_deletes_total.inc();
    }

    pub fn record_login_success(&self) {
        self.record_login_attempt();
    }

    pub fn record_login_attempt(&self) {
        self.auth_login_total.inc();
    }

    pub fn record_login_failure(&self) {
        self.auth_login_failed_total.inc();
    }

    pub fn record_register(&self) {
        self.auth_register_total.inc();
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits_total.inc();
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses_total.inc();
    }

    /// 记录一次新连接建立
    pub fn record_connection_established(&self) {
        self.active_connections_total.inc();
    }

    pub fn set_active_connections(&self, count: f64) {
        self.active_connections.set(count);
    }

    /// 更新空闲连接数
    pub fn set_idle_connections(&self, count: f64) {
        self.idle_connections.set(count);
    }

    pub fn set_database_pool_stats(&self, size: f64, active: f64) {
        self.database_pool_size.set(size);
        self.database_pool_active.set(active);
        // 自动计算空闲连接数
        self.database_pool_idle.set(size - active);
    }

    /// 更新数据库连接池完整统计（含等待队列）
    pub fn set_database_pool_stats_full(&self, size: f64, active: f64, idle: f64, waiting: f64) {
        self.database_pool_size.set(size);
        self.database_pool_active.set(active);
        self.database_pool_idle.set(idle);
        self.database_pool_waiting.set(waiting);
    }

    pub fn get_uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

/// 归一化 URL 路径，将数字 ID 和 UUID 替换为 `:id`，防止基数爆炸。
///
/// 示例：
///   `/videos/123/comments/456` → `/videos/:id/comments/:id`
///   `/videos/550e8400-e29b-41d4-a716-446655440000` → `/videos/:id`
fn normalize_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = parts
        .iter()
        .map(|part| {
            if part.is_empty() {
                return String::new();
            }
            // 纯数字
            if part.chars().all(|c| c.is_ascii_digit()) {
                return ":id".to_string();
            }
            // UUID 格式 (8-4-4-4-12)
            if part.len() == 36
                && part.chars().enumerate().all(|(i, c)| {
                    matches!(i, 8 | 13 | 18 | 23) && c == '-' || c.is_ascii_hexdigit()
                })
            {
                return ":id".to_string();
            }
            // HashID（纯字母数字，长度 8-32）
            if part.len() >= 8
                && part.len() <= 32
                && part.chars().all(|c| c.is_ascii_alphanumeric())
                && !part.chars().all(|c| c.is_ascii_digit())
                && part.chars().any(|c| c.is_ascii_digit())
                && part.chars().any(|c| c.is_ascii_alphabetic())
            {
                return ":id".to_string();
            }
            part.to_string()
        })
        .collect();
    normalized.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        // 纯数字 ID
        assert_eq!(normalize_path("/videos/123"), "/videos/:id");
        assert_eq!(normalize_path("/videos/123/comments/456"), "/videos/:id/comments/:id");

        // UUID 格式
        assert_eq!(
            normalize_path("/videos/550e8400-e29b-41d4-a716-446655440000"),
            "/videos/:id"
        );

        // 普通路径不变
        assert_eq!(normalize_path("/auth/login"), "/auth/login");
        assert_eq!(normalize_path("/videos"), "/videos");

        // 空路径
        assert_eq!(normalize_path("/"), "/");

        // HashID（字母+数字混合）
        assert_eq!(normalize_path("/share/aBcD1234"), "/share/:id");
    }

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        // 基础字段可访问
        assert_eq!(metrics.http_requests_total.get(), 0);
        assert_eq!(metrics.active_connections.get(), 0.0);
        assert_eq!(metrics.database_pool_size.get(), 0.0);
        assert_eq!(metrics.database_pool_idle.get(), 0.0);
    }

    #[test]
    fn test_record_request_with_labels() {
        let metrics = Metrics::new();
        metrics.record_request_with_labels("GET", 200, std::time::Duration::from_millis(50));
        metrics.record_request_with_labels("POST", 500, std::time::Duration::from_millis(200));

        // 编码后应包含这些标签
        let encoded = metrics.encode_metrics();
        assert!(encoded.contains("http_request_duration_by_route_seconds"));
        assert!(encoded.contains("method=\"GET\""));
        assert!(encoded.contains("status=\"200\""));
    }

    #[test]
    fn test_record_error() {
        let metrics = Metrics::new();
        metrics.record_error("GET", 500, "/videos/123");
        metrics.record_error("POST", 404, "/auth/me");

        let encoded = metrics.encode_metrics();
        assert!(encoded.contains("http_errors_total"));
        // 路径应被归一化
        assert!(encoded.contains("path=\"/videos/:id\""));
    }

    #[test]
    fn test_database_pool_stats() {
        let metrics = Metrics::new();
        metrics.set_database_pool_stats(10.0, 7.0);
        assert_eq!(metrics.database_pool_size.get(), 10.0);
        assert_eq!(metrics.database_pool_active.get(), 7.0);
        assert_eq!(metrics.database_pool_idle.get(), 3.0);
    }

    #[test]
    fn test_record_db_query() {
        let metrics = Metrics::new();
        metrics.record_db_query("select", std::time::Duration::from_millis(10));
        metrics.record_db_query("insert", std::time::Duration::from_millis(5));

        let encoded = metrics.encode_metrics();
        assert!(encoded.contains("database_query_duration_seconds"));
        assert!(encoded.contains("operation=\"select\""));
    }
}
