use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use tokio::runtime::Runtime;

// 模拟测试 tenant_repo 的性能
// 注意：这个基准测试需要数据库连接，实际运行时需要配置 DATABASE_URL

fn benchmark_tenant_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // 注意：这些测试需要实际的数据库连接
    // 在实际使用时，需要设置 DATABASE_URL 环境变量

    c.bench_function("normalize_host", |b| {
        b.iter(|| {
            black_box(
                atmos_video_backend::repositories::tenant_repo::normalize_host(
                    "test.example.com:8080",
                ),
            )
        })
    });

    c.bench_function("cache_stats", |b| {
        b.iter(|| black_box(atmos_video_backend::repositories::tenant_repo::cache_stats()))
    });
}

criterion_group!(benches, benchmark_tenant_operations);
criterion_main!(benches);
