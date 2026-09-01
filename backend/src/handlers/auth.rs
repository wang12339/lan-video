use axum::{
    extract::{Multipart, Request, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::sync::Arc;

use crate::middleware::auth::{self as auth_mw, AuthUser};
use crate::middleware::tenant::TenantContext;
use crate::models::auth::{
    AuthRequest, AuthResponse, ForgotPasswordRequest, ForgotPasswordResponse, ResetPasswordRequest,
    ResetPasswordToken, SendVerificationEmailResponse, UpdateEmailRequest, UserInfoResponse,
    UserProfileResponse, VerifyEmailRequest,
};
use crate::services::auth_service::is_valid_email;
use crate::state::AppState;
use crate::util::net::client_ip;
use crate::util::response::{error_response, internal_error_log, ErrorResponse};

const VERIFY_EMAIL_HTML: &str = include_str!("../../templates/verify_email.html");
const VERIFY_EMAIL_ERROR_HTML: &str = include_str!("../../templates/verify_email_error.html");
const BODY_LIMIT: usize = 1_048_576;

async fn parse_auth_request(
    req: Request,
) -> Result<AuthRequest, (StatusCode, Json<ErrorResponse>)> {
    let body = req.into_body();
    let body = axum::body::to_bytes(body, BODY_LIMIT).await.map_err(|e| {
        tracing::error!("Failed to read request body: {}", e);
        error_response(StatusCode::BAD_REQUEST, "invalid request body")
    })?;
    serde_json::from_slice(&body)
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "invalid request body"))
}

/// Build an auth response, setting the token cookie if present
fn auth_response(resp: AuthResponse, state: &AppState) -> impl IntoResponse {
    if let Some(ref token) = resp.token {
        let cookie_str = auth_mw::set_token_cookie(
            token,
            crate::services::auth_service::COOKIE_MAX_AGE,
            state.config.cookie_secure,
        );
        let mut http_resp = Json(resp).into_response();
        if let Ok(val) = HeaderValue::from_str(&cookie_str) {
            http_resp
                .headers_mut()
                .insert(axum::http::header::SET_COOKIE, val);
        }
        http_resp
    } else {
        Json(resp).into_response()
    }
}

fn handle_auth_result(
    result: Result<AuthResponse, crate::util::error::ServiceError>,
    state: &AppState,
) -> axum::response::Response {
    match result {
        Ok(resp) if resp.ok => auth_response(resp, state).into_response(),
        Ok(resp) => (StatusCode::UNAUTHORIZED, Json(resp)).into_response(),
        Err(crate::util::error::ServiceError::RateLimited) => {
            tracing::warn!("Auth request rate limited");
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthResponse {
                    ok: false,
                    token: None,
                    error: Some("请求过于频繁，请稍后再试".into()),
                }),
            )
                .into_response()
        }
        Err(crate::util::error::ServiceError::BadRequest(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                ok: false,
                token: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => {
            let (status, msg) = e.into_tuple();
            (
                status,
                Json(AuthResponse {
                    ok: false,
                    token: None,
                    error: Some(msg.0.error),
                }),
            )
                .into_response()
        }
    }
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<TenantContext>,
    req: Request,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let global_enabled = state.config.registration_enabled();
    let tenant_enabled = state
        .repos
        .tenant
        .get_by_id(tenant.tenant_id)
        .await
        .ok()
        .flatten()
        .map(|c| c.settings.registration_enabled)
        .unwrap_or(false);

    if !global_enabled && !tenant_enabled {
        return Err(error_response(StatusCode::NOT_FOUND, "Not Found"));
    }

    let ip = client_ip(&req);
    let auth_req = parse_auth_request(req).await?;

    Ok(handle_auth_result(
        state
            .services
            .auth
            .register(&auth_req, &ip, tenant.tenant_id)
            .await,
        &state,
    ))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<TenantContext>,
    req: Request,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let ip = client_ip(&req);
    let auth_req = parse_auth_request(req).await?;

    let result = state
        .services
        .auth
        .login(&auth_req, &ip, tenant.tenant_id)
        .await;
    if let Ok(ref resp) = result {
        if !resp.ok {
            let fail_key = format!("login_fail:{}", ip);
            if state
                .ip_rate_limiter
                .check_with(&fail_key, 10, 300, 0)
                .await
                .is_err()
            {
                tracing::warn!(
                    ip = %ip,
                    "suspicious login activity: repeated failures from same IP"
                );
            }
        }
    }
    Ok(handle_auth_result(result, &state))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let token = auth_mw::extract_bearer_token(&headers)
        .or_else(|| auth_mw::extract_token_from_cookie(&headers));

    state.services.auth.logout(None, token.as_deref()).await;

    let mut resp = Json(AuthResponse {
        ok: true,
        token: None,
        error: None,
    })
    .into_response();
    if let Ok(val) = HeaderValue::from_str(&auth_mw::clear_token_cookie(state.config.cookie_secure))
    {
        resp.headers_mut()
            .insert(axum::http::header::SET_COOKIE, val);
    }
    resp
}

/// GET /auth/user
pub async fn user_info(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserInfoResponse> {
    match state
        .services
        .auth
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
    match state
        .services
        .auth
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

    let avatars_dir = state.config.media_root.join("avatars");
    tokio::fs::create_dir_all(&avatars_dir).await.map_err(|e| {
        tracing::error!("Failed to create avatars dir: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "创建目录失败")
    })?;

    let filename = format!("{}.{}", auth_user.id, ext);
    let path = avatars_dir.join(&filename);

    let write_data = data;
    tokio::task::spawn_blocking(move || std::fs::write(&path, &write_data))
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

    let db_result = state
        .repos
        .user
        .update_avatar(auth_user.id, &avatar_url)
        .await;

    if let Err(e) = db_result {
        tracing::error!("Update avatar error: {}", e);
        let path = avatars_dir.join(&filename);
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "更新头像失败",
        ));
    }

    let current_path = avatars_dir.join(&filename);
    for ext in &["jpg", "png", "webp", "gif", "bmp"] {
        let old_path = avatars_dir.join(format!("{}.{}", auth_user.id, ext));
        if old_path != current_path && tokio::fs::metadata(&old_path).await.is_ok() {
            let _ = tokio::fs::remove_file(old_path).await;
        }
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "avatarUrl": avatar_url
    })))
}

pub async fn forgot_password(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Json<ForgotPasswordResponse> {
    let ip = client_ip(&req);

    let body = req.into_body();
    let body = match axum::body::to_bytes(body, BODY_LIMIT).await {
        Ok(b) => b,
        Err(_) => {
            return Json(ForgotPasswordResponse {
                ok: false,
                message: "请求无效".into(),
            });
        }
    };
    let forgot_req: ForgotPasswordRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return Json(ForgotPasswordResponse {
                ok: false,
                message: "请求无效".into(),
            });
        }
    };

    let email = forgot_req.email.trim().to_lowercase();

    let ip_key = format!("forgot_pwd:ip:{}", ip);
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

    // Argon2 哈希是 CPU 密集(约 100ms),必须移到 blocking 池,
    // 否则会卡住 async worker(与 auth_service 的 reset 路径一致)。
    let password_owned = password.to_string();
    let hash = tokio::task::spawn_blocking(move || crate::util::password::hash(&password_owned))
        .await
        .map_err(|e| internal_error_log("password hash join error", &e))?
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

    state.metrics.record_password_reset();

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "密码已重置" }),
    ))
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

pub async fn send_verification_email(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<SendVerificationEmailResponse> {
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
        if email.is_some() {
            let _ = state.repos.user.verify_email(auth_user.id).await;
        }

        Json(SendVerificationEmailResponse {
            ok: true,
            message: "验证邮件功能未配置。请联系管理员。".into(),
        })
    }
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
            let html = VERIFY_EMAIL_HTML.replace("{{BASE_URL}}", base);
            axum::response::Html(html).into_response()
        }
        Err(_) => {
            let html = VERIFY_EMAIL_ERROR_HTML.replace("{{BASE_URL}}", base);
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
