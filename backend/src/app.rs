use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::DefaultBodyLimit,
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware as axum_mw,
    routing::{delete, get, post, put},
    Router,
};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::db::init_pool;
use crate::handlers;
use crate::metrics::Metrics;
use crate::middleware::auth::{admin_auth, bearer_auth, media_auth, role_auth};
use crate::middleware::hotlink::hotlink_guard;
use crate::middleware::rate_limit::RateLimiter;
use crate::middleware::request_id::request_id;
use crate::middleware::request_log::request_log;
use crate::middleware::security::{create_cors_layer, security_headers};
use crate::middleware::upload_bandwidth::bandwidth_throttle;
use crate::repositories::comment_repo::CommentRepository;
use crate::repositories::playback_repo::PlaybackRepository;
use crate::repositories::playlist_repo::PlaylistRepository;
use crate::repositories::registration_repo::RegistrationRepository;
use crate::repositories::share_repo::ShareRepository;
use crate::repositories::tag_repo::TagRepository;
use crate::repositories::user_repo::UserRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::services::admin_service::AdminService;
use crate::services::auth_service::AuthService;
use crate::services::comment_service::CommentService;
use crate::services::share_service::ShareService;
use crate::services::media_service::MediaService;
use crate::services::playback_service::PlaybackService;
use crate::services::recommendation_service::RecommendationService;
use crate::services::search_service::SearchService;
use crate::services::tag_service::TagService;
use crate::services::task_queue::TaskQueue;
use crate::services::transcoder::Transcoder;
use crate::services::video_service::VideoService;
use crate::state::{AppState, PlaybackSessionTracker, VideoListCache};

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
    let video_repo = VideoRepository::new(pool.clone());
    let playback_repo = PlaybackRepository::new(pool.clone());
    let playlist_repo = PlaylistRepository::new(pool.clone());
    let comment_repo = CommentRepository::new(pool.clone());
    let share_repo = ShareRepository::new(pool.clone());
    let tag_repo = TagRepository::new(pool.clone());
    let registration_repo = RegistrationRepository::new(pool.clone());

    let video_service = VideoService::new(video_repo.clone(), config.clone());
    let media_service = MediaService::new(video_repo.clone(), config.clone());
    let playback_service = PlaybackService::new(playback_repo.clone());
    let tag_service = TagService::new(tag_repo.clone());
    let search_service = SearchService::new(video_repo.clone());
    let recommendation_service = RecommendationService::new(video_repo.clone());
    let comment_service = CommentService::new(comment_repo.clone());
    let share_service = ShareService::new(share_repo.clone());
    let admin_service = AdminService::new(user_repo.clone());
    let rate_limiter = RateLimiter::new();
    let ip_rate_limiter = RateLimiter::new();

    // Start cleanup tasks to prevent memory leak
    crate::middleware::rate_limit::start_cleanup_task(
        rate_limiter.clone(),
        ip_rate_limiter.clone(),
    );
    let auth_service = AuthService::new(
        user_repo.clone(),
        playback_service.clone(),
        rate_limiter.clone(),
        ip_rate_limiter.clone(),
        config.clone(),
    );

    let video_cache = VideoListCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(1024)
        .build();

    let metrics = Metrics::new();
    let transcoder = Transcoder::new(video_repo.clone(), config.clone());
    let task_queue = TaskQueue::new(transcoder.clone(), pool.clone());

    // Start task queue worker
    task_queue.start_worker().await;

    // Load persisted registration toggle from DB
    match registration_repo.get_enabled().await {
        Ok(db_val) => config.set_registration_enabled(db_val),
        Err(e) => tracing::warn!(
            "Failed to load registration_enabled from DB, using env default: {}",
            e
        ),
    }

    let state = Arc::new(AppState {
        registration_repo,
        user_repo,
        video_repo,
        playback_repo,
        playlist_repo,
        comment_repo,
        share_repo,
        tag_repo,
        video_service: video_service.clone(),
        media_service: media_service.clone(),
        playback_service,
        auth_service,
        tag_service,
        search_service,
        recommendation_service,
        comment_service,
        share_service,
        admin_service,
        config: config.clone(),
        rate_limiter,
        ip_rate_limiter,
        video_cache,
        playback_sessions: std::sync::Arc::new(PlaybackSessionTracker::new()),
        upload_locks: std::sync::Arc::new(dashmap::DashMap::new()),
        metrics,
        transcoder,
        task_queue,
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

    // Periodic database pool metrics (every 15s)
    {
        let pool = pool.clone();
        let metrics = state.metrics.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(15)).await;
                metrics.set_database_pool_stats(pool.size() as f64, pool.num_idle() as f64);
            }
        });
    }

    // Periodic expired share-link cleanup (SH-04). Runs every hour so a
    // steady stream of expired rows doesn't accumulate.
    {
        let share_repo = state.share_repo.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                match share_repo.cleanup_expired().await {
                    Ok(0) => {
                        tracing::debug!("No expired share links to clean up");
                    }
                    Ok(n) => tracing::info!("Cleaned up {} expired share links", n),
                    Err(e) => tracing::error!("Failed to clean up share links: {}", e),
                }
            }
        });
    }

    // Background thumbnail backfill for videos without covers
    {
        let svc = media_service.clone();
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
    let cors = create_cors_layer(&config.cors_origin);

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
            .route("/auth/user/avatar", post(handlers::auth::upload_avatar))
            .route("/auth/user/shares", get(handlers::shares::list_my_shares))
            .route(
                "/auth/user/shares/{share_id}",
                delete(handlers::shares::revoke_my_share),
            )
            .route("/auth/logout", post(handlers::auth::logout))
            .route("/admin/track", post(handlers::admin::track_action))
            .route(
                "/recommendations",
                get(handlers::recommendations::get_recommendations),
            )
            .route(
                "/recommendations/trending",
                get(handlers::recommendations::get_trending_videos),
            )
            .route(
                "/recommendations/recent",
                get(handlers::recommendations::get_recent_videos),
            )
            .route(
                "/recommendations/similar/{video_id}",
                get(handlers::recommendations::get_similar_videos),
            )
            .route(
                "/playlists",
                get(handlers::playlists::list_my_playlists)
                    .post(handlers::playlists::create_playlist),
            )
            .route(
                "/playlists/{id}",
                get(handlers::playlists::get_playlist)
                    .put(handlers::playlists::update_playlist)
                    .delete(handlers::playlists::delete_playlist),
            )
            .route(
                "/playlists/{id}/videos",
                post(handlers::playlists::add_video_to_playlist),
            )
            .route(
                "/playlists/{id}/videos/{video_id}",
                delete(handlers::playlists::remove_video_from_playlist),
            )
            .route(
                "/videos/{id}/comments",
                get(handlers::comments::list_comments).post(handlers::comments::create_comment),
            )
            .route(
                "/comments/{id}/replies",
                get(handlers::comments::list_replies),
            )
            .route("/comments/{id}", delete(handlers::comments::delete_comment))
            .route(
                "/videos/{id}/share",
                post(handlers::shares::create_share_link),
            )
            .route(
                "/videos/{id}/share/{share_id}",
                delete(handlers::shares::delete_share_link),
            )
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // Video list/detail routes (bearer auth + role >= 1 viewer)
    let video_routes = with_timeout(
        Router::new()
            .route("/videos", get(handlers::videos::list_videos))
            .route("/videos/favorites", get(handlers::videos::list_favorites))
            .route("/videos/search", get(handlers::videos::search_videos))
            .route(
                "/videos/search/suggest",
                get(handlers::videos::search_suggest),
            )
            .route("/videos/{id}", get(handlers::videos::get_video))
            .route("/videos/{id}/view", post(handlers::videos::increment_views))
            .route("/videos/{id}/like", post(handlers::videos::toggle_like))
            .route("/videos/{id}/like", get(handlers::videos::get_like_status))
            .route(
                "/videos/{id}/favorite",
                post(handlers::videos::toggle_favorite),
            )
            .route(
                "/videos/{id}/favorite",
                get(handlers::videos::get_favorite_status),
            )
            .route("/videos/{id}/tags", get(handlers::tags::get_video_tags))
            .route("/videos/{id}/tags", post(handlers::tags::add_tags_to_video))
            .route(
                "/videos/{id}/tags",
                delete(handlers::tags::remove_tags_from_video),
            )
            .route(
                "/videos/{id}/tags/{tag_id}",
                delete(handlers::tags::remove_tag_from_video),
            )
            .route_layer(axum_mw::from_fn(|req, next| role_auth(req, next, 1)))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // Playback history routes (bearer auth + role >= 1 viewer)
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
            .route(
                "/playback/session/start",
                post(handlers::playback::start_playback_session),
            )
            .route(
                "/playback/session/heartbeat",
                post(handlers::playback::playback_session_heartbeat),
            )
            .route(
                "/playback/session/stop",
                post(handlers::playback::stop_playback_session),
            )
            .route_layer(axum_mw::from_fn(|req, next| role_auth(req, next, 1)))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // Upload route with no body size limit (handles large video files)
    let upload_route = {
        let r = Router::new()
            .route("/admin/videos/upload", post(handlers::admin::upload_video))
            .route(
                "/admin/videos/upload-resume",
                post(handlers::admin::upload_resume),
            )
            .route(
                "/admin/videos/upload-status",
                get(handlers::admin::upload_status),
            )
            .layer(DefaultBodyLimit::disable())
            .route_layer(axum_mw::from_fn(admin_auth))
            .route_layer(axum_mw::from_fn(bearer_auth));
        with_timeout(r, 7200)
    };

    // Admin routes (bearer + admin auth)
    let admin_routes = with_timeout(
        Router::new()
            .route("/admin/users", get(handlers::admin::list_users))
            .route("/admin/users/{id}", delete(handlers::admin::delete_user))
            .route(
                "/admin/videos/external",
                post(handlers::admin::add_external_video),
            )
            .route(
                "/admin/videos/check-hashes",
                post(handlers::admin::check_hashes),
            )
            .route(
                "/admin/videos/check-files",
                post(handlers::admin::check_files),
            )
            .route("/admin/videos/scan", post(handlers::admin::scan_media))
            .route(
                "/admin/videos/backfill-thumbnails",
                post(handlers::admin::backfill_thumbnails),
            )
            .route(
                "/admin/videos/batch",
                delete(handlers::admin::delete_videos),
            )
            .route("/admin/videos/{id}", put(handlers::admin::update_video))
            .route("/admin/videos/{id}", delete(handlers::admin::delete_video))
            .route(
                "/admin/videos/{id}/cover",
                post(handlers::admin::upload_cover),
            )
            .route(
                "/admin/videos/batch-category",
                put(handlers::admin::batch_update_category),
            )
            .route(
                "/admin/videos/{id}/transcode",
                post(handlers::admin::transcode_video),
            )
            .route(
                "/admin/videos/{id}/transcode/status",
                get(handlers::admin::transcode_status),
            )
            .route(
                "/admin/videos/{id}/transcode/{resolution}",
                delete(handlers::admin::delete_variant),
            )
            .route(
                "/admin/videos/{id}/transcode/cancel",
                post(handlers::admin::cancel_transcode),
            )
            .route("/admin/tags", post(handlers::tags::create_tag))
            .route("/admin/tags/{id}", put(handlers::tags::update_tag))
            .route("/admin/tags/{id}", delete(handlers::tags::delete_tag))
            .route("/admin/stats", get(handlers::admin::get_stats))
            .route(
                "/admin/users/{id}/password",
                put(handlers::admin::reset_user_password),
            )
            .route(
                "/admin/users/{id}/admin",
                put(handlers::admin::toggle_user_admin),
            )
            .route(
                "/admin/users/{id}/approve",
                put(handlers::admin::approve_user),
            )
            .route("/admin/users/{id}/kick", post(handlers::admin::kick_user))
            .route(
                "/admin/config/registration",
                get(handlers::admin::get_registration_enabled),
            )
            .route(
                "/admin/config/registration",
                put(handlers::admin::set_registration_enabled),
            )
            .route("/admin/system", get(handlers::admin::system_info))
            .route("/admin/logs", get(handlers::admin::get_logs))
            .route("/admin/logs", delete(handlers::admin::clear_logs))
            .route_layer(axum_mw::from_fn(admin_auth))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        7200,
    );

    // Public routes (no auth needed — minimal set)
    let public_routes = with_timeout(
        Router::new()
            // Health is intentionally public for load balancer / k8s liveness probes
            .route("/health", get(handlers::server::health))
            // Login and register must be accessible without auth
            .route("/auth/register", post(handlers::auth::register))
            .route("/auth/login", post(handlers::auth::login))
            // Tags are public (read-only)
            .route("/tags", get(handlers::tags::list_tags))
            .route("/tags/popular", get(handlers::tags::get_popular_tags))
            .route("/tags/{id}", get(handlers::tags::get_tag))
            // Share tokens allow unauthenticated access to shared content
            .route("/share/{token}", get(handlers::shares::get_share_video))
            // SECURITY (H-06): public share token endpoint MUST be
            // rate-limited. The token itself is un-guessable, but the
            // response size is a side channel that can be used to enumerate
            // valid tokens.
            .route_layer(axum_mw::from_fn(
                crate::middleware::share_rate_limit::share_rate_limit,
            )),
        30,
    );

    // Internal monitoring routes (bearer auth + admin only — sensitive system info)
    let internal_routes = with_timeout(
        Router::new()
            .route("/server/info", get(handlers::server::server_info))
            .route("/metrics", get(handlers::server::metrics))
            .route(
                "/metrics/prometheus",
                get(handlers::server::metrics_prometheus),
            )
            .route_layer(axum_mw::from_fn(admin_auth))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    // OpenAPI docs routes (bearer auth + role >= 1 viewer — schema exposes attack surface)
    let docs_routes = with_timeout(
        Router::new()
            .route("/docs/openapi.json", get(handlers::server::openapi_spec))
            .route("/docs", get(handlers::server::docs_redirect))
            .route_layer(axum_mw::from_fn(|req, next| role_auth(req, next, 1)))
            .route_layer(axum_mw::from_fn(bearer_auth)),
        30,
    );

    let webapp_service = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ))
        .service(
            ServeDir::new(&config.webapp_root)
                .not_found_service(ServeFile::new(config.webapp_root.join("index.html"))),
        );

    let webapp_index =
        std::fs::read_to_string(config.webapp_root.join("index.html")).unwrap_or_default();

    let spa_fallback = move || {
        let body = webapp_index.clone();
        async move {
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
        }
    };

    Router::new()
        .merge(public_routes)
        .merge(internal_routes)
        .merge(docs_routes)
        .merge(auth_routes)
        .merge(video_routes)
        .merge(playback_routes)
        .merge(upload_route)
        .merge(admin_routes)
        .nest_service("/webapp", webapp_service)
        .fallback(spa_fallback)
        .nest_service(
            "/media",
            ServiceBuilder::new()
                .layer(axum_mw::from_fn(media_auth))
                // SECURITY (ST F-04): hotlink protection — block range
                // requests whose Referer/Origin points off-origin.
                .layer(axum_mw::from_fn(hotlink_guard))
                // SECURITY (ST F-05): bandwidth throttle.
                .layer(axum_mw::from_fn(bandwidth_throttle))
                .layer(SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    HeaderValue::from_static("private, max-age=300"),
                ))
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(600),
                ))
                .service(ServeDir::new(&config.media_root)),
        )
        // inject_state MUST be before request_log so it can access AppState for user lookup
        .layer(
            ServiceBuilder::new()
                .layer(axum_mw::from_fn(request_id))
                .layer(axum_mw::from_fn(request_log))
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors),
        )
        .layer(inject_state)
        .layer(axum_mw::from_fn(security_headers))
        .with_state(state)
}
