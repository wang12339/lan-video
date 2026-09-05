#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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

#[test]
fn test_service_error_bad_request() {
    let err = ServiceError::BadRequest("评论内容 1-2000 字符".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body.0.error, "评论内容 1-2000 字符");
}

#[test]
fn test_service_error_bad_request_empty_message() {
    let err = ServiceError::BadRequest(String::new());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body.0.error, "");
}

#[test]
fn test_service_error_validation() {
    let err = ServiceError::Validation("密码强度不足".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body.0.error, "密码强度不足");
}

#[test]
fn test_service_error_duplicate_maps_to_conflict() {
    let err = ServiceError::Duplicate("用户名已存在".into());
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body.0.error, "用户名已存在");
}

#[test]
fn test_service_error_internal_leaks_no_detail_for_any_variant() {
    for msg in [
        "password=secret123",
        "SELECT * FROM users WHERE password='x'",
        "/var/lib/postgres/data",
        "stack overflow at 0x7fff1234",
    ] {
        let err = ServiceError::Internal(msg.to_string());
        let (status, body) = err.into_tuple();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.0.error, "服务器内部错误");
        assert!(
            !body.0.error.contains("password") && !body.0.error.contains("secret"),
            "Internal error must not leak detail: {msg}"
        );
    }
}

#[test]
fn test_service_error_convenience_constructors_produce_correct_variants() {
    let cases: Vec<(&dyn Fn(String) -> ServiceError, StatusCode)> = vec![
        (&|m| ServiceError::not_found(m), StatusCode::NOT_FOUND),
        (&|m| ServiceError::forbidden(m), StatusCode::FORBIDDEN),
        (&|m| ServiceError::bad_request(m), StatusCode::BAD_REQUEST),
        (&|m| ServiceError::conflict(m), StatusCode::CONFLICT),
        (&|m| ServiceError::duplicate(m), StatusCode::CONFLICT),
        (
            &|m| ServiceError::quota_exceeded(m),
            StatusCode::INSUFFICIENT_STORAGE,
        ),
        (
            &|m| ServiceError::validation(m),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ];
    for (ctor, expected_status) in cases {
        let (status, _) = ctor("test".to_string()).into_tuple();
        assert_eq!(status, expected_status);
    }
}

#[test]
fn test_service_error_display_trait() {
    let err = ServiceError::NotFound("资源不存在".into());
    assert_eq!(format!("{err}"), "资源不存在");

    let err = ServiceError::RateLimited;
    assert_eq!(format!("{err}"), "请求过于频繁，请稍后再试");
}

#[test]
fn test_service_error_debug_does_not_leak_into_response() {
    let err = ServiceError::Internal("super_secret_key_12345".into());
    let debug_str = format!("{:?}", err);
    assert!(
        debug_str.contains("super_secret_key_12345"),
        "Debug should contain message for logging"
    );
    let (status, body) = err.into_tuple();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.0.error, "服务器内部错误");
    assert!(
        !body.0.error.contains("super_secret"),
        "Response must not leak internal detail"
    );
}
