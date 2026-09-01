//! 租户性能优化示例
//!
//! 本示例展示了如何使用优化后的 TenantRepository 进行租户查询操作。

use atmos_video_backend::repositories::tenant_repo::TenantRepository;
use atmos_video_backend::util::performance_monitor::get_performance_monitor;
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 从环境变量获取数据库连接字符串
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://kuaile@localhost:5432/atmos_video".to_string());

    // 创建连接池
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&database_url)
        .await?;

    // 创建租户仓库
    let tenant_repo = TenantRepository::new(pool, "http://localhost:8082".to_string());

    // 预热缓存
    println!("预热租户缓存...");
    tenant_repo.warm_cache().await;

    // 获取性能监控器
    let monitor = get_performance_monitor();

    // 测试批量查询
    println!("\n测试批量查询...");
    let slugs = vec!["default", "test", "demo"];
    let tenants = tenant_repo.find_by_slugs(&slugs).await;
    println!("找到 {} 个租户", tenants.len());

    // 测试单个查询
    println!("\n测试单个查询...");
    let start = std::time::Instant::now();
    let tenant = tenant_repo.find_by_slug("default").await;
    let duration = start.elapsed();

    if let Some(tenant) = tenant {
        println!("找到租户: {} (耗时: {:?})", tenant.name, duration);
    } else {
        println!("未找到租户 (耗时: {:?})", duration);
    }

    // 测试域名查询
    println!("\n测试域名查询...");
    let start = std::time::Instant::now();
    let tenant = tenant_repo.find_by_domain("atmos.whanghui.top").await;
    let duration = start.elapsed();

    if let Some(tenant) = tenant {
        println!("通过域名找到租户: {} (耗时: {:?})", tenant.name, duration);
    } else {
        println!("未通过域名找到租户 (耗时: {:?})", duration);
    }

    // 获取性能指标
    println!("\n性能指标:");
    let metrics = monitor.get_metrics().await;
    println!("总查询次数: {}", metrics.total_queries);
    println!("成功查询次数: {}", metrics.successful_queries);
    println!("失败查询次数: {}", metrics.failed_queries);
    println!("超时查询次数: {}", metrics.timeout_queries);
    println!("重试查询次数: {}", metrics.retry_queries);
    println!("平均查询耗时: {:.2} ms", metrics.avg_query_duration_ms);
    println!("P95 查询耗时: {:.2} ms", metrics.p95_query_duration_ms);
    println!("P99 查询耗时: {:.2} ms", metrics.p99_query_duration_ms);
    println!("缓存命中率: {:.2}%", metrics.cache_hit_rate * 100.0);

    // 获取缓存统计
    let (hits, misses, hit_rate) = atmos_video_backend::repositories::tenant_repo::cache_stats();
    println!("\n缓存统计:");
    println!("缓存命中次数: {}", hits);
    println!("缓存未命中次数: {}", misses);
    println!("缓存命中率: {:.2}%", hit_rate * 100.0);

    Ok(())
}
