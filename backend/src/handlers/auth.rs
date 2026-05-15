use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use std::sync::Arc;
use std::net::IpAddr;

use crate::middleware::auth::{AppState, AuthUser};
use crate::models::auth::*;
use crate::util::password;
use crate::util::response::{error_response, ErrorResponse};

/// Get rate limiter key from request headers or default
fn get_rate_limit_ip(headers: &HeaderMap) -> IpAddr {
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(ip_str) = forwarded.to_str() {
            if let Some(ip) = ip_str.split(',').next() {
                if let Ok(parsed) = ip.trim().parse::<IpAddr>() {
                    return parsed;
                }
            }
        }
    }
    IpAddr::from([0, 0, 0, 0])
}

/// POST /auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let ip = get_rate_limit_ip(&headers);
    if state.rate_limiter.check(ip).await.is_err() {
        return Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("too many attempts, try again later".into()),
        }));
    }

    if req.username.trim().is_empty() || req.password.trim().is_empty() {
        return Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("username and password are required".into()),
        }));
    }

    let count = state.user_repo.count_users().await.map_err(|e| {
        tracing::error!("count_users error: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;
    let is_admin = count == 0;

    if let Ok(Some(_)) = state.user_repo.find_by_username(&req.username).await {
        return Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("username already exists".into()),
        }));
    }

    let hash = password::hash(&req.password).map_err(|e| {
        tracing::error!("password hash error: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    let user_id = state.user_repo.create_user(&req.username.trim(), &hash, is_admin)
        .await
        .map_err(|e| {
            tracing::error!("create_user error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?;

    let token = state.user_repo.create_token(user_id)
        .await
        .map_err(|e| {
            tracing::error!("create_token error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?;

    state.rate_limiter.reset(ip).await;

    tracing::info!(
        username = %req.username.trim(),
        is_admin = %is_admin,
        "user registered"
    );

    Ok(Json(AuthResponse {
        ok: true,
        token: Some(token),
        error: None,
    }))
}

/// POST /auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let ip = get_rate_limit_ip(&headers);

    if state.rate_limiter.check(ip).await.is_err() {
        return Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("too many attempts, try again later".into()),
        }));
    }

    let user = state.user_repo.find_by_username(&req.username)
        .await
        .map_err(|e| {
            tracing::error!("find_by_username error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?;

    let user = match user {
        Some(u) => u,
        None => {
            return Ok(Json(AuthResponse {
                ok: false,
                token: None,
                error: Some("invalid username or password".into()),
            }));
        }
    };

    if !password::verify(&req.password, &user.password_hash).map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))? {
        return Ok(Json(AuthResponse {
            ok: false,
            token: None,
            error: Some("invalid username or password".into()),
        }));
    }

    let token = state.user_repo.create_token(user.id)
        .await
        .map_err(|e| {
            tracing::error!("create_token error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?;

    state.rate_limiter.reset(ip).await;

    tracing::info!(
        username = %req.username,
        "user logged in"
    );

    Ok(Json(AuthResponse {
        ok: true,
        token: Some(token),
        error: None,
    }))
}

/// POST /auth/access — validate access password (webapp access gate)
pub async fn access_check(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AccessRequest>,
) -> Json<AuthResponse> {
    let ip = get_rate_limit_ip(&headers);
    if state.rate_limiter.check(ip).await.is_err() {
        return Json(AuthResponse {
            ok: false, token: None, error: Some("too many attempts, try again later".into()),
        });
    }
    if req.password == state.config.access_password {
        state.rate_limiter.reset(ip).await;
        Json(AuthResponse { ok: true, token: None, error: None })
    } else {
        Json(AuthResponse { ok: false, token: None, error: Some("密码错误".into()) })
    }
}

/// POST /auth/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<AuthResponse> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    if let Some(t) = token {
        let _ = state.user_repo.delete_token(&t).await;
    }

    Json(AuthResponse {
        ok: true,
        token: None,
        error: None,
    })
}

/// GET /auth/user
pub async fn user_info(
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserInfoResponse> {
    Json(UserInfoResponse {
        username: auth_user.username,
        is_admin: auth_user.is_admin,
        created_at: String::new(),
    })
}

/// GET /auth/user/profile
pub async fn user_profile(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Json<UserProfileResponse> {
    let username = &auth_user.username;

    let (total_watched, total_time, recent) = state.video_service
        .get_user_profile_data(username)
        .await
        .unwrap_or((0, 0, vec![]));

    let user_record = state.user_repo.find_by_username(username)
        .await
        .ok()
        .flatten();

    let created_at = user_record
        .map(|u| u.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default();

    Json(UserProfileResponse {
        username: username.clone(),
        is_admin: auth_user.is_admin,
        created_at,
        total_videos_watched: total_watched,
        total_watch_time_ms: total_time,
        recent_history: recent,
    })
}
