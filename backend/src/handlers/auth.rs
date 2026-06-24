use axum::{
    extract::State,
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::sync::Arc;

use crate::state::AppState;
use crate::models::auth::{AuthRequest, AuthResponse, UserInfoResponse, UserProfileResponse};
use crate::services::auth_service::AuthService;
use crate::middleware::auth::{self as auth_mw, AuthUser};
use crate::util::response::{ErrorResponse, SafeJson};

/// Construct an AuthService from the AppState (avoids storing it redundantly)
fn auth_service(state: &AppState) -> AuthService {
    AuthService::new(
        state.user_repo.clone(),
        state.video_service.clone(),
        state.rate_limiter.clone(),
        state.config.clone(),
    )
}

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

/// Handle an auth result (register or login), mapping errors to rate-limit response
fn handle_auth_result(result: Result<AuthResponse, crate::services::auth_service::AuthError>, state: &AppState) -> axum::response::Response {
    match result {
        Ok(resp) => auth_response(resp, state).into_response(),
        Err(_) => Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("尝试次数过多，请稍后再试".into()),
        }).into_response(),
    }
}

/// POST /auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<AuthRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let svc = auth_service(&state);
    Ok(handle_auth_result(svc.register(&req).await, &state))
}

/// POST /auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<AuthRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let svc = auth_service(&state);
    Ok(handle_auth_result(svc.login(&req).await, &state))
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
