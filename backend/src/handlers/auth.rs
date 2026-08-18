use axum::{
    extract::{ConnectInfo, Multipart, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::middleware::auth::{self as auth_mw, AuthUser};
use crate::middleware::tenant::TenantContext;
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::state::AppState;
use crate::util::response::{error_response, internal_error_log, ErrorResponse};

/// Build an auth response, setting the token cookie if present
fn auth_response(resp: AuthResponse, state: &AppState) -> impl IntoResponse {
    if let Some(token) = resp.token.clone() {
        let mut http_resp = Json(resp).into_response();
        http_resp.headers_mut().insert(
            axum::http::header::SET_COOKIE,
            HeaderValue::from_str(&auth_mw::set_token_cookie(
                &token,
                crate::services::auth_service::COOKIE_MAX_AGE,
                state.config.cookie_secure,
            ))
            .expect("valid cookie header"),
        );
        http_resp
    } else {
        Json(resp).into_response()
    }
}

/// Handle an auth result (register or login), mapping errors appropriately
fn handle_auth_result(
    result: Result<AuthResponse, crate::util::error::ServiceError>,
    state: &AppState,
) -> axum::response::Response {
    match result {
        Ok(resp) => auth_response(resp, state).into_response(),
        Err(crate::util::error::ServiceError::RateLimited) => {
            tracing::warn!("Auth request rate limited");
            Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("尝试次数过多，请稍后再试".into()),
            })
            .into_response()
        }
        Err(crate::util::error::ServiceError::Internal(e)) => {
            tracing::error!("Auth internal error: {}", e);
            Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("服务器内部错误".into()),
            })
            .into_response()
        }
        Err(other) => other.into_response(),
    }
}

/// POST /auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Extension(tenant): Extension<TenantContext>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    // SECURITY (API F-002): the registration toggle is checked *before* JSON
    // parsing so that the error response is uniform regardless of payload
    // shape. Previously, `SafeJson` rejected empty/invalid bodies with 400
    // while the toggle check returned 404, leaking both the endpoint's
    // existence and the registration state.
    if !state.config.registration_enabled() {
        return Err(error_response(StatusCode::NOT_FOUND, "Not Found"));
    }
    // Real client IP from Cloudflare if available — falls back to socket peer.
    let client_ip = client_ip_from_headers(&headers, addr);

    // Manually parse the JSON body so we control the error message.
    let req: AuthRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "invalid request body",
            ))
        }
    };

    let svc = state.services.auth.clone();
    Ok(handle_auth_result(
        svc.register(&req, &client_ip, tenant.tenant_id).await,
        &state,
    ))
}

/// POST /auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Extension(tenant): Extension<TenantContext>,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let client_ip = client_ip_from_headers(&headers, addr);
    let req: AuthRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "invalid request body",
            ))
        }
    };
    let svc = state.services.auth.clone();
    Ok(handle_auth_result(
        svc.login(&req, &client_ip, tenant.tenant_id).await,
        &state,
    ))
}

/// POST /auth/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| auth_mw::extract_token_from_cookie(&headers));

    let username = match token.as_deref() {
        Some(t) => state
            .repos
            .user
            .find_user_by_token(t)
            .await
            .ok()
            .flatten()
            .map(|u| u.username),
        None => None,
    };

    let svc = state.services.auth.clone();
    svc.logout(username.as_deref(), token.as_deref()).await;

    let mut resp = Json(AuthResponse {
        ok: true,
        token: None,
        error: None,
    })
    .into_response();
    resp.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&auth_mw::clear_token_cookie(state.config.cookie_secure))
            .expect("valid cookie header"),
    );
    resp
}

/// Resolve the real client IP from proxy headers and fall back to the direct
/// socket peer. SECURITY: this is the IP we feed into per-IP rate limiters, so
/// trusting the wrong header would let attackers evade limits. The header is
/// only honoured when the direct peer is a trusted proxy:
///
/// - `cf-connecting-ip` is accepted only when the peer is inside Cloudflare's
///   published ranges (origin sits behind Cloudflare in production), or
///   unconditionally when `TRUSTED_PROXY=1` is set for custom proxies.
/// - `X-Forwarded-For` is accepted only when `TRUSTED_PROXY=1` is set.
///
/// A peer connecting straight to the origin can therefore never spoof the
/// client IP used for rate limiting.
fn client_ip_from_headers(headers: &axum::http::HeaderMap, peer: SocketAddr) -> String {
    let trusted_proxy = std::env::var("TRUSTED_PROXY")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if trusted_proxy {
        // X-Forwarded-For is "client, proxy1, proxy2" — the leftmost entry is
        // the client. cf-connecting-ip (Cloudflare) may also be present.
        for name in ["cf-connecting-ip", "x-forwarded-for"] {
            if let Some(ip) = headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(str::trim)
                .and_then(|s| s.parse::<std::net::IpAddr>().ok())
            {
                return ip.to_string();
            }
        }
    } else if is_cloudflare_peer(peer.ip()) {
        if let Some(ip) = headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<std::net::IpAddr>().ok())
        {
            return ip.to_string();
        }
    }
    peer.ip().to_string()
}

/// Cloudflare's published IPv4 ranges (https://www.cloudflare.com/ips/).
/// Only peers in these ranges may send `cf-connecting-ip` when
/// `TRUSTED_PROXY` is not set.
const CLOUDFLARE_IPV4: &[(&str, u8)] = &[
    ("173.245.48.0", 20),
    ("103.21.244.0", 22),
    ("103.22.200.0", 22),
    ("103.31.4.0", 22),
    ("141.101.64.0", 18),
    ("108.162.192.0", 18),
    ("190.93.240.0", 20),
    ("188.114.96.0", 20),
    ("197.234.240.0", 22),
    ("198.41.128.0", 17),
    ("162.158.0.0", 15),
    ("104.16.0.0", 13),
    ("104.24.0.0", 14),
    ("172.64.0.0", 13),
    ("131.0.72.0", 22),
];

/// Cloudflare's published IPv6 ranges.
const CLOUDFLARE_IPV6: &[(&str, u8)] = &[
    ("2400:cb00::", 32),
    ("2606:4700::", 32),
    ("2803:f800::", 32),
    ("2405:b500::", 32),
    ("2405:8100::", 32),
    ("2a06:98c0::", 29),
    ("2c0f:f248::", 32),
];

fn ipv4_in_network(ip: u32, network: u32, prefix: u8) -> bool {
    let mask = if prefix >= 32 {
        u32::MAX
    } else {
        u32::MAX << (32 - prefix)
    };
    (ip & mask) == (network & mask)
}

fn ipv6_in_network(ip: u128, network: u128, prefix: u8) -> bool {
    let mask = if prefix >= 128 {
        u128::MAX
    } else {
        u128::MAX << (128 - prefix)
    };
    (ip & mask) == (network & mask)
}

fn is_cloudflare_peer(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let ip = u32::from(v4);
            CLOUDFLARE_IPV4.iter().any(|(net, prefix)| {
                net.parse::<std::net::Ipv4Addr>()
                    .map(|n| ipv4_in_network(ip, u32::from(n), *prefix))
                    .unwrap_or(false)
            })
        }
        std::net::IpAddr::V6(v6) => {
            let ip = u128::from(v6);
            CLOUDFLARE_IPV6.iter().any(|(net, prefix)| {
                net.parse::<std::net::Ipv6Addr>()
                    .map(|n| ipv6_in_network(ip, u128::from(n), *prefix))
                    .unwrap_or(false)
            })
        }
    }
}

/// GET /auth/user
pub async fn user_info(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserInfoResponse> {
    let svc = state.services.auth.clone();
    match svc
        .user_info(&auth_user.username, auth_user.is_admin, auth_user.tenant_id)
        .await
    {
        Ok(resp) => Json(resp),
        Err(_) => Json(UserInfoResponse {
            id: auth_user.id,
            username: auth_user.username,
            is_admin: auth_user.is_admin,
            created_at: String::new(),
            email: None,
            email_verified: false,
        }),
    }
}

/// GET /auth/user/profile
pub async fn user_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserProfileResponse> {
    let svc = state.services.auth.clone();
    match svc
        .user_profile(&auth_user.username, auth_user.is_admin, auth_user.tenant_id)
        .await
    {
        Ok(resp) => Json(resp),
        Err(_) => Json(UserProfileResponse {
            username: auth_user.username,
            is_admin: auth_user.is_admin,
            created_at: String::new(),
            total_videos_watched: 0,
            total_watch_time_ms: 0,
            recent_history: vec![],
        }),
    }
}

/// POST /auth/user/avatar
pub async fn upload_avatar(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_ext: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("Multipart error: {}", e);
        error_response(StatusCode::BAD_REQUEST, "上传数据无效")
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                tracing::warn!("Read multipart error: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "读取文件失败")
            })?;
            if data.len() > 5 * 1024 * 1024 {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "头像文件不能超过 5MB",
                ));
            }
            // SECURITY (A08-02): don't trust the client's Content-Type. Use
            // magic-byte sniffing to identify the real image format. This
            // blocks uploads of HTML/SVG/JS renamed to .png/.jpg.
            let (ext, _mime) = match crate::services::media_service::infer_image(&data) {
                Some(t) => t,
                None => {
                    return Err(error_response(
                        StatusCode::BAD_REQUEST,
                        "不支持的图片格式，请上传 JPG/PNG/WebP/GIF",
                    ))
                }
            };
            file_ext = Some(ext.to_string());
            file_data = Some(data.to_vec());
        }
    }

    let data = file_data.ok_or(error_response(
        StatusCode::BAD_REQUEST,
        "请选择要上传的文件",
    ))?;
    let ext = file_ext.unwrap_or_else(|| "jpg".into());

    // Save to media/avatars/
    let avatars_dir = state.config.media_root.join("avatars");
    tokio::fs::create_dir_all(&avatars_dir).await.map_err(|e| {
        tracing::error!("Failed to create avatars dir: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "创建目录失败")
    })?;

    let filename = format!("{}.{}", auth_user.id, ext);
    let path = avatars_dir.join(&filename);
    let path_clone = path.clone();

    tokio::task::spawn_blocking(move || std::fs::write(&path_clone, &data))
        .await
        .map_err(|e| {
            tracing::error!("Spawn blocking error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "保存文件失败")
        })?
        .map_err(|e| {
            tracing::error!("Write file error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "保存文件失败")
        })?;

    let avatar_url = format!("/media/avatars/{}", filename);

    // Update user avatar in database
    let db_result = state
        .repos
        .user
        .update_avatar(auth_user.id, &avatar_url)
        .await;

    if let Err(e) = db_result {
        tracing::error!("Update avatar error: {}", e);
        // Don't leave an orphan file behind: the DB no longer references it,
        // so it would be unreachable garbage on disk.
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "更新头像失败",
        ));
    }

    // Clean up old avatars with different extensions (incl. bmp, which
    // infer_image can also accept)
    let avatars_dir = state.config.media_root.join("avatars");
    for ext in &["jpg", "png", "webp", "gif", "bmp"] {
        let old_path = avatars_dir.join(format!("{}.{}", auth_user.id, ext));
        if old_path != path && tokio::fs::metadata(&old_path).await.is_ok() {
            let _ = tokio::fs::remove_file(old_path).await;
        }
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "avatarUrl": avatar_url
    })))
}

#[derive(serde::Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(serde::Serialize)]
pub struct ForgotPasswordResponse {
    pub ok: bool,
    pub message: String,
}

/// POST /auth/forgot-password
pub async fn forgot_password(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<ForgotPasswordRequest>,
) -> Json<ForgotPasswordResponse> {
    // Emails are stored lowercased (see update_email), so normalise before
    // both the rate-limit key and the lookup to stay consistent.
    let email = req.email.trim().to_lowercase();

    // IP 级速率限制：每分钟最多 5 次请求
    let ip_key = format!("forgot_pwd:ip:{}", addr.ip());
    if state
        .ip_rate_limiter
        .check_with(&ip_key, 5, 60, 300)
        .await
        .is_err()
    {
        return Json(ForgotPasswordResponse {
            ok: true,
            message: "请求过于频繁，请稍后再试。".into(),
        });
    }

    // 邮箱级速率限制：每 5 分钟最多 2 次请求
    let email_key = format!("forgot_pwd:email:{}", email);
    if state
        .rate_limiter
        .check_with(&email_key, 2, 300, 600)
        .await
        .is_err()
    {
        return Json(ForgotPasswordResponse {
            ok: true,
            message: "请求过于频繁，请稍后再试。".into(),
        });
    }

    // SECURITY: the DB lookup, token creation and the (potentially hundreds of
    // ms) SMTP round-trip happen in a background task. The response always
    // returns immediately with the same body, so response *timing* cannot be
    // used to enumerate which emails are registered.
    let state = state.clone();
    tokio::spawn(async move {
        let Some(user) = state.repos.user.find_by_email(&email).await.ok().flatten() else {
            return;
        };
        let Some(token) = state
            .repos
            .user
            .create_password_reset_token(user.id)
            .await
            .ok()
        else {
            return;
        };
        let reset_url = format!(
            "{}/auth/reset-password?token={}",
            state.config.public_url.trim_end_matches('/'),
            token
        );
        state
            .services
            .email
            .send_password_reset(&email, &user.username, &reset_url)
            .await;
    });

    Json(ForgotPasswordResponse {
        ok: true,
        message: "如果该邮箱已注册，您将收到密码重置邮件。请检查您的收件箱。".into(),
    })
}

#[derive(serde::Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

/// POST /auth/reset-password
pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Validate the raw password, not a trimmed copy — register no longer
    // trims either, so both paths agree on what is hashed.
    let password = &req.password;
    let pw_len = password.chars().count();
    if !(8..=128).contains(&pw_len) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "密码长度需在 8-128 个字符之间",
        ));
    }

    if !crate::services::auth_service::is_password_strong_enough(password) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "密码过于简单，请使用包含大小写字母、数字、特殊字符中至少三种的密码",
        ));
    }

    let user_id = state
        .repos
        .user
        .find_valid_reset_token(&req.token)
        .await
        .map_err(|e| internal_error_log("find_valid_reset_token", &e))?
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "重置链接无效或已过期"))?;

    let hash = crate::util::password::hash(password)
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "密码处理失败"))?;

    let updated = state
        .repos
        .user
        .update_password_hash(user_id, &hash)
        .await
        .map_err(|e| internal_error_log("update_password_hash", &e))?;

    if !updated {
        // The user was deleted between token validation and the update.
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "重置链接无效或已过期",
        ));
    }

    if let Err(e) = state.repos.user.revoke_tokens_by_user_id(user_id).await {
        tracing::error!("revoke_tokens_by_user_id after password reset: {}", e);
    }

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "密码已重置" }),
    ))
}

#[derive(serde::Deserialize)]
pub struct ResetPasswordToken {
    pub token: String,
}

/// GET /auth/reset-password?token=xxx
///
/// Handles password reset links from emails. Redirects to the frontend
/// reset password page where the user can enter a new password.
pub async fn reset_password_get(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ResetPasswordToken>,
) -> axum::response::Redirect {
    let base = state.config.public_url.trim_end_matches('/');
    // Redirect to frontend with token in query params
    // The frontend AuthDialog will detect this and show the reset password form
    axum::response::Redirect::to(&format!("{}/webapp/?reset_token={}", base, params.token))
}

#[derive(serde::Deserialize)]
pub struct UpdateEmailRequest {
    pub email: String,
}

/// PUT /auth/user/email
pub async fn update_email(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<UpdateEmailRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let email = req.email.trim().to_lowercase();

    // Enhanced email validation
    if !is_valid_email(&email) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "请输入有效的邮箱地址",
        ));
    }

    state
        .repos
        .user
        .update_email(auth_user.id, &email)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("idx_users_email_unique") {
                    return error_response(StatusCode::CONFLICT, "该邮箱已被其他账号绑定");
                }
            }
            tracing::error!("update_email: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "邮箱已更新，验证状态已重置，请重新验证新邮箱" }),
    ))
}

fn is_valid_email(email: &str) -> bool {
    if email.is_empty() || email.len() > 254 {
        return false;
    }
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return false;
    }
    let (local, domain) = (parts[0], parts[1]);
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    if domain.contains("..") {
        return false;
    }
    // Reject whitespace and control characters: they break the envelope
    // and can be abused for header/command injection in SMTP.
    if email.chars().any(char::is_whitespace) || email.chars().any(char::is_control) {
        return false;
    }
    true
}

#[derive(serde::Serialize)]
pub struct SendVerificationEmailResponse {
    pub ok: bool,
    pub message: String,
}

/// POST /auth/send-verification-email
pub async fn send_verification_email(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<SendVerificationEmailResponse> {
    // 用户级速率限制：每 5 分钟最多 2 次请求
    let key = format!("verify_email:user:{}", auth_user.id);
    if state
        .rate_limiter
        .check_with(&key, 2, 300, 600)
        .await
        .is_err()
    {
        return Json(SendVerificationEmailResponse {
            ok: false,
            message: "请求过于频繁，请稍后再试。".into(),
        });
    }

    let email = state
        .repos
        .user
        .get_email(auth_user.id)
        .await
        .ok()
        .flatten();

    if state.services.email.is_configured() {
        if let Some(ref email) = email {
            if let Ok(token) = state
                .repos
                .user
                .create_email_verification_token(auth_user.id)
                .await
            {
                let verify_url = format!(
                    "{}/auth/verify-email?token={}",
                    state.config.public_url.trim_end_matches('/'),
                    token
                );
                state
                    .services
                    .email
                    .send_email_verification(email, &auth_user.username, &verify_url)
                    .await;
            }
        }

        Json(SendVerificationEmailResponse {
            ok: true,
            message: "验证邮件已发送。如果您的邮箱没有收到，请稍后再试。".into(),
        })
    } else {
        // SMTP 未配置时直接标记已验证（开发/测试模式）
        if email.is_some() {
            let _ = state.repos.user.verify_email(auth_user.id).await;
        }

        Json(SendVerificationEmailResponse {
            ok: true,
            message: "验证邮件功能未配置。请联系管理员。".into(),
        })
    }
}

#[derive(serde::Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

/// GET /auth/verify-email?token=xxx
///
/// Handles email verification links from emails. Verifies the token
/// and shows a success/failure page directly.
pub async fn verify_email_get(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<VerifyEmailRequest>,
) -> axum::response::Response {
    let result = async {
        let user_id = state
            .repos
            .user
            .find_valid_email_verification_token(&params.token)
            .await
            .map_err(|e| {
                tracing::error!("find_valid_email_verification_token: {}", e);
                "服务器内部错误"
            })?
            .ok_or("验证链接无效或已过期")?;

        state.repos.user.verify_email(user_id).await.map_err(|e| {
            tracing::error!("verify_email: {}", e);
            "服务器内部错误"
        })?;

        Ok::<_, &str>(())
    }
    .await;

    let base = state.config.public_url.trim_end_matches('/');
    match result {
        Ok(_) => {
            let html = format!(
                r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>邮箱验证成功</title>
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif; background: #0a0a0e; color: #fff; min-height: 100vh; display: flex; align-items: center; justify-content: center; }}
    .card {{ background: #15151a; border: 1px solid rgba(255,255,255,0.12); border-radius: 16px; padding: 48px; text-align: center; max-width: 420px; width: 90%; }}
    .icon {{ font-size: 64px; margin-bottom: 24px; }}
    h1 {{ font-size: 24px; margin-bottom: 12px; }}
    p {{ color: #9a9aa6; font-size: 15px; line-height: 1.6; margin-bottom: 32px; }}
    a {{ display: inline-block; background: #ff4433; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 500; font-size: 15px; transition: background 0.2s; }}
    a:hover {{ background: #ff6655; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="icon">✅</div>
    <h1>邮箱验证成功</h1>
    <p>您的邮箱已成功验证！<br>现在可以正常使用所有功能。</p>
    <a href="{}/webapp/">进入 Atmos Video</a>
  </div>
</body>
</html>"#,
                base
            );
            axum::response::Html(html).into_response()
        }
        Err(_) => {
            let html = format!(
                r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>邮箱验证失败</title>
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif; background: #0a0a0e; color: #fff; min-height: 100vh; display: flex; align-items: center; justify-content: center; }}
    .card {{ background: #15151a; border: 1px solid rgba(255,255,255,0.12); border-radius: 16px; padding: 48px; text-align: center; max-width: 420px; width: 90%; }}
    .icon {{ font-size: 64px; margin-bottom: 24px; }}
    h1 {{ font-size: 24px; margin-bottom: 12px; }}
    p {{ color: #9a9aa6; font-size: 15px; line-height: 1.6; margin-bottom: 32px; }}
    a {{ display: inline-block; background: #ff4433; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 500; font-size: 15px; transition: background 0.2s; }}
    a:hover {{ background: #ff6655; }}
  </style>
</head>
<body>
  <div class="card">
    <div class="icon">❌</div>
    <h1>邮箱验证失败</h1>
    <p>验证链接无效或已过期。<br>请登录后重新发送验证邮件。</p>
    <a href="{}/webapp/">进入 Atmos Video</a>
  </div>
</body>
</html>"#,
                base
            );
            axum::response::Html(html).into_response()
        }
    }
}

/// POST /auth/verify-email
pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyEmailRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let user_id = state
        .repos
        .user
        .find_valid_email_verification_token(&req.token)
        .await
        .map_err(|e| internal_error_log("find_valid_email_verification_token", &e))?
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "验证链接无效或已过期"))?;

    state
        .repos
        .user
        .verify_email(user_id)
        .await
        .map_err(|e| internal_error_log("verify_email", &e))?;

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "邮箱验证成功" }),
    ))
}
