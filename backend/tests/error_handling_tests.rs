use atmos_video_backend::util::error::ServiceError;
use axum::http::StatusCode;

#[test]
fn test_service_error_not_found() {
    let err = ServiceError::NotFound("视频不存在".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.0.error, "视频不存在");
}

#[test]
fn test_service_error_forbidden() {
    let err = ServiceError::Forbidden("权限不足".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body.0.error, "权限不足");
}

#[test]
fn test_service_error_rate_limited() {
    let err = ServiceError::RateLimited;
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body.0.error, "请求过于频繁，请稍后再试");
}

#[test]
fn test_service_error_internal_no_leak() {
    let err = ServiceError::Internal("数据库连接失败: password=secret123".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    // 确保不泄露敏感信息
    assert!(!body.0.error.contains("secret123"));
    assert_eq!(body.0.error, "服务器内部错误");
}

#[test]
fn test_service_error_conflict() {
    let err = ServiceError::Conflict("标签已存在".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body.0.error, "标签已存在");
}

#[test]
fn test_service_error_quota_exceeded() {
    let err = ServiceError::QuotaExceeded("存储配额已用尽".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
    assert_eq!(body.0.error, "存储配额已用尽");
}
