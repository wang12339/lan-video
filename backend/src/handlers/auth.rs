use axum::{
    extract::{ConnectInfo, Multipart, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::middleware::auth::{self as auth_mw, AuthUser};
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

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
    result: Result<AuthResponse, crate::services::auth_service::AuthError>,
    state: &AppState,
) -> axum::response::Response {
    match result {
        Ok(resp) => auth_response(resp, state).into_response(),
        Err(crate::services::auth_service::AuthError::RateLimited) => {
            tracing::warn!("Auth request rate limited");
            Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("尝试次数过多，请稍后再试".into()),
            })
            .into_response()
        }
        Err(crate::services::auth_service::AuthError::Internal(e)) => {
            tracing::error!("Auth internal error: {}", e);
            Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("服务器内部错误".into()),
            })
            .into_response()
        }
    }
}

/// POST /auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
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

    let svc = state.auth_service.clone();
    Ok(handle_auth_result(
        svc.register(&req, &client_ip).await,
        &state,
    ))
}

/// POST /auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
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
    let svc = state.auth_service.clone();
    Ok(handle_auth_result(
        svc.login(&req, &client_ip).await,
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
            .user_repo
            .find_user_by_token(t)
            .await
            .ok()
            .flatten()
            .map(|u| u.username),
        None => None,
    };

    let svc = state.auth_service.clone();
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

/// Resolve the real client IP from Cloudflare headers (when present) and fall
/// back to the direct socket peer. SECURITY: this is the IP we feed into
/// per-IP rate limiters, so trusting the wrong header would let attackers
/// evade limits. Cloudflare is the only proxy we trust; if you put the
/// service behind another proxy, set TRUSTED_PROXY=1 and review the
/// `TRUST_X_FORWARDED_FOR` flow.
fn client_ip_from_headers(headers: &axum::http::HeaderMap, peer: SocketAddr) -> String {
    if let Some(cf_ip) = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        return cf_ip.to_string();
    }
    peer.ip().to_string()
}

/// GET /auth/user
pub async fn user_info(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserInfoResponse> {
    let svc = state.auth_service.clone();
    match svc.user_info(&auth_user.username, auth_user.is_admin).await {
        Ok(resp) => Json(resp),
        Err(_) => Json(UserInfoResponse {
            id: auth_user.id,
            username: auth_user.username,
            is_admin: auth_user.is_admin,
            created_at: String::new(),
        }),
    }
}

/// GET /auth/user/profile
pub async fn user_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserProfileResponse> {
    let svc = state.auth_service.clone();
    match svc
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
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "上传数据无效".into(),
            }),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                tracing::warn!("Read multipart error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "读取文件失败".into(),
                    }),
                )
            })?;
            if data.len() > 5 * 1024 * 1024 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "头像文件不能超过 5MB".into(),
                    }),
                ));
            }
            // SECURITY (A08-02): don't trust the client's Content-Type. Use
            // magic-byte sniffing to identify the real image format. This
            // blocks uploads of HTML/SVG/JS renamed to .png/.jpg.
            let (ext, _mime) = match crate::services::media_service::infer_image(&data) {
                Some(t) => t,
                None => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "不支持的图片格式，请上传 JPG/PNG/WebP/GIF".into(),
                        }),
                    ))
                }
            };
            file_ext = Some(ext.to_string());
            file_data = Some(data.to_vec());
        }
    }

    let data = file_data.ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "请选择要上传的文件".into(),
        }),
    ))?;
    let ext = file_ext.unwrap_or_else(|| "jpg".into());

    // Save to media/avatars/
    let avatars_dir = state.config.media_root.join("avatars");
    tokio::fs::create_dir_all(&avatars_dir).await.map_err(|e| {
        tracing::error!("Failed to create avatars dir: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "创建目录失败".into(),
            }),
        )
    })?;

    let filename = format!("{}.{}", auth_user.id, ext);
    let path = avatars_dir.join(&filename);
    let path_clone = path.clone();

    tokio::task::spawn_blocking(move || std::fs::write(&path_clone, &data))
        .await
        .map_err(|e| {
            tracing::error!("Spawn blocking error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "保存文件失败".into(),
                }),
            )
        })?
        .map_err(|e| {
            tracing::error!("Write file error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "保存文件失败".into(),
                }),
            )
        })?;

    let avatar_url = format!("/media/avatars/{}", filename);

    // Update user avatar in database
    state
        .user_repo
        .update_avatar(auth_user.id, &avatar_url)
        .await
        .map_err(|e| {
            tracing::error!("Update avatar error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "更新头像失败".into(),
                }),
            )
        })?;

    // Clean up old avatars with different extensions
    let avatars_dir = state.config.media_root.join("avatars");
    for ext in &["jpg", "png", "webp", "gif"] {
        let old_path = avatars_dir.join(format!("{}.{}", auth_user.id, ext));
        if old_path != path && old_path.exists() {
            let _ = std::fs::remove_file(old_path);
        }
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "avatarUrl": avatar_url
    })))
}
