# TenantRepository 性能优化指南

## 概述

本指南详细介绍了 `tenant_repo.rs` 的性能优化措施，包括连接池优化、查询超时、重试机制和 SQL 查询优化。

## 优化措施

### 1. 连接池优化

#### 配置参数

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| 最大连接数 | `DB_MAX_CONNECTIONS` | 100 | 连接池最大连接数 |
| 最小连接数 | `DB_MIN_CONNECTIONS` | 2 | 连接池最小连接数 |
| 获取超时 | `DB_ACQUIRE_TIMEOUT_SECS` | 10秒 | 获取连接的超时时间 |
| 空闲超时 | `DB_IDLE_TIMEOUT_SECS` | 300秒 | 空闲连接的超时时间 |
| 最大生命周期 | `DB_MAX_LIFETIME_SECS` | 1800秒 | 连接的最大生命周期 |

#### 优化效果

- **连接复用**：通过连接池避免频繁创建和销毁连接
- **连接回收**：空闲超时和最大生命周期确保连接不会泄漏
- **故障恢复**：连接超时和重试机制提高系统容错能力

### 2. 查询超时机制

#### 配置参数

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| 查询超时 | `QUERY_TIMEOUT_SECS` | 5秒 | 单次查询的超时时间 |

#### 实现原理

```rust
let result = tokio::time::timeout(
    Duration::from_secs(QUERY_TIMEOUT_SECS),
    log_slow_query(label, &query_fn),
).await;
```

- 使用 `tokio::time::timeout` 包装查询
- 超时后返回 `sqlx::Error::PoolTimedOut`
- 记录超时日志用于监控

### 3. 重试机制

#### 配置参数

| 参数 | 环境变量 | 默认值 | 说明 |
|------|----------|--------|------|
| 最大重试次数 | `QUERY_MAX_RETRIES` | 3次 | 查询失败后的最大重试次数 |
| 重试基础延迟 | `QUERY_RETRY_BASE_DELAY_MS` | 100毫秒 | 重试的基础延迟时间 |

#### 重试策略

- **指数退避**：每次重试延迟时间翻倍（100ms, 200ms, 400ms...）
- **错误分类**：区分超时错误和普通错误
- **日志记录**：记录每次重试的详细信息

#### 实现示例

```rust
for attempt in 0..MAX_RETRIES {
    let result = execute_query().await;
    
    match result {
        Ok(value) => return Ok(value),
        Err(e) => {
            // 记录错误日志
            if attempt < MAX_RETRIES - 1 {
                let delay_ms = RETRY_BASE_DELAY_MS * 2u64.pow(attempt);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}
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

**优化效果**：
- 减少数据传输量
- 避免查询不需要的列
- 提高查询效率

#### 4.2 批量查询优化

**新增方法**：
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

**优化效果**：
- 减少数据库往返次数
- 批量处理提高吞吐量
- 支持缓存预热

#### 4.3 索引优化

创建迁移文件 `041_optimize_tenant_indexes.sql`：

```sql
-- 部分索引：只索引活跃租户
CREATE INDEX IF NOT EXISTS idx_tenants_slug_active ON tenants(slug) WHERE is_active = TRUE;

-- 部分索引：只索引活跃租户且 domain 不为空
CREATE INDEX IF NOT EXISTS idx_tenants_custom_domain_active ON tenants(custom_domain) 
WHERE is_active = TRUE AND custom_domain IS NOT NULL;

-- 复合索引：优化批量查询
CREATE INDEX IF NOT EXISTS idx_tenants_active_slug_domain ON tenants(is_active, slug, custom_domain);

-- 更新统计信息
ANALYZE tenants;
```

**优化效果**：
- 减少索引大小
- 提高查询速度
- 优化查询规划器决策

### 5. 缓存优化

#### 缓存策略

- **TTL 缓存**：条目存活时间 5 分钟
- **空闲超时**：条目空闲 60 秒后被驱逐
- **LRU 策略**：使用最近最少使用算法管理缓存
- **容量限制**：最多缓存 10,000 个条目

#### 缓存监控

```rust
// 获取缓存统计信息
let (hits, misses, hit_rate) = tenant_repo::cache_stats();
```

## 性能监控

### 1. 慢查询日志

使用 `log_slow_query` 函数记录超过阈值（100ms）的查询：

```rust
let result = log_slow_query("tenant_find_by_slug", || {
    sqlx::query_as::<_, Tenant>("SELECT ...").fetch_optional(&self.pool)
}).await;
```

### 2. 性能指标 API

提供 REST API 端点监控性能指标：

- `GET /admin/performance/metrics` - 获取性能指标
- `POST /admin/performance/reset` - 重置性能指标

#### 指标说明

| 指标 | 说明 |
|------|------|
| `total_queries` | 总查询次数 |
| `successful_queries` | 成功查询次数 |
| `failed_queries` | 失败查询次数 |
| `timeout_queries` | 超时查询次数 |
| `retry_queries` | 重试查询次数 |
| `avg_query_duration_ms` | 平均查询耗时（毫秒） |
| `p95_query_duration_ms` | P95 查询耗时（毫秒） |
| `p99_query_duration_ms` | P99 查询耗时（毫秒） |
| `cache_hit_rate` | 缓存命中率 |
| `success_rate` | 成功率 |
| `timeout_rate` | 超时率 |
| `retry_rate` | 重试率 |

### 3. 缓存统计

```rust
// 获取缓存统计信息
let (hits, misses, hit_rate) = tenant_repo::cache_stats();
```

## 配置建议

### 开发环境

```env
# 数据库连接池
DB_MAX_CONNECTIONS=20
DB_MIN_CONNECTIONS=2
DB_ACQUIRE_TIMEOUT_SECS=10
DB_IDLE_TIMEOUT_SECS=300
DB_MAX_LIFETIME_SECS=1800

# 查询重试
QUERY_MAX_RETRIES=2
QUERY_RETRY_BASE_DELAY_MS=50
QUERY_TIMEOUT_SECS=3

# 缓存
TENANT_CACHE_TTL_SECS=60
TENANT_CACHE_IDLE_SECS=30
TENANT_CACHE_MAX_ENTRIES=1000
```

### 生产环境

```env
# 数据库连接池
DB_MAX_CONNECTIONS=100
DB_MIN_CONNECTIONS=10
DB_ACQUIRE_TIMEOUT_SECS=15
DB_IDLE_TIMEOUT_SECS=600
DB_MAX_LIFETIME_SECS=3600

# 查询重试
QUERY_MAX_RETRIES=3
QUERY_RETRY_BASE_DELAY_MS=100
QUERY_TIMEOUT_SECS=5

# 缓存
TENANT_CACHE_TTL_SECS=300
TENANT_CACHE_IDLE_SECS=60
TENANT_CACHE_MAX_ENTRIES=10000
```

## 性能测试

### 基准测试

运行基准测试：

```bash
cd backend
cargo bench --bench tenant_performance
```

### 压力测试

使用 `wrk` 或 `ab` 进行压力测试：

```bash
# 测试租户解析性能
wrk -t12 -c400 -d30s http://localhost:8082/api/tenants/test
```

## 监控和告警

### 1. Prometheus 集成

可以将性能指标导出到 Prometheus：

```rust
// 在 metrics 端点中添加 Prometheus 格式输出
```

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