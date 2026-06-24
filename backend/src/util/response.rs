use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    Json,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

/// Type alias for handler results — the standard axum error tuple.
/// Use this instead of writing `(StatusCode, Json<ErrorResponse>)` everywhere.
pub type HandlerResult<T> = Result<T, (StatusCode, Json<ErrorResponse>)>;

/// Helper to produce a consistent JSON error body across all handlers.
/// Pattern: `(StatusCode, Json<ErrorResponse>)` — the standard axum tuple `IntoResponse`.
pub fn error_response(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

/// Convenience: build a 400 Bad Request response
pub fn bad_request(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    error_response(StatusCode::BAD_REQUEST, msg)
}

/// Convenience: build a 401 Unauthorized response
pub fn unauthorized(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    error_response(StatusCode::UNAUTHORIZED, msg)
}

/// Convenience: build a 403 Forbidden response
pub fn forbidden(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    error_response(StatusCode::FORBIDDEN, msg)
}

/// Convenience: build a 404 Not Found response
pub fn not_found(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    error_response(StatusCode::NOT_FOUND, msg)
}

/// Convenience: build a 429 Too Many Requests response
pub fn too_many_requests(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    error_response(StatusCode::TOO_MANY_REQUESTS, msg)
}

/// Convenience: build a 500 Internal Server Error response
pub fn internal_error(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
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

    #[test]
    fn test_bad_request() {
        let (status, body) = bad_request("invalid input");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0.error, "invalid input");
    }

    #[test]
    fn test_unauthorized() {
        let (status, body) = unauthorized("need login");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.0.error, "need login");
    }

    #[test]
    fn test_forbidden() {
        let (status, body) = forbidden("admin only");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body.0.error, "admin only");
    }

    #[test]
    fn test_not_found() {
        let (status, body) = not_found("video not found");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0.error, "video not found");
    }

    #[test]
    fn test_too_many_requests() {
        let (status, body) = too_many_requests("slow down");
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body.0.error, "slow down");
    }

    #[test]
    fn test_internal_error() {
        let (status, body) = internal_error("something broke");
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.0.error, "something broke");
    }

    #[test]
    fn test_handler_result_type_alias() {
        fn example() -> HandlerResult<String> {
            Ok("hello".to_string())
        }
        let result = example();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn test_handler_result_error() {
        fn example() -> HandlerResult<String> {
            Err(not_found("not here"))
        }
        let result = example();
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0.error, "not here");
    }
}
