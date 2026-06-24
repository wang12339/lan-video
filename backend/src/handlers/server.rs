use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

use crate::state::AppState;

/// Server start time for uptime reporting
static START_TIME: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

#[derive(Serialize)]
pub struct ServerInfo {
    pub version: String,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub uptime_secs: u64,
}

pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /health — liveness + readiness probe
///
/// Pings the database with `SELECT 1`. Returns 200 with `{"status":"ok","db":"ok",...}`
/// or 503 with `{"status":"degraded","db":"unreachable",...}` if the DB is down.
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime_secs = START_TIME.elapsed().as_secs();

    match sqlx::query("SELECT 1").execute(&state.db_pool).await {
        Ok(_) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok".into(),
                db: "ok".into(),
                uptime_secs,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Health check DB ping failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "degraded".into(),
                    db: "unreachable".into(),
                    uptime_secs,
                }),
            )
                .into_response()
        }
    }
}
