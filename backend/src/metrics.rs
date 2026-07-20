use prometheus::{Encoder, Gauge, Histogram, HistogramOpts, IntCounter, Registry, TextEncoder};
use std::time::Instant;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,

    // HTTP请求指标
    pub http_requests_total: IntCounter,
    pub http_request_duration_seconds: Histogram,
    pub http_requests_in_flight: Gauge,

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

    // 系统指标
    pub active_connections: Gauge,
    pub database_pool_size: Gauge,
    pub database_pool_active: Gauge,

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

        let video_views_total = IntCounter::new("video_views_total", "Total number of video views")
            .expect("metrics: video_views_total name collision");

        let video_uploads_total =
            IntCounter::new("video_uploads_total", "Total number of video uploads")
                .expect("metrics: video_uploads_total name collision");

        let video_deletes_total =
            IntCounter::new("video_deletes_total", "Total number of video deletions")
                .expect("metrics: video_deletes_total name collision");

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

        let cache_hits_total = IntCounter::new("cache_hits_total", "Total number of cache hits")
            .expect("metrics: cache_hits_total name collision");

        let cache_misses_total =
            IntCounter::new("cache_misses_total", "Total number of cache misses")
                .expect("metrics: cache_misses_total name collision");

        let active_connections = Gauge::new("active_connections", "Number of active connections")
            .expect("metrics: active_connections name collision");

        let database_pool_size = Gauge::new("database_pool_size", "Database connection pool size")
            .expect("metrics: database_pool_size name collision");

        let database_pool_active = Gauge::new(
            "database_pool_active",
            "Number of active database connections",
        )
        .expect("metrics: database_pool_active name collision");

        fn register_metric<T: prometheus::core::Collector + 'static>(
            registry: &Registry,
            metric: T,
            name: &str,
        ) {
            registry.register(Box::new(metric)).unwrap_or_else(|e| {
                tracing::error!("metrics: failed to register {}: {}", name, e);
            });
        }

        register_metric(
            &registry,
            http_requests_total.clone(),
            "http_requests_total",
        );
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
        register_metric(&registry, video_views_total.clone(), "video_views_total");
        register_metric(
            &registry,
            video_uploads_total.clone(),
            "video_uploads_total",
        );
        register_metric(
            &registry,
            video_deletes_total.clone(),
            "video_deletes_total",
        );
        register_metric(&registry, auth_login_total.clone(), "auth_login_total");
        register_metric(
            &registry,
            auth_login_failed_total.clone(),
            "auth_login_failed_total",
        );
        register_metric(
            &registry,
            auth_register_total.clone(),
            "auth_register_total",
        );
        register_metric(&registry, cache_hits_total.clone(), "cache_hits_total");
        register_metric(&registry, cache_misses_total.clone(), "cache_misses_total");
        register_metric(&registry, active_connections.clone(), "active_connections");
        register_metric(&registry, database_pool_size.clone(), "database_pool_size");
        register_metric(
            &registry,
            database_pool_active.clone(),
            "database_pool_active",
        );

        Metrics {
            registry,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_flight,
            video_views_total,
            video_uploads_total,
            video_deletes_total,
            auth_login_total,
            auth_login_failed_total,
            auth_register_total,
            cache_hits_total,
            cache_misses_total,
            active_connections,
            database_pool_size,
            database_pool_active,
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

    pub fn record_request(&self, duration: std::time::Duration) {
        self.http_requests_total.inc();
        self.http_request_duration_seconds
            .observe(duration.as_secs_f64());
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

    pub fn set_active_connections(&self, count: f64) {
        self.active_connections.set(count);
    }

    pub fn set_database_pool_stats(&self, size: f64, active: f64) {
        self.database_pool_size.set(size);
        self.database_pool_active.set(active);
    }

    pub fn get_uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}
