use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct ServerInfo {
    pub version: String,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn server_info() -> Json<ServerInfo> {
    Json(ServerInfo {
        version: "1.0".to_string(),
    })
}

/// GET /health — Docker/Kubernetes liveness probe
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}
