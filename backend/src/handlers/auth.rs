use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::sync::Arc;

use crate::state::AppState;
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::services::auth_service::AuthService;
use crate::middleware::auth::{self as auth_mw, AuthUser};
use crate::util::response::{ErrorResponse, SafeJson};

/// Extract IP from headers for rate limiting.
/// When behind ddnsto tunnel, all requests appear to come from the proxy.
/// We don't trust X-Forwarded-For since ddnsto passes it through without adding the real IP.
/// Using a single global bucket is better than letting attackers spoof unlimited IPs.
fn get_rate_limit_ip(_headers: &HeaderMap) -> std::net::IpAddr {
    std::net::IpAddr::from([0, 0, 0, 0])
}

/// Construct an AuthService from the AppState (avoids storing it redundantly)
fn auth_service(state: &AppState) -> AuthService {
    AuthService::new(
        state.user_repo.clone(),
        state.video_service.clone(),
        state.rate_limiter.clone(),
        state.config.clone(),
    )
}

/// POST /auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    SafeJson(req): SafeJson<AuthRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let ip = get_rate_limit_ip(&headers);
    let svc = auth_service(&state);

    match svc.register(&req, ip).await {
        Ok(resp) => {
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
                Ok(http_resp)
            } else {
                Ok(Json(resp).into_response())
            }
        }
        Err(_) => Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("尝试次数过多，请稍后再试".into()),
        })
        .into_response()),
    }
}

/// POST /auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    SafeJson(req): SafeJson<AuthRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let ip = get_rate_limit_ip(&headers);
    let svc = auth_service(&state);

    match svc.login(&req, ip).await {
        Ok(resp) => {
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
                Ok(http_resp)
            } else {
                Ok(Json(resp).into_response())
            }
        }
        Err(_) => Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("尝试次数过多，请稍后再试".into()),
        })
        .into_response()),
    }
}

/// POST /auth/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| auth_mw::extract_token_from_cookie(&headers));

    let svc = auth_service(&state);
    svc.logout(token.as_deref()).await;

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

/// GET /auth/user
pub async fn user_info(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserInfoResponse> {
    let svc = auth_service(&state);
    match svc.user_info(&auth_user.username, auth_user.is_admin).await {
        Ok(resp) => Json(resp),
        Err(_) => Json(UserInfoResponse {
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
    let svc = auth_service(&state);
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
