mod config;
mod db;
mod models;
mod handlers;
mod middleware;
mod repositories;
mod services;
mod util;

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::timeout::TimeoutLayer;
use tower::ServiceBuilder;
use axum::{
    extract::Request,
    middleware as axum_mw,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::services::ServeDir;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::{HeaderValue, StatusCode};

use crate::config::AppConfig;
use crate::db::init_pool;
use crate::middleware::auth::{AppState, RateLimiter, bearer_auth, admin_auth};
use crate::repositories::user_repo::UserRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::services::video_service::VideoService;

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, starting graceful shutdown");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .json()
        .init();

    dotenvy::dotenv().ok();

    let config = AppConfig::from_env();
    let pool = init_pool(&config.database_url).await;

    let user_repo = UserRepository::new(pool.clone());
    let video_repo = VideoRepository::new(pool);
    let video_service = VideoService::new(video_repo, config.clone());

    let video_cache = crate::middleware::auth::VideoListCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(64)
        .build();

    let state = Arc::new(AppState {
        user_repo,
        video_service,
        config: config.clone(),
        rate_limiter: RateLimiter::new(),
        video_cache,
    });

    // Periodic rate limiter cleanup (remove expired entries every 5 minutes)
    {
        let rate_limiter = state.rate_limiter.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                rate_limiter.cleanup_expired().await;
            }
        });
    }

    // CORS: permissive origin required for LAN apps with dynamic IPs; restrict methods/headers
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST,
                        axum::http::Method::PUT, axum::http::Method::DELETE])
        .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::AUTHORIZATION,
            "X-Username".parse().expect("static header name is valid"),
            "X-Password".parse().expect("static header name is valid"),
            "X-Admin-Token".parse().expect("static header name is valid")]);

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

    // Routes requiring bearer auth (no admin check) — 30s timeout
    let auth_routes = Router::new()
        .route("/auth/user", get(handlers::auth::user_info))
        .route("/auth/user/profile", get(handlers::auth::user_profile))
        .route("/auth/logout", post(handlers::auth::logout))
        .route_layer(axum_mw::from_fn(bearer_auth))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)));

    // Video list/detail routes (bearer auth) — 30s timeout
    let video_routes = Router::new()
        .route("/videos", get(handlers::videos::list_videos))
        .route("/videos/{id}", get(handlers::videos::get_video))
        .route_layer(axum_mw::from_fn(bearer_auth))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)));

    // Playback history routes (bearer auth) — 30s timeout
    let playback_routes = Router::new()
        .route("/playback/history/{video_id}", get(handlers::playback::get_playback_history_for_video))
        .route("/playback/history", get(handlers::playback::list_playback_history))
        .route("/playback/history", post(handlers::playback::update_playback_history))
        .route_layer(axum_mw::from_fn(bearer_auth))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)));

    // Admin routes (bearer + admin auth) — 10min timeout for uploads
    let admin_routes = Router::new()
        .route("/admin/videos/external", post(handlers::admin::add_external_video))
        .route("/admin/videos/upload", post(handlers::admin::upload_video))
        .route("/admin/videos/check-hashes", post(handlers::admin::check_hashes))
        .route("/admin/videos/check-files", post(handlers::admin::check_files))
        .route("/admin/videos/scan", post(handlers::admin::scan_media))
        .route("/admin/videos/backfill-thumbnails", post(handlers::admin::backfill_thumbnails))
        .route("/admin/videos/batch", delete(handlers::admin::delete_videos))
        .route("/admin/videos/{id}", put(handlers::admin::update_video))
        .route("/admin/videos/{id}", delete(handlers::admin::delete_video))
        .route("/admin/videos/{id}/cover", post(handlers::admin::upload_cover))
        .route_layer(axum_mw::from_fn(admin_auth))
        .route_layer(axum_mw::from_fn(bearer_auth))
        .layer(RequestBodyLimitLayer::new(250 * 1024 * 1024)) // 250MB max upload
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(600))); // 10min for large uploads

    // Public routes (no auth needed) — 30s timeout
    let public_routes = Router::new()
        .route("/server/info", get(handlers::server::server_info))
        .route("/health", get(handlers::server::health))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/access", post(handlers::auth::access_check))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)));

    // Build router
    let app = Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(video_routes)
        .merge(playback_routes)
        .merge(admin_routes)
        .nest_service("/webapp", ServeDir::new("../webapp"))
        .nest_service("/media", ServiceBuilder::new()
            .layer(SetResponseHeaderLayer::overriding(
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=3600"),
            ))
            .service(ServeDir::new(&config.media_root)))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(inject_state)
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    tracing::info!("Starting server on {}", addr);
    tracing::info!("Media root: {}", config.media_root.display());

    let listener = tokio::net::TcpListener::bind(addr).await
        .unwrap_or_else(|e| panic!("failed to bind TCP listener on {}: {}", addr, e));

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| panic!("axum serve exited with error: {}", e));
}
