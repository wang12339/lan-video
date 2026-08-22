use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db::log_slow_query;
use crate::middleware::tenant::{TenantContext, TenantStatus};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Tenant {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub custom_domain: Option<String>,
    pub is_active: bool,
    pub max_users: i32,
    pub max_storage_bytes: i64,
    pub plan: String,
    pub settings: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantConfig {
    pub tenant_id: i64,
    pub slug: String,
    pub name: String,
    pub host: String,
    pub status: String,
    pub plan: String,
    pub max_users: i32,
    pub max_storage_bytes: i64,
    pub settings: TenantSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantSettings {
    pub max_upload_size_mb: u64,
    pub max_videos_per_user: u32,
    pub registration_enabled: bool,
    pub custom_theme: Option<String>,
    pub storage_quota_gb: u64,
}

/// `tenant_usage_stats` 视图的行类型
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TenantUsageStatsRow {
    pub tenant_id: i64,
    pub slug: String,
    pub name: String,
    pub user_count: i64,
    pub video_count: i64,
    pub storage_used_bytes: f64,
    pub max_storage_bytes: i64,
    pub storage_usage_percent: f64,
}

#[derive(Clone)]
pub struct TenantRepository {
    pool: PgPool,
}

/// 查询超时时间（秒）
const QUERY_TIMEOUT_SECS: u64 = 5;
/// 最大重试次数
const MAX_RETRIES: u32 = 3;
/// 重试基础延迟（毫秒）
const RETRY_BASE_DELAY_MS: u64 = 100;

/// Cache TTL: 5 minutes (300 seconds)
const TENANT_CACHE_TTL_SECS: u64 = 300;
/// Cache idle timeout: 60 seconds (entries unused for 60s are evicted)
const TENANT_CACHE_IDLE_SECS: u64 = 60;
/// LRU max capacity: 10,000 entries
const TENANT_CACHE_MAX_ENTRIES: u64 = 10_000;
const MAX_HOST_LEN: usize = 255;

static TENANT_CACHE: OnceLock<Cache<String, Option<TenantContext>>> = OnceLock::new();

/// Cache hit/miss statistics
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

/// Build the global tenant cache with TTL, idle timeout (LRU eviction), and capacity limits.
fn tenant_cache() -> &'static Cache<String, Option<TenantContext>> {
    TENANT_CACHE.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(TENANT_CACHE_TTL_SECS))
            .time_to_idle(Duration::from_secs(TENANT_CACHE_IDLE_SECS))
            .max_capacity(TENANT_CACHE_MAX_ENTRIES)
            .build()
    })
}

/// Record a cache hit.
fn record_cache_hit() {
    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

/// Record a cache miss.
fn record_cache_miss() {
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
}

/// Get current cache hit/miss stats as (hits, misses, hit_rate).
pub fn cache_stats() -> (u64, u64, f64) {
    let hits = CACHE_HITS.load(Ordering::Relaxed);
    let misses = CACHE_MISSES.load(Ordering::Relaxed);
    let total = hits + misses;
    let hit_rate = if total > 0 {
        hits as f64 / total as f64
    } else {
        0.0
    };
    (hits, misses, hit_rate)
}

/// 执行带超时和重试的数据库查询
async fn execute_with_retry<T, F, Fut>(label: &str, query_fn: F) -> Result<T, sqlx::Error>
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

impl TenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Warm up the tenant cache by pre-loading the default tenant and all active tenants.
    /// Call this at application startup to avoid cold-start latency.
    pub async fn warm_cache(&self) {
        tracing::info!("warming tenant cache");

        // Pre-load default tenant
        if let Some(default) = self.default_context().await {
            tenant_cache().insert("default".to_string(), Some(default));
        }

        // Pre-load all active tenants by their custom domains and slugs
        // 优化：使用具体列名代替 *，避免查询不需要的列
        let result = execute_with_retry("tenant_warm_cache", || {
            sqlx::query_as::<_, Tenant>(
                "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan, settings
                 FROM tenants 
                 WHERE is_active = TRUE",
            )
            .fetch_all(&self.pool)
        })
        .await;

        match result {
            Ok(tenants) => {
                for tenant in &tenants {
                    let ctx = Some(Self::to_context(tenant));
                    // Cache by slug
                    tenant_cache().insert(tenant.slug.clone(), ctx.clone());
                    // Cache by custom domain if present
                    if let Some(ref domain) = tenant.custom_domain {
                        let normalized = normalize_host(domain);
                        if !normalized.is_empty() {
                            tenant_cache().insert(normalized, ctx);
                        }
                    }
                }
                tracing::info!(count = tenants.len(), "tenant cache warmed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to warm tenant cache");
            }
        }
    }

    pub async fn find_by_slug(&self, slug: &str) -> Option<Tenant> {
        let result = execute_with_retry("tenant_find_by_slug", || {
            sqlx::query_as::<_, Tenant>(
                "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan, settings
                 FROM tenants 
                 WHERE slug = $1 AND is_active = TRUE",
            )
            .bind(slug)
            .fetch_optional(&self.pool)
        })
        .await;

        result.unwrap_or_else(|e| {
            tracing::warn!(%slug, error = %e, "tenant find_by_slug failed after retries");
            None
        })
    }

    pub async fn find_by_domain(&self, domain: &str) -> Option<Tenant> {
        let result = execute_with_retry("tenant_find_by_domain", || {
            sqlx::query_as::<_, Tenant>(
                "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan, settings
                 FROM tenants 
                 WHERE custom_domain = $1 AND is_active = TRUE",
            )
            .bind(domain)
            .fetch_optional(&self.pool)
        })
        .await;

        result.unwrap_or_else(|e| {
            tracing::warn!(%domain, error = %e, "tenant find_by_domain failed after retries");
            None
        })
    }

    /// 批量预加载租户信息（优化批量查询场景）
    pub async fn find_by_slugs(&self, slugs: &[&str]) -> Vec<Tenant> {
        if slugs.is_empty() {
            return Vec::new();
        }

        let result = execute_with_retry("tenant_find_by_slugs", || {
            sqlx::query_as::<_, Tenant>(
                "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan, settings
                 FROM tenants 
                 WHERE slug = ANY($1) AND is_active = TRUE",
            )
            .bind(slugs)
            .fetch_all(&self.pool)
        })
        .await;

        result.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "tenant find_by_slugs failed after retries");
            Vec::new()
        })
    }

    pub async fn resolve_from_host(&self, host: &str) -> Option<TenantContext> {
        let host_clean = normalize_host(host);

        if host_clean.is_empty() {
            return self.default_context().await;
        }

        if host_clean.len() > MAX_HOST_LEN {
            return None;
        }

        if let Some(cached) = tenant_cache().get(&host_clean) {
            record_cache_hit();
            return cached;
        }

        record_cache_miss();
        let resolved = self.resolve_host(&host_clean).await;
        tenant_cache().insert(host_clean, resolved.clone());
        resolved
    }

    async fn resolve_host(&self, host: &str) -> Option<TenantContext> {
        if let Some(tenant) = self.find_by_domain(host).await {
            return Some(Self::to_context(&tenant));
        }

        let Some(base) = base_host() else {
            return self.resolve_host_permissive(host).await;
        };

        if host == "localhost" || host == base || is_ip_literal(host) {
            return self.default_context().await;
        }

        if host_matches_base(host, &base) || host.ends_with(".localhost") {
            let slug = slug_for(host);
            if slug.is_empty() || slug == host {
                return self.default_context().await;
            }
            return self.find_by_slug(slug).await.map(|t| Self::to_context(&t));
        }

        None
    }

    async fn resolve_host_permissive(&self, host: &str) -> Option<TenantContext> {
        let slug = slug_for(host);
        if slug.is_empty() || slug == host {
            return self.default_context().await;
        }
        self.find_by_slug(slug).await.map(|t| Self::to_context(&t))
    }

    async fn default_context(&self) -> Option<TenantContext> {
        self.find_by_slug("default")
            .await
            .map(|t| Self::to_context(&t))
    }

    fn to_context(tenant: &Tenant) -> TenantContext {
        TenantContext {
            tenant_id: tenant.id,
            slug: tenant.slug.clone(),
            status: if tenant.is_active {
                TenantStatus::Active
            } else {
                TenantStatus::Disabled
            },
            maintenance_eta: None,
            plan: tenant.plan.clone(),
            max_users: tenant.max_users,
            max_storage_bytes: tenant.max_storage_bytes,
        }
    }

    /// Convert a Tenant to a TenantConfig with settings from database.
    pub fn to_config(tenant: &Tenant) -> TenantConfig {
        let settings = tenant
            .settings
            .as_ref()
            .and_then(|s| serde_json::from_value::<TenantSettings>(s.clone()).ok())
            .unwrap_or_default();

        TenantConfig {
            tenant_id: tenant.id,
            slug: tenant.slug.clone(),
            name: tenant.name.clone(),
            host: tenant.custom_domain.clone().unwrap_or_default(),
            status: if tenant.is_active {
                "active".to_string()
            } else {
                "disabled".to_string()
            },
            plan: tenant.plan.clone(),
            max_users: tenant.max_users,
            max_storage_bytes: tenant.max_storage_bytes,
            settings,
        }
    }

    /// Fetch tenant configuration by slug.
    pub async fn get_config_by_slug(&self, slug: &str) -> Option<TenantConfig> {
        self.find_by_slug(slug).await.map(|t| Self::to_config(&t))
    }

    /// Fetch tenant configuration by domain.
    pub async fn get_config_by_domain(&self, domain: &str) -> Option<TenantConfig> {
        self.find_by_domain(domain)
            .await
            .map(|t| Self::to_config(&t))
    }

    /// 根据 ID 获取租户配置。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    ///
    /// # 返回
    /// * `Ok(Some(TenantConfig))` - 找到租户
    /// * `Ok(None)` - 租户不存在
    /// * `Err(sqlx::Error)` - 数据库查询失败（含重试后仍失败）
    pub async fn get_by_id(&self, tenant_id: i64) -> Result<Option<TenantConfig>, sqlx::Error> {
        let result = execute_with_retry("tenant_get_by_id", || {
            sqlx::query_as::<_, Tenant>(
                "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan, settings
                 FROM tenants
                 WHERE id = $1",
            )
            .bind(tenant_id)
            .fetch_optional(&self.pool)
        })
        .await?;

        Ok(result.map(|t| Self::to_config(&t)))
    }

    /// 更新租户配置（将 TenantSettings 序列化为 JSONB 存入 settings 列）。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    /// * `settings` - 新的租户配置
    ///
    /// # 返回
    /// * `Ok(())` - 更新成功
    /// * `Err(sqlx::Error)` - 数据库更新失败
    pub async fn update_settings(
        &self,
        tenant_id: i64,
        settings: TenantSettings,
    ) -> Result<(), sqlx::Error> {
        let settings_json =
            serde_json::to_value(&settings).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        execute_with_retry("tenant_update_settings", || {
            sqlx::query("UPDATE tenants SET settings = $1 WHERE id = $2")
                .bind(&settings_json)
                .bind(tenant_id)
                .execute(&self.pool)
        })
        .await?;

        Ok(())
    }

    /// 查询 `tenant_usage_stats` 视图，获取租户使用统计。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    ///
    /// # 返回
    /// * `Ok(Some(TenantUsageStatsRow))` - 找到统计数据
    /// * `Ok(None)` - 无统计数据（租户可能不存在）
    /// * `Err(sqlx::Error)` - 数据库查询失败
    pub async fn get_usage_stats(
        &self,
        tenant_id: i64,
    ) -> Result<Option<TenantUsageStatsRow>, sqlx::Error> {
        execute_with_retry("tenant_get_usage_stats", || {
            sqlx::query_as::<_, TenantUsageStatsRow>(
                "SELECT tenant_id, slug, name, user_count, video_count, 
                        storage_used_bytes::FLOAT8 as storage_used_bytes, 
                        max_storage_bytes, 
                        storage_usage_percent::FLOAT8 as storage_usage_percent
                 FROM tenant_usage_stats
                 WHERE tenant_id = $1",
            )
            .bind(tenant_id)
            .fetch_optional(&self.pool)
        })
        .await
    }

    /// 获取所有租户列表。
    ///
    /// # 返回
    /// * `Ok(Vec<TenantConfig>)` - 所有租户配置列表
    /// * `Err(sqlx::Error)` - 数据库查询失败
    pub async fn list_all(&self) -> Result<Vec<TenantConfig>, sqlx::Error> {
        let tenants = execute_with_retry("tenant_list_all", || {
            sqlx::query_as::<_, Tenant>(
                "SELECT id, name, slug, custom_domain, is_active, max_users, max_storage_bytes, plan, settings
                 FROM tenants
                 ORDER BY id",
            )
            .fetch_all(&self.pool)
        })
        .await?;

        Ok(tenants.iter().map(Self::to_config).collect())
    }

    /// 设置租户启用/禁用状态。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    /// * `is_active` - 是否启用
    ///
    /// # 返回
    /// * `Ok(true)` - 更新成功
    /// * `Ok(false)` - 租户不存在
    /// * `Err(sqlx::Error)` - 数据库更新失败
    pub async fn set_active(&self, tenant_id: i64, is_active: bool) -> Result<bool, sqlx::Error> {
        let rows = execute_with_retry("tenant_set_active", || {
            sqlx::query("UPDATE tenants SET is_active = $1 WHERE id = $2")
                .bind(is_active)
                .bind(tenant_id)
                .execute(&self.pool)
        })
        .await?;

        Ok(rows.rows_affected() > 0)
    }

    /// Resolve tenant configuration from host header.
    pub async fn resolve_config_from_host(&self, host: &str) -> Option<TenantConfig> {
        let host_clean = normalize_host(host);

        if host_clean.is_empty() {
            return self
                .find_by_slug("default")
                .await
                .map(|t| Self::to_config(&t));
        }

        if host_clean.len() > MAX_HOST_LEN {
            return None;
        }

        // Try to find by custom domain first
        if let Some(tenant) = self.find_by_domain(&host_clean).await {
            return Some(Self::to_config(&tenant));
        }

        // Try to extract slug from subdomain
        let slug = slug_for(&host_clean);
        if !slug.is_empty() && slug != host_clean {
            return self.get_config_by_slug(slug).await;
        }

        // Fallback to default tenant
        self.find_by_slug("default")
            .await
            .map(|t| Self::to_config(&t))
    }
}

impl Default for TenantSettings {
    fn default() -> Self {
        Self {
            max_upload_size_mb: 500,
            max_videos_per_user: 1000,
            registration_enabled: false,
            custom_theme: None,
            storage_quota_gb: 50,
        }
    }
}

fn normalize_host(host: &str) -> String {
    let host = host.trim().to_ascii_lowercase();
    let host = if let Some(after_bracket) = host.strip_prefix('[') {
        after_bracket.split(']').next().unwrap_or(host.as_str())
    } else {
        host.split(':').next().unwrap_or(host.as_str())
    };
    host.trim_end_matches('.').to_string()
}

fn is_ip_literal(host: &str) -> bool {
    host.parse::<std::net::IpAddr>().is_ok()
}

fn slug_for(host: &str) -> &str {
    host.split('.').next().unwrap_or("")
}

fn host_matches_base(host: &str, base: &str) -> bool {
    host.len() > base.len() && host.ends_with(&format!(".{}", base))
}

fn base_host() -> Option<String> {
    static BASE_HOST: OnceLock<Option<String>> = OnceLock::new();
    BASE_HOST
        .get_or_init(|| {
            std::env::var("PUBLIC_URL")
                .ok()
                .map(|u| parse_url_host(&u))
                .filter(|h| !h.is_empty())
        })
        .clone()
}

fn parse_url_host(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host_and_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host_and_port.split('@').next_back().unwrap_or("");
    let host = if let Some(after_bracket) = host.strip_prefix('[') {
        after_bracket.split(']').next().unwrap_or("")
    } else {
        host.split(':').next().unwrap_or("")
    };
    host.trim_end_matches('.').to_ascii_lowercase()
}
