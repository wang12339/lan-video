use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;

use crate::state::AppState;
use crate::util::response::ErrorResponse;

fn error_response(status: StatusCode, msg: &str) -> Response {
    (status, Json(ErrorResponse { error: msg.into() })).into_response()
}

/// Bearer token authentication middleware.
pub async fn bearer_auth(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器配置错误");
    };

    let token = extract_bearer_token(req.headers())
        .or_else(|| extract_token_from_cookie(req.headers()));
    let Some(token) = token else {
        return error_response(StatusCode::UNAUTHORIZED, "authentication failed");
    };

    let user = match state.user_repo.find_user_by_token(&token).await {
        Ok(Some(u)) => u,
        Ok(None) => return error_response(StatusCode::UNAUTHORIZED, "authentication failed"),
        Err(e) => {
            tracing::error!("DB error in auth: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };
    let mut req = req;
    req.extensions_mut().insert(AuthUser {
        id: user.id,
        username: user.username.clone(),
        is_admin: user.is_admin,
    });
    next.run(req).await
}

/// Admin authentication middleware — checks AuthUser.is_admin from bearer_auth.
pub async fn admin_auth(req: Request, next: Next) -> Response {
    let auth_user = req.extensions().get::<AuthUser>().cloned();
    match auth_user {
        Some(user) if user.is_admin => next.run(req).await,
        Some(_) => error_response(StatusCode::FORBIDDEN, "需要管理员权限"),
        None => error_response(StatusCode::UNAUTHORIZED, "需要登录"),
    }
}

/// Media file authentication middleware.
/// Supports Authorization header and Cookie-based auth.
/// Browser `<video>` tags automatically send cookies with range requests.
pub async fn media_auth(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器配置错误");
    };

    match extract_bearer_token(req.headers())
        .or_else(|| extract_token_from_cookie(req.headers()))
    {
        Some(token) => match state.user_repo.find_user_by_token(&token).await {
            Ok(Some(_)) => next.run(req).await,
            Ok(None) => error_response(StatusCode::UNAUTHORIZED, "需要登录"),
            Err(e) => {
                tracing::error!("DB error in media_auth: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        },
        None => error_response(StatusCode::UNAUTHORIZED, "需要登录"),
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    auth.strip_prefix("Bearer ")
        .map(|s| s.to_string())
}

/// Extract auth token from HttpOnly cookie (same-origin requests)
pub fn extract_token_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("Cookie")?.to_str().ok()?;
    for pair in cookie.split(';') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()?.trim() == "token" {
            return Some(parts.next()?.to_string());
        }
    }
    None
}

/// Build a Set-Cookie header value for the auth token
pub fn set_token_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "token={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        token, max_age_secs, secure_flag
    )
}

/// Build a Set-Cookie header value that clears the auth token
pub fn clear_token_cookie(secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "token=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        secure_flag
    )
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    #[allow(dead_code)]
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}
