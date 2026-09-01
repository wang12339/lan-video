use std::future::Future;
use std::net::SocketAddr;

use tracing_subscriber::prelude::*;

use atmos_video_backend::app::build_router;
use atmos_video_backend::config::AppConfig;

/// Initialize Sentry crash reporting from SENTRY_DSN env var (optional)
fn init_sentry(config: &AppConfig) {
    let dsn = config.sentry_dsn.clone();
    if !dsn.is_empty() {
        let _guard = sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: Some(config.sentry_environment.clone().into()),
                traces_sample_rate: 0.2,
                ..Default::default()
            },
        ));
        Box::leak(Box::new(_guard));
        tracing::info!("Sentry initialized");
    } else {
        tracing::info!("SENTRY_DSN not set — Sentry disabled");
    }
}

/// Create a shutdown signal future (Ctrl+C / SIGTERM)
#[allow(clippy::expect_used)]
fn shutdown_signal() -> impl Future<Output = ()> {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    async move {
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
        tracing::info!("shutdown signal received, starting graceful shutdown");
    }
}

#[allow(clippy::expect_used, clippy::panic)]
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = AppConfig::from_env();

    std::fs::create_dir_all(&config.log_dir).unwrap_or_else(|e| {
        tracing::warn!(
            "Failed to create log directory {}: {}",
            config.log_dir.display(),
            e
        )
    });

    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "atmos.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(_guard));

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // Write to both stdout (human-readable) and file (JSON) simultaneously
    let stdout_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout);
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    // Must run after the tracing subscriber is initialized, otherwise
    // init_sentry's own log lines are silently dropped.
    init_sentry(&config);

    let (data_dir_result, media_root_result) = tokio::join!(
        tokio::fs::create_dir_all(&config.data_dir),
        tokio::fs::create_dir_all(&config.media_root)
    );
    data_dir_result.unwrap_or_else(|e| panic!("Failed to create data directory: {}", e));
    media_root_result.unwrap_or_else(|e| panic!("Failed to create media root directory: {}", e));

    let app = build_router(config.clone()).await;

    // Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind TCP listener on {}: {}", addr, e));

    let is_dev = matches!(
        config.app_env.to_ascii_lowercase().as_str(),
        "development" | "dev" | "local"
    );
    let app = if !is_dev {
        let cfg = config.clone();
        app.layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let cfg = cfg.clone();
                async move {
                    use axum::response::Response;

                    let is_https = req
                        .headers()
                        .get("x-forwarded-proto")
                        .and_then(|v| v.to_str().ok())
                        .map(|v| v.eq_ignore_ascii_case("https"))
                        .unwrap_or(false);
                    if !is_https {
                        let path = req.uri().path();
                        if path != "/health" && !path.starts_with("/health/") {
                            let public_url = cfg.public_url.trim_end_matches('/');
                            let target = match req.uri().query() {
                                Some(q) => format!("{public_url}{path}?{q}"),
                                None => format!("{public_url}{path}"),
                            };
                            if let Ok(header) = axum::http::HeaderValue::from_str(&target) {
                                if let Ok(resp) = Response::builder()
                                    .status(axum::http::StatusCode::PERMANENT_REDIRECT)
                                    .header(axum::http::header::LOCATION, header)
                                    .body(axum::body::Body::empty())
                                {
                                    return resp;
                                }
                            }
                        }
                    }
                    next.run(req).await
                }
            },
        ))
    } else {
        app
    };

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        app_env = %config.app_env,
        server_port = config.server_port,
        "Atmos Video server starting"
    );
    tracing::info!(public_url = %config.public_url, "public base url");
    tracing::info!(media_root = %config.media_root.display(), "media root");
    tracing::info!(
        redis = if config.redis_url.is_empty() {
            "disabled"
        } else {
            "enabled"
        },
        admin_ip_whitelist = if config.admin_ip_whitelist.is_empty() {
            "disabled"
        } else {
            "enabled"
        },
        registration = if config.registration_enabled() {
            "enabled"
        } else {
            "disabled"
        },
        "feature flags"
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap_or_else(|e| {
        tracing::error!("server exited with error: {}", e);
        std::process::exit(1);
    });

    tracing::info!("Server shutdown complete");
}
