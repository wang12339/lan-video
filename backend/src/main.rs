mod app;

use std::net::SocketAddr;
use std::future::Future;

use tracing_subscriber::prelude::*;

use lan_video_backend::config::AppConfig;
use crate::app::build_router;

/// Initialize Sentry crash reporting from SENTRY_DSN env var (optional)
fn init_sentry() {
    let dsn = std::env::var("SENTRY_DSN").unwrap_or_default();
    if !dsn.is_empty() {
        let _guard = sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: Some(
                    std::env::var("SENTRY_ENVIRONMENT")
                        .unwrap_or_else(|_| "production".into())
                        .into(),
                ),
                traces_sample_rate: 0.2,
                ..Default::default()
            },
        ));
        Box::leak(Box::new(_guard));
        tracing::info!("Sentry initialized");
    } else {
        tracing::warn!("SENTRY_DSN not set — Sentry disabled");
    }
}

/// Create a shutdown signal future (Ctrl+C / SIGTERM)
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
            .recv().await;
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

#[tokio::main]
async fn main() {
    let log_dir = std::path::Path::new("./logs");
    std::fs::create_dir_all(log_dir).ok();

    let file_appender = tracing_appender::rolling::daily(log_dir, "atmos.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(_guard));

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // Write to both stdout (human-readable) and file (JSON) simultaneously
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout);
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    dotenvy::dotenv().ok();

    init_sentry();

    let config = AppConfig::from_env();

    // Initialize data and media directories
    tokio::fs::create_dir_all("./data").await
        .unwrap_or_else(|e| panic!("Failed to create data directory: {}", e));
    tokio::fs::create_dir_all(&config.media_root).await
        .unwrap_or_else(|e| panic!("Failed to create media root directory: {}", e));

    let app = build_router(config.clone()).await;

    // Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let listener = tokio::net::TcpListener::bind(addr).await
        .unwrap_or_else(|e| panic!("failed to bind TCP listener on {}: {}", addr, e));

    tracing::info!("Atmos Video server starting on http://{}", addr);
    tracing::info!("Media root: {}", config.media_root.display());

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| tracing::error!("server exited with error: {}", e));

    tracing::info!("Server shutdown complete");
}
