use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    Json,
};
use serde::de::DeserializeOwned;
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

/// JSON wrapper that sanitizes deserialization errors to avoid info leakage.
/// Replace `Json<T>` with `SafeJson<T>` in handler signatures.
pub struct SafeJson<T>(pub T);

impl<T, S> FromRequest<S> for SafeJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
    Json<T>: FromRequest<S, Rejection = axum::extract::rejection::JsonRejection>,
{
    type Rejection = (StatusCode, Json<ErrorResponse>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(SafeJson(value.0)),
            Err(rejection) => {
                tracing::debug!("JSON deserialization error: {}", rejection.body_text());
                Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid request body",
                ))
            }
        }
    }
}

impl<T> std::ops::Deref for SafeJson<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_error_response_format() {
        let (status, body) = error_response(StatusCode::NOT_FOUND, "not found");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0.error, "not found");
    }

    #[test]
    fn test_error_response_into_string() {
        let (status, body) = error_response(StatusCode::BAD_REQUEST, format!("invalid input: {}", 42));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0.error, "invalid input: 42");
    }

    #[test]
    fn test_safe_json_deref() {
        let wrapper = SafeJson(42u32);
        assert_eq!(*wrapper, 42);
    }
}
