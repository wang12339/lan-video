use std::sync::Arc;
use std::time::Duration;

use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tower::ServiceBuilder;
use axum::{
    extract::Request,
    extract::DefaultBodyLimit,
    http::{HeaderValue, StatusCode},
    middleware as axum_mw,
    response::Redirect,
    routing::{delete, get, post, put},
    Router,
};

use lan_video_backend::config::AppConfig;
use lan_video_backend::db::init_pool;
use lan_video_backend::handlers;
use lan_video_backend::middleware::auth::{bearer_auth, admin_auth, media_auth};
use lan_video_backend::middleware::rate_limit::RateLimiter;
use lan_video_backend::openapi;
use lan_video_backend::repositories::user_repo::UserRepository;
use lan_video_backend::repositories::video_repo::VideoRepository;
use lan_video_backend::services::video_service::VideoService;
use lan_video_backend::state::{AppState, VideoListCache};

/// Add security headers to every response
async fn security_headers(
    req: Request,
    next: axum_mw::Next,
) -> impl axum::response::IntoResponse {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        axum::http::header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; media-src 'self' blob:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; script-src 'self'",
        ),
    );
    res
}

/// Apply a timeout layer with standard REQUEST_TIMEOUT status code
fn with_timeout<S>(router: Router<S>, secs: u64) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.layer(TimeoutLayer::with_status_code(
        StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(secs),
    ))
}

pub async fn build_router(config: AppConfig) -> Router {
    let pool = init_pool(&config.database_url).await;

    let user_repo = UserRepository::new(pool.clone());
    let video_repo = VideoRepository::new(pool);
    let video_service = VideoService::new(video_repo, config.clone());

    let video_cache = VideoListCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(1024)
        .build();

    let state = Arc::new(AppState {
        user_repo,
        video_service: video_service.clone(),
        config: config.clone(),
        rate_limiter: RateLimiter::new(),
        video_cache,
    });

    // Periodic expired token cleanup every 5 minutes
    {
        let user_repo = state.user_repo.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await;
                match user_repo.cleanup_expired_tokens().await {
                    Ok(n) => {
                        if n > 0 {
                            tracing::debug!("Cleaned up {} expired tokens", n);
                        }
                    }
                    Err(e) => tracing::error!("Failed to clean up expired tokens: {}", e),
                }
            }
        });
    }

    // Background thumbnail backfill for videos without covers
    {
        let svc = video_service.clone();
        tokio::spawn(async move {
            tracing::info!("Starting thumbnail backfill for videos without covers...");
            match svc.backfill_thumbnails().await {
                Ok((generated, errors)) => {
                    for e in &errors {
                        tracing::warn!("Thumbnail backfill error: {}", e);
                    }
                    tracing::info!(
                        "Thumbnail backfill complete: {} generated, {} errors",
                        generated,
                        errors.len()
                    );
                }
                Err(e) => tracing::error!("Thumbnail backfill failed: {}", e),
            }
        });
    }

    // CORS: only allow the configured origin
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::exact(
            config
                .cors_origin
                .parse::<HeaderValue>()
                .expect("invalid CORS_ORIGIN"),
        ))
        .allow_credentials(true)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ]);

    // Inject state into request extensions for middleware access
    let inject_state = {
        let state = state.clone();
        axum_mw::from_fn(move |req: Request, next: axum_mw::Next| {
            let state = state.clone();
            async move {
                let mut req = req;
                req.extensions_mut().insert(state);
                next.run(req).await
            }
        })
    };

    // Routes requiring bearer auth (no admin check)
    let auth_routes = with_timeout(
        Router::new()
            .route("/auth/user", get(handlers::auth::user_info))
            .route("/auth/user/profile", get(handlers::auth::user_profile))
            .route("/auth/logout", post(handlers::auth::logout))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // Video list/detail routes (bearer auth)
    let video_routes = with_timeout(
        Router::new()
            .route("/videos", get(handlers::videos::list_videos))
            .route("/videos/{id}", get(handlers::videos::get_video))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // Playback history routes (bearer auth)
    let playback_routes = with_timeout(
        Router::new()
            .route(
                "/playback/history/{video_id}",
                get(handlers::playback::get_playback_history_for_video),
            )
            .route(
                "/playback/history",
                get(handlers::playback::list_playback_history),
            )
            .route(
                "/playback/history",
                post(handlers::playback::update_playback_history),
            )
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // Upload route with no body size limit (handles large video files)
    let upload_route = {
        let r = Router::new()
            .route("/admin/videos/upload", post(handlers::admin::upload_video))
            .layer(DefaultBodyLimit::disable())
            .route_layer(axum_mw::from_fn(admin_auth))
            .route_layer(axum_mw::from_fn(bearer_auth));
        with_timeout(r, 7200)
    };

    // Admin routes (bearer + admin auth)
    let admin_routes = with_timeout(
        Router::new()
            .route("/admin/videos/external", post(handlers::admin::add_external_video))
            .route("/admin/videos/check-hashes", post(handlers::admin::check_hashes))
            .route("/admin/videos/check-files", post(handlers::admin::check_files))
            .route("/admin/videos/scan", post(handlers::admin::scan_media))
            .route(
                "/admin/videos/backfill-thumbnails",
                post(handlers::admin::backfill_thumbnails),
            )
            .route("/admin/videos/batch", delete(handlers::admin::delete_videos))
            .route("/admin/videos/{id}", put(handlers::admin::update_video))
            .route("/admin/videos/{id}", delete(handlers::admin::delete_video))
            .route("/admin/videos/{id}/cover", post(handlers::admin::upload_cover))
            .route_layer(axum_mw::from_fn(admin_auth))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        7200,
    );

    // Public routes (no auth needed)
    async fn openapi_spec() -> axum::Json<serde_json::Value> {
        axum::Json(openapi::spec())
    }

    async fn docs_redirect() -> Redirect {
        Redirect::permanent("/docs/openapi.json")
    }

    let public_routes = with_timeout(
        Router::new()
            .route("/server/info", get(handlers::server::server_info))
            .route("/health", get(handlers::server::health))
            .route("/auth/register", post(handlers::auth::register))
            .route("/auth/login", post(handlers::auth::login))
            .route("/docs/openapi.json", get(openapi_spec))
            .route("/docs", get(docs_redirect)),
        30,
    );

    Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(video_routes)
        .merge(playback_routes)
        .merge(upload_route)
        .merge(admin_routes)
        .nest_service("/webapp", ServeDir::new(&config.webapp_root))
        .nest_service(
            "/media",
            ServiceBuilder::new()
                .layer(axum_mw::from_fn(media_auth))
                .layer(SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    HeaderValue::from_static("private, max-age=300"),
                ))
                .service(ServeDir::new(&config.media_root)),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(inject_state),
        )
        .layer(axum_mw::from_fn(security_headers))
        .with_state(state)
}
