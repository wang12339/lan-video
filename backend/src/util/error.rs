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
    /// 409 —— 资源冲突
    #[error("{0}")]
    Conflict(String),
    /// 409 —— 资源重复/冲突（如用户名已存在、视频已存在）
    #[error("{0}")]
    Duplicate(String),
    /// 507 —— 配额超限（如存储配额已满）
    #[error("{0}")]
    QuotaExceeded(String),
    /// 422 —— 输入验证失败（如密码强度不足）
    #[error("{0}")]
    Validation(String),
    /// 500 —— payload 只进日志，响应固定为 "服务器内部错误"
    #[error("{0}")]
    Internal(String),
}

static INTERNAL_ERROR_MSG: &str = "服务器内部错误";
static RATE_LIMITED_MSG: &str = "请求过于频繁，请稍后再试";

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

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    pub fn duplicate(msg: impl Into<String>) -> Self {
        Self::Duplicate(msg.into())
    }

    pub fn quota_exceeded(msg: impl Into<String>) -> Self {
        Self::QuotaExceeded(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        let msg = msg.into();
        tracing::error!("service error: {}", msg);
        Self::Internal(msg)
    }

    pub fn into_tuple(self) -> (StatusCode, Json<ErrorResponse>) {
        let (status, msg) = match self {
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, RATE_LIMITED_MSG.into()),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            Self::Conflict(m) => (StatusCode::CONFLICT, m),
            Self::Duplicate(m) => (StatusCode::CONFLICT, m),
            Self::QuotaExceeded(m) => (StatusCode::INSUFFICIENT_STORAGE, m),
            Self::Validation(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERROR_MSG.into()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_into_tuple() {
        let (status, body) = ServiceError::NotFound("资源不存在".into()).into_tuple();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0.error, "资源不存在");
    }

    #[test]
    fn test_forbidden_into_tuple() {
        let (status, body) = ServiceError::Forbidden("权限不足".into()).into_tuple();
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body.0.error, "权限不足");
    }

    #[test]
    fn test_rate_limited_into_tuple() {
        let (status, body) = ServiceError::RateLimited.into_tuple();
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body.0.error, "请求过于频繁，请稍后再试");
    }

    #[test]
    fn test_bad_request_into_tuple() {
        let (status, body) = ServiceError::BadRequest("参数无效".into()).into_tuple();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0.error, "参数无效");
    }

    #[test]
    fn test_conflict_into_tuple() {
        let (status, body) = ServiceError::Conflict("资源冲突".into()).into_tuple();
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body.0.error, "资源冲突");
    }

    #[test]
    fn test_duplicate_into_tuple() {
        let (status, body) = ServiceError::Duplicate("用户名已存在".into()).into_tuple();
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body.0.error, "用户名已存在");
    }

    #[test]
    fn test_quota_exceeded_into_tuple() {
        let (status, body) = ServiceError::QuotaExceeded("配额已满".into()).into_tuple();
        assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
        assert_eq!(body.0.error, "配额已满");
    }

    #[test]
    fn test_validation_into_tuple() {
        let (status, body) = ServiceError::Validation("密码强度不足".into()).into_tuple();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.0.error, "密码强度不足");
    }

    #[test]
    fn test_internal_into_tuple_hides_details() {
        let (status, body) =
            ServiceError::Internal("secret db connection info".into()).into_tuple();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        // Internal 的 payload 不泄露给客户端，固定返回 "服务器内部错误"
        assert_eq!(body.0.error, "服务器内部错误");
    }

    #[test]
    fn test_convenience_constructors() {
        // 验证便捷构造函数生成正确的变体
        let (s, _) = ServiceError::not_found("x").into_tuple();
        assert_eq!(s, StatusCode::NOT_FOUND);

        let (s, _) = ServiceError::forbidden("x").into_tuple();
        assert_eq!(s, StatusCode::FORBIDDEN);

        let (s, _) = ServiceError::bad_request("x").into_tuple();
        assert_eq!(s, StatusCode::BAD_REQUEST);

        let (s, _) = ServiceError::conflict("x").into_tuple();
        assert_eq!(s, StatusCode::CONFLICT);

        let (s, _) = ServiceError::duplicate("x").into_tuple();
        assert_eq!(s, StatusCode::CONFLICT);

        let (s, _) = ServiceError::quota_exceeded("x").into_tuple();
        assert_eq!(s, StatusCode::INSUFFICIENT_STORAGE);

        let (s, _) = ServiceError::validation("x").into_tuple();
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);

        let (s, b) = ServiceError::internal("x").into_tuple();
        assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(b.0.error, "服务器内部错误");
    }
}
