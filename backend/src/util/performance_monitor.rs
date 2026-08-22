use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 性能监控指标
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub timeout_queries: u64,
    pub retry_queries: u64,
    pub avg_query_duration_ms: f64,
    pub p95_query_duration_ms: f64,
    pub p99_query_duration_ms: f64,
    pub cache_hit_rate: f64,
}

/// 性能监控器
pub struct PerformanceMonitor {
    total_queries: AtomicU64,
    successful_queries: AtomicU64,
    failed_queries: AtomicU64,
    timeout_queries: AtomicU64,
    retry_queries: AtomicU64,
    query_durations: Arc<RwLock<Vec<Duration>>>,
    max_duration_history: usize,
}

impl PerformanceMonitor {
    pub fn new(max_duration_history: usize) -> Self {
        Self {
            total_queries: AtomicU64::new(0),
            successful_queries: AtomicU64::new(0),
            failed_queries: AtomicU64::new(0),
            timeout_queries: AtomicU64::new(0),
            retry_queries: AtomicU64::new(0),
            query_durations: Arc::new(RwLock::new(Vec::new())),
            max_duration_history,
        }
    }

    /// 记录查询开始
    pub fn record_query_start(&self) -> Instant {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        Instant::now()
    }

    /// 记录查询成功
    pub async fn record_query_success(&self, start: Instant) {
        let duration = start.elapsed();
        self.successful_queries.fetch_add(1, Ordering::Relaxed);
        self.record_duration(duration).await;
    }

    /// 记录查询失败
    pub async fn record_query_failure(&self, start: Instant) {
        let duration = start.elapsed();
        self.failed_queries.fetch_add(1, Ordering::Relaxed);
        self.record_duration(duration).await;
    }

    /// 记录查询超时
    pub async fn record_query_timeout(&self, start: Instant) {
        let duration = start.elapsed();
        self.timeout_queries.fetch_add(1, Ordering::Relaxed);
        self.record_duration(duration).await;
    }

    /// 记录查询重试
    pub fn record_query_retry(&self) {
        self.retry_queries.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录查询持续时间
    async fn record_duration(&self, duration: Duration) {
        let mut durations = self.query_durations.write().await;
        durations.push(duration);

        // 保持历史记录在限制内
        if durations.len() > self.max_duration_history {
            durations.remove(0);
        }
    }

    /// 获取性能指标
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let durations = self.query_durations.read().await;
        let total = self.total_queries.load(Ordering::Relaxed);
        let successful = self.successful_queries.load(Ordering::Relaxed);
        let failed = self.failed_queries.load(Ordering::Relaxed);
        let timeouts = self.timeout_queries.load(Ordering::Relaxed);
        let retries = self.retry_queries.load(Ordering::Relaxed);

        let avg_duration_ms = if !durations.is_empty() {
            let sum: Duration = durations.iter().sum();
            sum.as_millis() as f64 / durations.len() as f64
        } else {
            0.0
        };

        let p95_duration_ms = Self::percentile(&durations, 95);
        let p99_duration_ms = Self::percentile(&durations, 99);

        let cache_hit_rate = if total > 0 {
            successful as f64 / total as f64
        } else {
            0.0
        };

        PerformanceMetrics {
            total_queries: total,
            successful_queries: successful,
            failed_queries: failed,
            timeout_queries: timeouts,
            retry_queries: retries,
            avg_query_duration_ms: avg_duration_ms,
            p95_query_duration_ms: p95_duration_ms,
            p99_query_duration_ms: p99_duration_ms,
            cache_hit_rate,
        }
    }

    /// 计算百分位数
    fn percentile(durations: &[Duration], percentile: u32) -> f64 {
        if durations.is_empty() {
            return 0.0;
        }

        let mut sorted = durations.to_vec();
        sorted.sort();

        let index = (percentile as f64 / 100.0 * sorted.len() as f64) as usize;
        let index = index.min(sorted.len() - 1);

        sorted[index].as_millis() as f64
    }

    /// 重置所有指标
    pub fn reset(&self) {
        self.total_queries.store(0, Ordering::Relaxed);
        self.successful_queries.store(0, Ordering::Relaxed);
        self.failed_queries.store(0, Ordering::Relaxed);
        self.timeout_queries.store(0, Ordering::Relaxed);
        self.retry_queries.store(0, Ordering::Relaxed);
        // 注意：query_durations 需要异步重置，这里简化处理
    }
}

/// 性能监控器单例
static PERFORMANCE_MONITOR: once_cell::sync::Lazy<PerformanceMonitor> =
    once_cell::sync::Lazy::new(|| PerformanceMonitor::new(1000));

/// 获取全局性能监控器
pub fn get_performance_monitor() -> &'static PerformanceMonitor {
    &PERFORMANCE_MONITOR
}

/// 性能监控包装器
pub struct MonitoredQuery<F> {
    query_fn: F,
    label: String,
    monitor: &'static PerformanceMonitor,
}

impl<F, Fut, T> MonitoredQuery<F>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    pub fn new(label: &str, query_fn: F) -> Self {
        Self {
            query_fn,
            label: label.to_string(),
            monitor: get_performance_monitor(),
        }
    }

    pub async fn execute(self) -> Result<T, sqlx::Error> {
        let start = self.monitor.record_query_start();

        match (self.query_fn)().await {
            Ok(result) => {
                self.monitor.record_query_success(start).await;
                Ok(result)
            }
            Err(e) => {
                self.monitor.record_query_failure(start).await;
                Err(e)
            }
        }
    }
}