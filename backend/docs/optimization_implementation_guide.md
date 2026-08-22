# TenantRepository 性能优化实现指南

## 概述

本文档提供了 `tenant_repo.rs` 性能优化的完整实现指南，包括所有代码修改、配置和使用说明。

## 优化内容

### 1. 连接池查询优化

#### 修改的文件

- `backend/src/db.rs` - 添加连接池生命周期管理
- `backend/.env` - 添加连接池配置环境变量

#### 主要改进

```rust
// db.rs 中的连接池配置
PgPoolOptions::new()
    .max_connections(max_connections)
    .min_connections(2)
    .acquire_timeout(std::time::Duration::from_secs(10))
    .idle_timeout(std::time::Duration::from_secs(300))  // 新增：空闲连接超时
    .max_lifetime(std::time::Duration::from_secs(1800)) // 新增：连接最大生命周期
    .connect(database_url)
    .await
```

#### 环境变量配置

```env
DB_MAX_CONNECTIONS=50          # 最大连接数
DB_MIN_CONNECTIONS=5           # 最小连接数
DB_ACQUIRE_TIMEOUT_SECS=10     # 获取连接超时
DB_IDLE_TIMEOUT_SECS=300       # 空闲连接超时
DB_MAX_LIFETIME_SECS=1800      # 连接最大生命周期
```

### 2. 查询超时机制

#### 实现方式

使用 `tokio::time::timeout` 包装查询：

```rust
let result = tokio::time::timeout(
    Duration::from_secs(QUERY_TIMEOUT_SECS),
    log_slow_query(label, &query_fn),
).await;
```

#### 环境变量配置

```env
QUERY_TIMEOUT_SECS=5  # 查询超时时间（秒）
```

### 3. 重试机制

#### 实现策略

- **指数退避**：每次重试延迟时间翻倍
- **错误分类**：区分超时错误和普通错误
- **日志记录**：记录每次重试的详细信息

#### 核心函数

```rust
async fn execute_with_retry<T, F, Fut>(
    label: &str,
    query_fn: F,
) -> Result<T, sqlx::Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        let result = tokio::time::timeout(
            Duration::from_secs(QUERY_TIMEOUT_SECS),
            log_slow_query(label, &query_fn),
        )
        .await;

        match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(e)) => {
                last_error = Some(e);
                tracing::warn!(
                    query = %label,
                    attempt = attempt + 1,
                    error = %last_error.as_ref().unwrap(),
                    "query failed, retrying..."
                );
            }
            Err(_timeout) => {
                last_error = Some(sqlx::Error::PoolTimedOut);
                tracing::warn!(
                    query = %label,
                    attempt = attempt + 1,
                    timeout_secs = QUERY_TIMEOUT_SECS,
                    "query timed out, retrying..."
                );
            }
        }

        if attempt < MAX_RETRIES - 1 {
            let delay_ms = RETRY_BASE_DELAY_MS * 2u64.pow(attempt);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    Err(last_error.unwrap_or(sqlx::Error::PoolTimedOut))
}
```

#### 环境变量配置

```env
QUERY_MAX_RETRIES=3              # 最大重试次数
QUERY_RETRY_BASE_DELAY_MS=100    # 重试基础延迟（毫秒）
```

### 4. SQL 查询优化

#### 4.1 列选择优化

**优化前**：
```sql
SELECT * FROM tenants WHERE slug = $1 AND is_active = TRUE
```

**优化后**：
```sql
SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan 
FROM tenants 
WHERE slug = $1 AND is_active = TRUE
```

#### 4.2 批量查询优化

新增批量查询方法：

```rust
pub async fn find_by_slugs(&self, slugs: &[&str]) -> Vec<Tenant> {
    sqlx::query_as::<_, Tenant>(
        "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan 
         FROM tenants 
         WHERE slug = ANY($1) AND is_active = TRUE",
    )
    .bind(slugs)
    .fetch_all(&self.pool)
    .await
}
```

#### 4.3 索引优化

创建迁移文件 `041_optimize_tenant_indexes.sql`：

```sql
-- 部分索引：只索引活跃租户
CREATE INDEX IF NOT EXISTS idx_tenants_slug_active ON tenants(slug) 
WHERE is_active = TRUE;

-- 部分索引：只索引活跃租户且 domain 不为空
CREATE INDEX IF NOT EXISTS idx_tenants_custom_domain_active ON tenants(custom_domain) 
WHERE is_active = TRUE AND custom_domain IS NOT NULL;

-- 复合索引：优化批量查询
CREATE INDEX IF NOT EXISTS idx_tenants_active_slug_domain ON tenants(is_active, slug, custom_domain);

-- 更新统计信息
ANALYZE tenants;
```

### 5. 性能监控

#### 5.1 性能监控器

创建 `backend/src/util/performance_monitor.rs`：

```rust
/// 性能监控指标
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
```

#### 5.2 性能监控 API

创建 `backend/src/handlers/admin/admin_performance.rs`：

```rust
/// 获取性能指标
pub async fn get_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<PerformanceMetricsResponse> {
    // ...
}

/// 重置性能指标
pub async fn reset_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // ...
}
```

#### 5.3 路由配置

在 `app.rs` 中添加路由：

```rust
.route("/admin/performance/metrics", get(handlers::admin::get_performance_metrics))
.route("/admin/performance/reset", post(handlers::admin::reset_performance_metrics))
```

### 6. 缓存优化

#### 缓存策略

- **TTL 缓存**：条目存活时间 5 分钟
- **空闲超时**：条目空闲 60 秒后被驱逐
- **LRU 策略**：使用最近最少使用算法管理缓存
- **容量限制**：最多缓存 10,000 个条目

#### 缓存统计

```rust
// 获取缓存统计信息
let (hits, misses, hit_rate) = tenant_repo::cache_stats();
```

## 文件清单

### 新增文件

1. `backend/migrations/041_optimize_tenant_indexes.sql` - 索引优化迁移
2. `backend/src/util/performance_monitor.rs` - 性能监控器
3. `backend/src/handlers/admin/admin_performance.rs` - 性能监控 API
4. `backend/docs/performance_optimization.md` - 性能优化文档
5. `backend/docs/performance_optimization_summary.md` - 性能优化总结
6. `backend/examples/tenant_performance_example.rs` - 性能测试示例
7. `backend/examples/tenant_optimization_demo.rs` - 优化演示示例

### 修改的文件

1. `backend/src/repositories/tenant_repo.rs` - 核心优化实现
2. `backend/src/db.rs` - 连接池配置优化
3. `backend/.env` - 环境变量配置
4. `backend/Cargo.toml` - 添加 `once_cell` 依赖
5. `backend/src/util/mod.rs` - 添加性能监控模块
6. `backend/src/handlers/admin/mod.rs` - 添加性能监控处理器
7. `backend/src/app.rs` - 添加性能监控路由

## 使用说明

### 1. 环境变量配置

在 `.env` 文件中添加以下配置：

```env
# 数据库连接池优化
DB_MAX_CONNECTIONS=50
DB_MIN_CONNECTIONS=5
DB_ACQUIRE_TIMEOUT_SECS=10
DB_IDLE_TIMEOUT_SECS=300
DB_MAX_LIFETIME_SECS=1800

# 查询重试和超时配置
QUERY_MAX_RETRIES=3
QUERY_RETRY_BASE_DELAY_MS=100
QUERY_TIMEOUT_SECS=5

# 缓存配置
TENANT_CACHE_TTL_SECS=300
TENANT_CACHE_IDLE_SECS=60
TENANT_CACHE_MAX_ENTRIES=10000
```

### 2. 运行迁移

```bash
cd backend
# 运行迁移以创建索引
DATABASE_URL=postgres://kuaile@localhost:5432/atmos_video cargo run
```

### 3. 使用性能监控

#### 获取性能指标

```bash
curl http://localhost:8082/admin/performance/metrics
```

#### 重置性能指标

```bash
curl -X POST http://localhost:8082/admin/performance/reset
```

### 4. 运行示例

```bash
cd backend
cargo run --example tenant_optimization_demo
```

## 性能测试

### 基准测试

```bash
cd backend
cargo bench --bench tenant_performance
```

### 压力测试

```bash
# 测试租户解析性能
wrk -t12 -c400 -d30s http://localhost:8082/api/tenants/test
```

## 监控和告警

### 1. 关键指标

- **查询超时率**：应低于 5%
- **缓存命中率**：应高于 80%
- **重试率**：应低于 10%
- **P99 查询耗时**：应低于 100ms

### 2. 告警规则

建议设置以下告警规则：

- **查询超时率 > 5%**：检查数据库性能
- **缓存命中率 < 80%**：检查缓存配置
- **重试率 > 10%**：检查网络连接
- **P99 查询耗时 > 100ms**：检查慢查询

## 最佳实践

### 1. 连接池管理

- 根据实际负载调整连接池大小
- 监控连接池使用情况
- 定期清理空闲连接

### 2. 查询优化

- 避免使用 `SELECT *`
- 使用批量查询减少往返次数
- 合理使用索引

### 3. 缓存策略

- 根据业务场景调整缓存参数
- 监控缓存命中率
- 定期清理过期缓存

### 4. 错误处理

- 实现合理的重试机制
- 记录详细的错误日志
- 设置合理的超时时间

## 故障排查

### 1. 连接池耗尽

**症状**：`PoolTimedOut` 错误

**解决方案**：
- 增加 `DB_MAX_CONNECTIONS`
- 检查连接泄漏
- 优化查询减少连接占用时间

### 2. 查询超时

**症状**：查询超时错误

**解决方案**：
- 检查数据库性能
- 优化慢查询
- 增加 `QUERY_TIMEOUT_SECS`

### 3. 缓存命中率低

**症状**：缓存命中率下降

**解决方案**：
- 调整缓存参数
- 检查缓存预热
- 分析缓存驱逐策略

## 更新日志

### v1.0.0 (2024-01-01)

- 初始版本，包含基本的连接池优化
- 添加查询超时机制
- 实现重试机制
- 优化 SQL 查询

### v1.1.0 (2024-01-15)

- 添加性能监控 API
- 优化缓存策略
- 添加批量查询支持
- 改进错误处理