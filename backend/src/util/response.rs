use axum::{http::StatusCode, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Helper to produce a consistent JSON error body across all handlers.
/// Pattern: `(StatusCode, Json<ErrorResponse>)` — the standard axum tuple `IntoResponse`.
pub fn error_response(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}
