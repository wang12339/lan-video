use axum::{
    extract::{Multipart, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::sync::Arc;

use crate::middleware::auth::{self as auth_mw, AuthUser};
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
    // CSRF 防线：登录/注册只接受 application/json（大小写不敏感，允许
    // `; charset=` 参数）。浏览器跨站表单只能发 text/plain / urlencoded，
    // 这里直接 415，阻断无需 CORS 预检的跨站登录/注册。
    let media_type = req
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .unwrap_or("");
    if !media_type.eq_ignore_ascii_case("application/json") {
        return Err(error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported media type, expected application/json",
        ));
    }

    let body = req.into_body();
    let body = axum::body::to_bytes(body, BODY_LIMIT).await.map_err(|e| {
        tracing::error!("Failed to read request body: {}", e);
        error_response(StatusCode::BAD_REQUEST, "invalid request body")
    })?;
    serde_json::from_slice(&body)
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "invalid request body"))
}

/// Build an auth response, setting the token cookie if present
pub(crate) fn auth_response(resp: AuthResponse, state: &AppState) -> impl IntoResponse {
    if let Some(ref token) = resp.token {
        let cookie_str = auth_mw::set_token_cookie(
            token,
            crate::services::auth_service::COOKIE_MAX_AGE,
            state.config.cookie_secure,
        );
        // L1 配套：登录成功时下发 csrf_token cookie（非 HttpOnly，前端
        // JS 读取后以 X-CSRF-Token 头回传，csrf_guard 严格比对）。
        let csrf_str = auth_mw::set_csrf_cookie(
            crate::services::auth_service::COOKIE_MAX_AGE,
            state.config.cookie_secure,
        );
        let mut http_resp = Json(resp).into_response();
        if let Ok(val) = HeaderValue::from_str(&cookie_str) {
            http_resp
                .headers_mut()
                .insert(axum::http::header::SET_COOKIE, val);
        }
        if let Ok(val) = HeaderValue::from_str(&csrf_str) {
            http_resp
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, val);
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

/// 访客模式：匿名期间上传的内容合并。
///
/// 登录/注册成功后，如果请求 cookie 里还带着访客影子账号的会话 token，
/// 把该访客名下的全部内容（视频/播放历史/点赞/收藏/播放列表/评论/弹幕）
/// 合并到刚认证的真实账号，然后删除影子账号。
async fn merge_guest_session_if_any(state: &AppState, headers: &HeaderMap, new_token: &str) {
    let Some(real_user) = state.services.auth.user_for_token(new_token).await else {
        return;
    };
    // 访客内容合并对管理员同样生效（管理员也可能是曾经的访客）
    let Some(guest_token) = auth_mw::extract_token_from_cookie(headers) else {
        return;
    };
    if guest_token == new_token {
        return;
    }
    let Some(guest) = state.services.auth.user_for_token(&guest_token).await else {
        return;
    };
    if !guest.is_guest || guest.id == real_user.id {
        return;
    }
    match state
        .services
        .auth
        .merge_guest_into(guest.id, real_user.id)
        .await
    {
        Ok(n) => tracing::info!(
            guest = %guest.username,
            user = %real_user.username,
            merged_videos = n,
            "guest content merged into real account"
        ),
        Err(e) => tracing::error!(
            guest_id = guest.id,
            user_id = real_user.id,
            "guest content merge failed: {}",
            e
        ),
    }
}

/// 新用户注册待审批：异步邮件通知管理员（不阻塞注册响应）。
///
/// 收件人 = 管理员账号邮箱 + 可选 `ADMIN_NOTIFY_EMAILS`（逗号分隔，用于
/// 管理员账号未填邮箱或需要分发列表的场景），去重后逐个发送。
/// SMTP 未配置时 `EmailService::send` 只记日志，不会报错。
fn notify_admins_new_registration(state: Arc<AppState>, username: String) {
    tokio::spawn(async move {
        let mut recipients = match state.services.auth.admin_emails().await {
            Ok(list) => list,
            Err(e) => {
                tracing::warn!(
                    "pending-registration notice: failed to load admin emails: {}",
                    e
                );
                Vec::new()
            }
        };
        if let Ok(extra) = std::env::var("ADMIN_NOTIFY_EMAILS") {
            recipients.extend(
                extra
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            );
        }
        recipients.sort();
        recipients.dedup();

        if recipients.is_empty() {
            tracing::info!("pending-registration notice: no admin email configured, skipping");
            return;
        }

        let admin_url = format!(
            "{}/webapp/admin?tab=users",
            state.config.public_url.trim_end_matches('/')
        );
        for email in &recipients {
            state
                .services
                .email
                .send_pending_registration_notice(email, &username, &admin_url)
                .await;
        }
        tracing::info!(
            user = %username,
            recipients = recipients.len(),
            "pending-registration notice sent"
        );
    });
}

#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    summary = "Register a new user",
    description = "Create a new user account. Requires REGISTRATION_ENABLED=true. The first registered user becomes admin only when ALLOW_FIRST_USER_ADMIN=true; otherwise new users start as viewers and need admin approval. Rate-limited per username.",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Registration result", body = AuthResponse),
        (status = 429, description = "Too many attempts — rate limited", body = AuthResponse)
    )
)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let global_enabled = state.config.registration_enabled();

    if !global_enabled {
        return Err(error_response(StatusCode::NOT_FOUND, "Not Found"));
    }

    let headers = req.headers().clone();
    let ip = client_ip(&req);
    let auth_req = parse_auth_request(req).await?;

    let result = state.services.auth.register(&auth_req, &ip).await;
    if let Ok(ref resp) = result {
        if resp.ok {
            // Only successful registrations count; rejected ones (duplicate
            // username, weak password, registration closed) are not signups.
            state.metrics.record_register();
            if let Some(token) = &resp.token {
                merge_guest_session_if_any(&state, &headers, token).await;
            } else {
                // 无 token = 等待管理员审批的新注册 → 邮件通知管理员
                notify_admins_new_registration(state.clone(), auth_req.username);
            }
        }
    }

    Ok(handle_auth_result(result, &state))
}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    summary = "Login with username and password",
    description = "Authenticate and receive a bearer token. Rate-limited per username.",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Login result with auth token", body = AuthResponse),
        (status = 429, description = "Too many attempts — rate limited", body = AuthResponse)
    )
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let headers = req.headers().clone();
    let ip = client_ip(&req);
    let auth_req = parse_auth_request(req).await?;

    let result = state.services.auth.login(&auth_req, &ip).await;
    match &result {
        Ok(resp) if resp.ok => {
            state.metrics.record_login_success();
            if let Some(token) = &resp.token {
                merge_guest_session_if_any(&state, &headers, token).await;
                // 单会话“后登录优先”：普通用户本次登录已吊销全部旧 token，
                // 同步失效该用户的媒体鉴权缓存（管理员可多设备，不涉及踢旧）。
                if let Some(user) = state.services.auth.user_for_token(token).await {
                    if user.role < 3 {
                        auth_mw::invalidate_media_auth_user(&state, user.id).await;
                    }
                }
            }
        }
        // Wrong password, unknown user, unapproved or locked account: all count
        // as failed attempts, which is what the brute-force alert watches.
        Ok(_) => {
            state.metrics.record_login_failure();
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
        Err(_) => {
            state.metrics.record_login_failure();
        }
    }
    Ok(handle_auth_result(result, &state))
}

/// POST /auth/guest — 访客模式入口
///
/// 为没有会话的访问者创建匿名访客影子账号并签发 7 天 token（HttpOnly
/// cookie）。访客与登录用户同权限层（role=1），但只能看到/播放自己
/// 上传的内容；之后注册或登录真实账号时，访客期间的内容自动合并。
/// 幂等性由前端保证：先 `GET /auth/user`，仅在 401 时才调用本接口。
#[utoipa::path(
    post,
    path = "/auth/guest",
    tag = "auth",
    summary = "Enter guest mode (anonymous session)",
    description = "创建匿名访客影子账号并签发 7 天 token（同时以 bearer token 与 HttpOnly cookie 下发）。访客与登录用户同权限层，但只能看到/播放自己上传的内容；之后注册或登录真实账号时内容自动合并。是否调用由前端保证幂等（先 GET /auth/user，仅 401 时调用）。按 IP 限速。",
    responses(
        (status = 200, description = "Guest session created", body = AuthResponse),
        (status = 429, description = "Too many guest sessions from this IP — rate limited", body = AuthResponse)
    )
)]
pub async fn guest_session(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let ip = client_ip(&req);
    Ok(handle_auth_result(
        state.services.auth.create_guest_session(&ip).await,
        &state,
    ))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    summary = "Logout and invalidate token",
    description = "Invalidate the current auth token and clear the session cookie",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "Logged out successfully", body = AuthResponse))
)]
pub async fn logout(State(state): State<Arc<AppState>>, req: Request) -> impl IntoResponse {
    let token = auth_mw::extract_bearer_token(req.headers())
        .or_else(|| auth_mw::extract_token_from_cookie(req.headers()));
    let ip = client_ip(&req);

    state
        .services
        .auth
        .logout(None, token.as_deref(), &ip)
        .await;

    // 失效该 token 的媒体鉴权缓存（本地精确清除 + Redis DEL），
    // 否则已登出的设备在 10 秒 TTL 内仍能用旧 token 读媒体文件。
    if let Some(t) = token.as_deref() {
        auth_mw::invalidate_media_auth_token(&state, t).await;
    }

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

#[utoipa::path(
    get,
    path = "/auth/user",
    tag = "auth",
    summary = "Get current user info",
    description = "Returns basic information about the authenticated user",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "User details", body = UserInfoResponse))
)]
/// GET /auth/user
pub async fn user_info(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserInfoResponse> {
    match state
        .services
        .auth
        .user_info(&auth_user.username, auth_user.is_admin)
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
            avatar_url: None,
            is_guest: false,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/auth/user/profile",
    tag = "auth",
    summary = "Get user profile with stats",
    description = "Returns user profile including watch history, total watch time, and videos watched count",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "Profile with watch history", body = UserProfileResponse))
)]
/// GET /auth/user/profile
pub async fn user_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserProfileResponse> {
    match state
        .services
        .auth
        .user_profile(&auth_user.username, auth_user.is_admin)
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

#[utoipa::path(
    post,
    path = "/auth/user/avatar",
    tag = "auth",
    summary = "Upload avatar image",
    description = "通过 multipart 表单上传头像（JPG/PNG/WebP/GIF，最大 5MB），按 magic bytes 校验文件类型",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Avatar uploaded", body = serde_json::Value),
        (status = 400, description = "不支持的图片格式或文件过大")
    )
)]
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

#[utoipa::path(
    post,
    path = "/auth/forgot-password",
    tag = "auth",
    summary = "Request a password reset email",
    description = "发送密码重置邮件。无论邮箱是否注册都返回相同响应，避免邮箱枚举。IP 与邮箱均有速率限制。",
    request_body = ForgotPasswordRequest,
    responses((status = 200, description = "Reset request accepted", body = ForgotPasswordResponse))
)]
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
    // 邮箱维度：3 次/小时，达到上限后仅 60 秒短冷却。旧策略（2 次/5 分钟
    // → 封锁 600 秒）让攻击者能用极低代价对受害者邮箱造成长时间拒绝服务。
    if state
        .rate_limiter
        .check_with(&email_key, 3, 3600, 60)
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
        let Some(user) = state.services.auth.user_by_email(&email).await else {
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

#[utoipa::path(
    post,
    path = "/auth/reset-password",
    tag = "auth",
    summary = "Reset password",
    description = "使用邮件中的令牌设置新密码（8-128 字符，需包含大小写字母、数字、特殊字符中至少三种），成功后吊销该用户所有令牌",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset", body = serde_json::Value),
        (status = 400, description = "重置链接无效或已过期")
    )
)]
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

    // 原子化：改密码 + 吊销全部 token 同事务提交；吊销失败即整体失败，
    // 不会留下“密码已改、旧 token 仍可用”的状态。
    let updated = state
        .repos
        .user
        .reset_password_and_revoke_tokens(user_id, &hash)
        .await
        .map_err(|e| internal_error_log("reset_password_and_revoke_tokens", &e))?;

    if !updated {
        // The user was deleted between token validation and the update.
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "重置链接无效或已过期",
        ));
    }

    // 同步失效该用户媒体鉴权缓存（Redis 可用时精确，否则 10s TTL 兜底）
    auth_mw::invalidate_media_auth_user(&state, user_id).await;

    state.metrics.record_password_reset();

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "密码已重置" }),
    ))
}

#[utoipa::path(
    get,
    path = "/auth/reset-password",
    tag = "auth",
    summary = "Password reset page (email link)",
    description = "处理邮件中的重置链接，携带 token 重定向到前端重置密码页面",
    params(("token" = String, Query, description = "密码重置令牌")),
    responses((status = 303, description = "重定向到前端重置密码页面"))
)]
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

#[utoipa::path(
    put,
    path = "/auth/user/email",
    tag = "auth",
    summary = "Update current user's email",
    description = "更新当前用户邮箱（自动转小写）。需当前密码重认证；成功后吊销全部旧 token 并重置邮箱验证状态。",
    security(("bearerAuth" = [])),
    request_body = UpdateEmailRequest,
    responses(
        (status = 200, description = "邮箱已更新", body = serde_json::Value),
        (status = 400, description = "邮箱格式无效"),
        (status = 401, description = "当前密码错误"),
        (status = 409, description = "该邮箱已被其他账号绑定")
    )
)]
/// PUT /auth/user/email
///
/// 安全模型（本次加固）：
/// 1. 改邮箱属于敏感操作，必须先用当前密码重认证（防会话内跨站请求/
///    物理接触者静默改绑邮箱后走密码找回接管账号）；
/// 2. 成功后吊销该用户全部旧 token（其他设备立即下线），并签发一个新
///    token 返回给当前设备（刚验证过密码），响应按 `auth_response` 的
///    写法下发 token/csrf cookie；
/// 3. `user_repo.update_email` 会把 `email_verified` 置回 false，防止未
///    验证的新邮箱被当成已验证。
pub async fn update_email(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Json(req): Json<UpdateEmailRequest>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let email = req.email.trim().to_lowercase();

    // Enhanced email validation
    if !is_valid_email(&email) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "请输入有效的邮箱地址",
        ));
    }

    // 重认证：密码错误/为空统一泛化 401，不泄露账号状态
    if req.password.is_empty()
        || !state
            .services
            .auth
            .verify_user_password(&auth_user.username, &req.password)
            .await
            .map_err(|e| internal_error_log("verify_user_password", &e))?
    {
        tracing::warn!(
            user_id = auth_user.id,
            "update_email rejected: password re-auth failed"
        );
        return Err(error_response(StatusCode::UNAUTHORIZED, "当前密码错误"));
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

    // 改绑成功：先吊销全部旧 token（其他设备全部下线），再为当前设备
    // 签发新 token。吊销失败则整体失败，避免留下可用旧会话。
    if let Err(e) = state
        .repos
        .user
        .revoke_tokens_by_user_id(auth_user.id)
        .await
    {
        tracing::error!("update_email: revoke_tokens_by_user_id failed: {}", e);
        return Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "服务器内部错误",
        ));
    }
    let new_token = state
        .repos
        .user
        .create_token(auth_user.id)
        .await
        .map_err(|e| internal_error_log("create_token after update_email", &e))?;

    // 媒体鉴权缓存同步失效该用户旧 token（Redis 可用时精确，否则 TTL 兜底）
    auth_mw::invalidate_media_auth_user(&state, auth_user.id).await;

    // 与 auth_response 相同的 cookie 处理：HttpOnly token + 非 HttpOnly csrf
    let mut resp = Json(serde_json::json!({
        "ok": true,
        "message": "邮箱已更新，其他设备已下线，请重新验证新邮箱",
        "token": new_token,
    }))
    .into_response();
    if let Ok(val) = HeaderValue::from_str(&auth_mw::set_token_cookie(
        &new_token,
        crate::services::auth_service::COOKIE_MAX_AGE,
        state.config.cookie_secure,
    )) {
        resp.headers_mut()
            .insert(axum::http::header::SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&auth_mw::set_csrf_cookie(
        crate::services::auth_service::COOKIE_MAX_AGE,
        state.config.cookie_secure,
    )) {
        resp.headers_mut()
            .append(axum::http::header::SET_COOKIE, val);
    }
    Ok(resp)
}

#[utoipa::path(
    post,
    path = "/auth/send-verification-email",
    tag = "auth",
    summary = "Resend email verification link",
    description = "重新发送邮箱验证邮件。每 5 分钟最多 2 次（用户级速率限制）；SMTP 未配置时返回 503",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Verification email result", body = SendVerificationEmailResponse),
        (status = 503, description = "邮件服务未配置")
    )
)]
pub async fn send_verification_email(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<SendVerificationEmailResponse>, (StatusCode, Json<ErrorResponse>)> {
    // SMTP 未配置：明确返回 503，绝不静默把 email_verified 置 true
    //（旧行为让用户误以为邮箱已验证，属于虚假验证）。
    if !state.services.email.is_configured() {
        return Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "邮件服务未配置",
        ));
    }

    let key = format!("verify_email:user:{}", auth_user.id);
    if state
        .rate_limiter
        .check_with(&key, 2, 300, 600)
        .await
        .is_err()
    {
        return Ok(Json(SendVerificationEmailResponse {
            ok: false,
            message: "请求过于频繁，请稍后再试。".into(),
        }));
    }

    let email = state
        .repos
        .user
        .get_email(auth_user.id)
        .await
        .ok()
        .flatten();

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

    Ok(Json(SendVerificationEmailResponse {
        ok: true,
        message: "验证邮件已发送。如果您的邮箱没有收到，请稍后再试。".into(),
    }))
}

#[utoipa::path(
    get,
    path = "/auth/verify-email",
    tag = "auth",
    summary = "Email verification page (email link)",
    description = "处理邮件中的验证链接，验证成功后直接返回成功/失败 HTML 页面",
    params(("token" = String, Query, description = "邮箱验证令牌")),
    responses((status = 200, description = "验证结果 HTML 页面", content_type = "text/html", body = String))
)]
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

        state
            .services
            .auth
            .mark_email_verified(user_id)
            .await
            .map_err(|e| {
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

#[utoipa::path(
    post,
    path = "/auth/verify-email",
    tag = "auth",
    summary = "Verify email with token",
    description = "使用令牌验证邮箱地址",
    request_body = VerifyEmailRequest,
    responses(
        (status = 200, description = "Email verified", body = serde_json::Value),
        (status = 400, description = "验证链接无效或已过期")
    )
)]
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
