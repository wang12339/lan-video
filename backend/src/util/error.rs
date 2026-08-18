use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use crate::util::response::{error_response, ErrorResponse};

/// 统一的服务层错误类型 —— service → handler 的唯一错误通道。
///
/// 之前每个 service 各自定义错误枚举（`AdminError` / `AuthError` /
/// `CommentError` / `ShareError`），各自重复实现 `into_response` 与
/// `From<sqlx::Error>`。收敛到这里后：
/// - service 只构造 `ServiceError`（带具体文案），handler 直接
///   `.map_err(ServiceError::into_tuple)`；
/// - 内部错误细节（`Internal` 的 payload）只进日志，响应固定为
///   500 "服务器内部错误"，杜绝信息泄露；
/// - 新增 service 不再需要重复的错误样板。
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// 404 —— 携带具体文案（如 "用户不存在"）
    #[error("{0}")]
    NotFound(String),
    /// 403 —— 携带具体文案
    #[error("{0}")]
    Forbidden(String),
    /// 429 —— 限流
    #[error("请求过于频繁，请稍后再试")]
    RateLimited,
    /// 400 —— 携带具体文案
    #[error("{0}")]
    BadRequest(String),
    /// 500 —— payload 只进日志，响应固定为 "服务器内部错误"
    #[error("{0}")]
    Internal(String),
}

impl ServiceError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    /// 构造内部错误：立即记录日志（细节进日志，响应层固定 500）。
    pub fn internal(msg: impl Into<String>) -> Self {
        let msg = msg.into();
        tracing::error!("service error: {}", msg);
        Self::Internal(msg)
    }

    /// 转换为 axum handler 的标准错误元组。
    pub fn into_tuple(self) -> (StatusCode, Json<ErrorResponse>) {
        let (status, msg) = match self {
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "请求过于频繁，请稍后再试".into(),
            ),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误".into()),
        };
        error_response(status, msg)
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("sqlx error: {}", e);
        Self::Internal("数据库错误".into())
    }
}

impl From<String> for ServiceError {
    fn from(s: String) -> Self {
        Self::internal(s)
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        self.into_tuple().into_response()
    }
}
