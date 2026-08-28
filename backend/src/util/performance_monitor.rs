use std::collections::VecDeque;
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
    query_durations: Arc<RwLock<VecDeque<Duration>>>,
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
            query_durations: Arc::new(RwLock::new(VecDeque::with_capacity(max_duration_history))),
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
        if durations.len() >= self.max_duration_history {
            durations.pop_front();
        }
        durations.push_back(duration);
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
            let sum: Duration = durations.iter().copied().sum();
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

    fn percentile(durations: &VecDeque<Duration>, percentile: u32) -> f64 {
        if durations.is_empty() {
            return 0.0;
        }

        let mut sorted: Vec<Duration> = durations.iter().copied().collect();
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
static PERFORMANCE_MONITOR: std::sync::LazyLock<PerformanceMonitor> =
    std::sync::LazyLock::new(|| PerformanceMonitor::new(1000));

/// 获取全局性能监控器
pub fn get_performance_monitor() -> &'static PerformanceMonitor {
    &PERFORMANCE_MONITOR
}
