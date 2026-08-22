use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use dashmap::DashMap;
use serde::Deserialize;

use crate::state::AppState;

/// 租户状态
#[derive(Debug, Clone, PartialEq)]
pub enum TenantStatus {
    /// 正常运行
    Active,
    /// 已禁用
    Disabled,
    /// 维护中
    Maintenance,
}

impl TenantStatus {
    /// 从字符串解析状态
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" | "" => Self::Active,
            "disabled" | "inactive" => Self::Disabled,
            "maintenance" => Self::Maintenance,
            _ => Self::Active,
        }
    }
}

/// 租户上下文信息
#[derive(Clone, Debug)]
pub struct TenantContext {
    pub tenant_id: i64,
    pub slug: String,
    pub status: TenantStatus,
    pub maintenance_eta: Option<String>,
    pub plan: String,
    pub max_users: i32,
    pub max_storage_bytes: i64,
}

/// 租户级限流器
///
/// 基于 DashMap 的内存限流器，每个租户独立限流。
/// 使用滑动窗口算法，支持突发流量。
#[derive(Clone)]
pub struct TenantRateLimiter {
    /// 租户请求计数：key = tenant_id, value = (窗口开始时间, 请求计数)
    counters: Arc<DashMap<i64, (Instant, u64)>>,
    /// 每个租户每分钟最大请求数
    max_requests_per_minute: u64,
}

impl TenantRateLimiter {
    /// 创建新的租户限流器
    ///
    /// # 参数
    /// * `max_requests_per_minute` - 每个租户每分钟最大请求数
    pub fn new(max_requests_per_minute: u64) -> Self {
        Self {
            counters: Arc::new(DashMap::new()),
            max_requests_per_minute,
        }
    }

    /// 检查租户是否超过限流
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    ///
    /// # 返回
    /// * `true` - 允许请求
    /// * `false` - 超过限流
    pub fn check(&self, tenant_id: i64) -> bool {
        let now = Instant::now();
        let mut entry = self.counters.entry(tenant_id).or_insert_with(|| (now, 0));

        let (window_start, count) = entry.value_mut();

        // 如果窗口已过期（超过 1 分钟），重置计数器
        if now.duration_since(*window_start).as_secs() >= 60 {
            *window_start = now;
            *count = 0;
        }

        *count += 1;

        // 检查是否超过限制
        *count <= self.max_requests_per_minute
    }

    /// 清理过期的计数器条目
    ///
    /// 应定期调用以防止内存泄漏
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.counters
            .retain(|_, (window_start, _)| now.duration_since(*window_start).as_secs() < 120);
    }
}

/// 从数据库行反序列化租户信息
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TenantRow {
    id: i64,
    slug: String,
    is_active: bool,
    plan: String,
    max_users: i32,
    max_storage_bytes: i64,
    status: Option<String>,
    maintenance_eta: Option<String>,
}

/// 解析租户上下文
///
/// # 参数
/// * `req` - HTTP 请求
/// * `next` - 下一个中间件
///
/// # 返回
/// * 成功时返回下一个中间件的响应
/// * 失败时返回错误响应
pub async fn resolve_tenant(mut req: Request, next: Next) -> Response {
    let start_time = Instant::now();

    // 获取应用状态
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        tracing::error!("AppState not found in request extensions");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "server config error".to_string(),
        )
            .into_response();
    };

    // 提取 Host 头
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    // 解析租户
    let tenant = state.repos.tenant.resolve_from_host(&host).await;

    let Some(tenant) = tenant else {
        tracing::warn!(host = %host, "no tenant found for host");
        return (StatusCode::NOT_FOUND, "unknown site").into_response();
    };

    // 检查租户状态
    match tenant.status {
        TenantStatus::Disabled => {
            tracing::warn!(
                tenant_id = tenant.tenant_id,
                slug = %tenant.slug,
                host = %host,
                "tenant disabled"
            );
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "站点已禁用",
                    "tenant": tenant.slug
                })),
            )
                .into_response();
        }
        TenantStatus::Maintenance => {
            tracing::warn!(
                tenant_id = tenant.tenant_id,
                slug = %tenant.slug,
                host = %host,
                "tenant under maintenance"
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "站点维护中",
                    "tenant": tenant.slug,
                    "estimated_time": tenant.maintenance_eta
                })),
            )
                .into_response();
        }
        TenantStatus::Active => {}
    }

    // 检查租户级限流
    let tenant_limiter = TenantRateLimiter::new(1000); // 每分钟 1000 请求
    if !tenant_limiter.check(tenant.tenant_id) {
        tracing::warn!(
            tenant_id = tenant.tenant_id,
            slug = %tenant.slug,
            host = %host,
            "tenant rate limited"
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "请求过于频繁，请稍后重试",
                "tenant": tenant.slug
            })),
        )
            .into_response();
    }

    // 注入租户上下文
    req.extensions_mut().insert(tenant.clone());

    // 记录租户上下文日志
    tracing::info!(
        tenant_id = tenant.tenant_id,
        tenant_slug = %tenant.slug,
        tenant_status = ?tenant.status,
        tenant_plan = %tenant.plan,
        host = %host,
        duration_ms = start_time.elapsed().as_millis() as u64,
        "tenant resolved"
    );

    // 执行下一个中间件
    let response = next.run(req).await;

    // 记录响应状态
    tracing::debug!(
        tenant_id = tenant.tenant_id,
        tenant_slug = %tenant.slug,
        status = response.status().as_u16(),
        duration_ms = start_time.elapsed().as_millis() as u64,
        "tenant request completed"
    );

    response
}

/// 健康检查端点
///
/// 用于检查租户系统是否正常工作
pub async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

/// 健康检查响应结构
#[derive(serde::Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tenant_system: String,
}

/// 详细的健康检查端点
///
/// 包含租户系统状态信息
pub async fn detailed_health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!(HealthCheckResponse {
            status: "healthy".to_string(),
            timestamp: chrono::Utc::now(),
            tenant_system: "operational".to_string(),
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicBool;

    use axum::{body::Body, middleware, routing::get, Router};
    use tower::ServiceExt;

    use crate::config::AppConfig;
    use crate::metrics::Metrics;
    use crate::middleware::rate_limit::RateLimiter;
    use crate::repositories::comment_repo::CommentRepository;
    use crate::repositories::playback_repo::PlaybackRepository;
    use crate::repositories::playlist_repo::PlaylistRepository;
    use crate::repositories::registration_repo::RegistrationRepository;
    use crate::repositories::share_repo::ShareRepository;
    use crate::repositories::tag_repo::TagRepository;
    use crate::repositories::tenant_repo::TenantRepository;
    use crate::repositories::user_repo::UserRepository;
    use crate::repositories::video_repo::VideoRepository;
    use crate::services::admin_service::AdminService;
    use crate::services::auth_service::AuthService;
    use crate::services::comment_service::CommentService;
    use crate::services::email_service::EmailService;
    use crate::services::media_service::MediaService;
    use crate::services::playback_service::PlaybackService;
    use crate::services::playlist_service::PlaylistService;
    use crate::services::recommendation_service::RecommendationService;
    use crate::services::search_service::SearchService;
    use crate::services::share_service::ShareService;
    use crate::services::tag_service::TagService;
    use crate::services::task_queue::TaskQueue;
    use crate::services::tenant_service::TenantService;
    use crate::services::transcoder::Transcoder;
    use crate::services::video_service::VideoService;
    use crate::state::{AppState, PlaybackSessionTracker, RepoLayer, ServiceLayer};
    use dashmap::DashMap;
    use moka::sync::Cache;
    use sqlx::postgres::PgPoolOptions;

    /// AppState whose tenant repo points at a dead port (1): host resolution
    /// always fails fast (connection refused) and the middleware determinis-
    /// tically returns 404 — no live database is required. This exercises the
    /// full pipeline: Host-header extraction, lowercasing, `normalize_host`
    /// (port / IPv6-bracket / trailing-dot stripping), the 255-byte guard and
    /// the subdomain/slug resolution paths.
    fn test_state() -> Arc<AppState> {
        let config = AppConfig {
            database_url: String::new(),
            server_port: 0,
            public_url: "https://video.example.com".to_string(),
            media_root: std::env::temp_dir(),
            webapp_root: std::env::temp_dir(),
            log_dir: std::env::temp_dir(),
            data_dir: std::env::temp_dir(),
            registration_enabled: Arc::new(AtomicBool::new(false)),
            cors_origin: String::new(),
            cookie_secure: false,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_username: String::new(),
            smtp_password: String::new(),
            smtp_from: String::new(),
            redis_url: String::new(),
        };
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(500))
            .connect_lazy("postgres://127.0.0.1:1/atmos_video_test")
            .expect("lazy pool");
        let repos = RepoLayer {
            registration: RegistrationRepository::new(pool.clone()),
            user: UserRepository::new(pool.clone()),
            video: VideoRepository::new(pool.clone()),
            playback: PlaybackRepository::new(pool.clone()),
            playlist: PlaylistRepository::new(pool.clone()),
            comment: CommentRepository::new(pool.clone()),
            share: ShareRepository::new(pool.clone()),
            tag: TagRepository::new(pool.clone()),
            tenant: TenantRepository::new(pool.clone()),
        };
        let playback_service = PlaybackService::new(repos.playback.clone());
        let playlist_service = PlaylistService::new(repos.playlist.clone());
        let services = ServiceLayer {
            video: VideoService::new(repos.video.clone(), config.clone()),
            media: MediaService::new(repos.video.clone(), config.clone()),
            playback: playback_service.clone(),
            playlist: playlist_service,
            auth: AuthService::new(
                repos.user.clone(),
                playback_service,
                RateLimiter::new(),
                RateLimiter::new(),
                config.clone(),
            ),
            email: EmailService::new(config.clone()),
            tag: TagService::new(repos.tag.clone(), repos.video.clone()),
            search: SearchService::new(repos.video.clone()),
            recommendation: RecommendationService::new(repos.video.clone()),
            comment: CommentService::new(repos.comment.clone(), repos.video.clone()),
            share: ShareService::new(repos.share.clone()),
            admin: AdminService::new(repos.user.clone()),
            tenant: TenantService::new(repos.tenant.clone()),
        };
        let transcoder = Transcoder::new(&std::env::temp_dir());
        Arc::new(AppState {
            repos,
            services,
            config,
            rate_limiter: RateLimiter::new(),
            ip_rate_limiter: RateLimiter::new(),
            video_cache: Cache::builder().max_capacity(10_000).build(),
            recommendation_cache: Cache::builder().max_capacity(10_000).build(),
            video_detail_cache: Cache::builder().max_capacity(10_000).build(),
            playback_sessions: Arc::new(PlaybackSessionTracker::new()),
            upload_locks: Arc::new(DashMap::new()),
            metrics: Metrics::new(),
            redis: None,
            transcoder: transcoder.clone(),
            task_queue: TaskQueue::new(transcoder, pool),
        })
    }

    fn tenant_app() -> Router {
        Router::new()
            .route("/{*any}", get(|| async { (StatusCode::OK, "ok") }))
            .layer(middleware::from_fn(resolve_tenant))
    }

    fn host_req(host: Option<&axum::http::HeaderValue>, state: &Arc<AppState>) -> Request {
        let mut builder = Request::builder().uri("/");
        if let Some(host) = host {
            builder = builder.header("host", host);
        }
        let mut req = builder.body(Body::empty()).unwrap();
        req.extensions_mut().insert(state.clone());
        req
    }

    #[tokio::test]
    async fn missing_state_returns_500() {
        let app = Router::new()
            .route("/{*any}", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(resolve_tenant));
        let res = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn absent_host_header_yields_404() {
        let state = test_state();
        let app = tenant_app();
        // No Host header at all
        let res = app.clone().oneshot(host_req(None, &state)).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        // Empty Host header
        let res = app
            .oneshot(host_req(Some(&"".parse().unwrap()), &state))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn host_forms_all_resolve_to_404_without_known_tenants() {
        let state = test_state();
        let app = tenant_app();
        // Each Host form must reach `normalize_host` and the resolver without
        // panicking and end in a clean 404 (resolution needs a live DB).
        let hosts = [
            "localhost",
            "localhost:8082",
            "127.0.0.1",
            "127.0.0.1:8082",
            "video.example.com",
            "video.example.com:8082",
            "VIDEO.EXAMPLE.COM",
            "Video.Example.Com.",
            "sub.video.example.com",
            "evil.example",
            "attacker.com:443",
            "example.com@evil.example", // forged userinfo-in-host
            "[::1]",
            "[::1]:8082",
            "[2001:db8::1]",
            "[2001:db8::1]:8082",
        ];
        for host in hosts {
            let res = app
                .clone()
                .oneshot(host_req(Some(&host.parse().unwrap()), &state))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::NOT_FOUND, "host: {host}");
        }
    }

    #[tokio::test]
    async fn overlong_host_yields_404() {
        let state = test_state();
        let app = tenant_app();
        // 300 bytes exceeds MAX_HOST_LEN (255) — rejected before any lookup
        let host = format!("{}.com", "a".repeat(300));
        let res = app
            .oneshot(host_req(Some(&host.parse().unwrap()), &state))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn non_utf8_host_header_yields_404() {
        let state = test_state();
        let app = tenant_app();
        // Non-UTF8 host bytes fail to_str() → treated as absent host
        let host = axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap();
        let res = app.oneshot(host_req(Some(&host), &state)).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn tenant_status_from_str() {
        assert_eq!(TenantStatus::from_str("active"), TenantStatus::Active);
        assert_eq!(TenantStatus::from_str("disabled"), TenantStatus::Disabled);
        assert_eq!(TenantStatus::from_str("inactive"), TenantStatus::Disabled);
        assert_eq!(
            TenantStatus::from_str("maintenance"),
            TenantStatus::Maintenance
        );
        assert_eq!(TenantStatus::from_str(""), TenantStatus::Active);
        assert_eq!(TenantStatus::from_str("unknown"), TenantStatus::Active);
    }

    #[test]
    fn tenant_rate_limiter_allows_within_limit() {
        let limiter = TenantRateLimiter::new(10);
        for _ in 0..10 {
            assert!(limiter.check(1));
        }
        assert!(!limiter.check(1));
    }

    #[test]
    fn tenant_rate_limiter_is_independent_per_tenant() {
        let limiter = TenantRateLimiter::new(2);
        assert!(limiter.check(1));
        assert!(limiter.check(2));
        assert!(limiter.check(1));
        assert!(!limiter.check(1));
        assert!(limiter.check(2));
    }

    #[tokio::test]
    async fn health_check_returns_200() {
        let response = health_check().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn detailed_health_check_returns_200_with_json() {
        let response = detailed_health_check().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
