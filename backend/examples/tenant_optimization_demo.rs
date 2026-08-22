//! TenantRepository 性能优化演示
//! 
//! 本示例展示了如何使用优化后的 TenantRepository 进行租户查询操作，
//! 包括连接池优化、查询超时、重试机制和 SQL 查询优化。

use atmos_video_backend::repositories::tenant_repo::TenantRepository;
use atmos_video_backend::util::performance_monitor::get_performance_monitor;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 从环境变量获取数据库连接字符串
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://kuaile@localhost:5432/atmos_video".to_string());

    println!("=== TenantRepository 性能优化演示 ===\n");

    // 1. 创建优化的连接池
    println!("1. 创建优化的连接池...");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(std::time::Duration::from_secs(300))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect(&database_url)
        .await?;

    // 2. 创建租户仓库
    let tenant_repo = TenantRepository::new(pool);

    // 3. 预热缓存
    println!("2. 预热租户缓存...");
    let start = Instant::now();
    tenant_repo.warm_cache().await;
    let duration = start.elapsed();
    println!("   缓存预热完成，耗时: {:?}\n", duration);

    // 4. 获取性能监控器
    let monitor = get_performance_monitor();

    // 5. 测试批量查询
    println!("3. 测试批量查询优化...");
    let slugs = vec!["default", "test", "demo"];
    let start = Instant::now();
    let tenants = tenant_repo.find_by_slugs(&slugs).await;
    let duration = start.elapsed();
    println!("   批量查询 {} 个 slug，找到 {} 个租户，耗时: {:?}", 
             slugs.len(), tenants.len(), duration);

    // 6. 测试单个查询（带重试和超时）
    println!("\n4. 测试单个查询（带重试和超时）...");
    let start = Instant::now();
    let tenant = tenant_repo.find_by_slug("default").await;
    let duration = start.elapsed();
    
    if let Some(tenant) = tenant {
        println!("   找到租户: {} (ID: {}, 计划: {})", tenant.name, tenant.id, tenant.plan);
    } else {
        println!("   未找到租户");
    }
    println!("   查询耗时: {:?}", duration);

    // 7. 测试域名查询
    println!("\n5. 测试域名查询...");
    let start = Instant::now();
    let tenant = tenant_repo.find_by_domain("atmos.whanghui.top").await;
    let duration = start.elapsed();
    
    if let Some(tenant) = tenant {
        println!("   通过域名找到租户: {} (ID: {}, 计划: {})", tenant.name, tenant.id, tenant.plan);
    } else {
        println!("   未通过域名找到租户");
    }
    println!("   查询耗时: {:?}", duration);

    // 8. 测试主机解析
    println!("\n6. 测试主机解析（带缓存）...");
    let hosts = vec![
        "localhost",
        "atmos.whanghui.top",
        "test.atmos.whanghui.top",
    ];

    for host in hosts {
        let start = Instant::now();
        let context = tenant_repo.resolve_from_host(host).await;
        let duration = start.elapsed();
        
        if let Some(context) = context {
            println!("   主机 '{}' -> 租户 ID: {}, slug: '{}'", 
                     host, context.tenant_id, context.slug);
        } else {
            println!("   主机 '{}' -> 未解析", host);
        }
        println!("   解析耗时: {:?}", duration);
    }

    // 9. 获取性能指标
    println!("\n7. 性能指标:");
    let metrics = monitor.get_metrics().await;
    println!("   总查询次数: {}", metrics.total_queries);
    println!("   成功查询次数: {}", metrics.successful_queries);
    println!("   失败查询次数: {}", metrics.failed_queries);
    println!("   超时查询次数: {}", metrics.timeout_queries);
    println!("   重试查询次数: {}", metrics.retry_queries);
    println!("   平均查询耗时: {:.2} ms", metrics.avg_query_duration_ms);
    println!("   P95 查询耗时: {:.2} ms", metrics.p95_query_duration_ms);
    println!("   P99 查询耗时: {:.2} ms", metrics.p99_query_duration_ms);
    println!("   缓存命中率: {:.2}%", metrics.cache_hit_rate * 100.0);

    // 10. 获取缓存统计
    println!("\n8. 缓存统计:");
    let (hits, misses, hit_rate) = atmos_video_backend::repositories::tenant_repo::cache_stats();
    println!("   缓存命中次数: {}", hits);
    println!("   缓存未命中次数: {}", misses);
    println!("   缓存命中率: {:.2}%", hit_rate * 100.0);

    // 11. 优化建议
    println!("\n=== 优化建议 ===");
    println!("1. 根据实际负载调整连接池大小");
    println!("2. 监控查询超时率，必要时调整超时时间");
    println!("3. 根据缓存命中率调整缓存参数");
    println!("4. 定期检查慢查询日志");
    println!("5. 使用批量查询减少数据库往返次数");

    Ok(())
}