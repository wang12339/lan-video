use std::sync::OnceLock;
use std::time::Duration;

use moka::sync::Cache;
use sqlx::PgPool;

use crate::middleware::tenant::TenantContext;

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
}

#[derive(Clone)]
pub struct TenantRepository {
    pool: PgPool,
}

const TENANT_CACHE_TTL_SECS: u64 = 60;
const TENANT_CACHE_MAX_ENTRIES: u64 = 10_000;
const MAX_HOST_LEN: usize = 255;

static TENANT_CACHE: OnceLock<Cache<String, Option<TenantContext>>> = OnceLock::new();

fn tenant_cache() -> &'static Cache<String, Option<TenantContext>> {
    TENANT_CACHE.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(TENANT_CACHE_TTL_SECS))
            .max_capacity(TENANT_CACHE_MAX_ENTRIES)
            .build()
    })
}

impl TenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_slug(&self, slug: &str) -> Option<Tenant> {
        sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE slug = $1 AND is_active = TRUE")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(%slug, error = %e, "tenant find_by_slug failed");
                None
            })
    }

    pub async fn find_by_domain(&self, domain: &str) -> Option<Tenant> {
        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE custom_domain = $1 AND is_active = TRUE",
        )
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(%domain, error = %e, "tenant find_by_domain failed");
            None
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
            return cached;
        }

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
