use std::sync::Arc;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::middleware::tenant::TenantContext;
use crate::state::AppState;
use crate::util::response::error_response;

pub(super) fn error_response_response(status: StatusCode, msg: &str) -> Response {
    let mut response = error_response(status, msg).into_response();
    // SECURITY: prevent CDN from caching error responses
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
}

/// CSRF protection: when using cookie auth for state-changing requests,
/// require a custom header (not sent by browsers automatically on cross-origin requests).
pub(super) fn csrf_guard(req: &Request) -> Result<(), Box<Response>> {
    let method = req.method();
    let is_mutation = method == axum::http::Method::POST
        || method == axum::http::Method::PUT
        || method == axum::http::Method::DELETE
        || method == axum::http::Method::PATCH;

    // Only enforce for mutation requests authenticated via cookie
    if is_mutation && extract_bearer_token(req.headers()).is_none() {
        // Standard CSRF defense: reject requests without a custom header.
        // Browsers will not set X-Requested-With cross-origin without a CORS
        // preflight, and the CORS layer does NOT include x-requested-with in
        // Access-Control-Allow-Headers, so this is safe.
        if req
            .headers()
            .get("x-requested-with")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "XMLHttpRequest")
            .unwrap_or(false)
        {
            return Ok(());
        }
        // x-csrf-token is a fallback for more granular control.
        if req
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .is_none()
        {
            return Err(Box::new(error_response_response(
                StatusCode::FORBIDDEN,
                "CSRF protection: missing required header",
            )));
        }
    }
    Ok(())
}

/// Bearer token authentication middleware.
pub async fn bearer_auth(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器配置错误");
    };

    let token =
        extract_bearer_token(req.headers()).or_else(|| extract_token_from_cookie(req.headers()));
    let Some(token) = token else {
        return error_response_response(StatusCode::UNAUTHORIZED, "未登录");
    };

    // SECURITY: reject malformed tokens before any DB work. Tokens are
    // 256-bit alphanumeric strings; anything else is garbage from an
    // attacker and would otherwise trigger a hashed lookup per request
    // (cheap DoS amplification: 2 queries per unique bogus token).
    if !is_valid_auth_token(&token) {
        return error_response_response(StatusCode::UNAUTHORIZED, "authentication failed");
    }

    if let Err(resp) = csrf_guard(&req) {
        return *resp;
    }

    // First try normal lookup (excludes revoked tokens)
    let user = match state.repos.user.find_user_by_token(&token).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Check if the token exists but was revoked (admin kicked)
            match state.repos.user.find_token_detail(&token).await {
                Ok(Some((_u, true, _valid))) => {
                    return error_response_response(
                        StatusCode::UNAUTHORIZED,
                        "你的账号已被管理员强制下线",
                    );
                }
                Ok(Some((_u, false, false))) => {
                    return error_response_response(
                        StatusCode::UNAUTHORIZED,
                        "登录已过期，请重新登录",
                    );
                }
                _ => {
                    return error_response_response(
                        StatusCode::UNAUTHORIZED,
                        "authentication failed",
                    );
                }
            }
        }
        Err(e) => {
            tracing::error!("DB error in auth: {}", e);
            return error_response_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };

    // SECURITY (H-01): an auth token is bound to the tenant that issued it
    // (auth_tokens.tenant_id). A token minted on tenant A must never
    // authenticate against tenant B's domain — reject with 403 instead of
    // leaking the cross-tenant user identity.
    let tenant = req.extensions().get::<TenantContext>().cloned();
    let tenant_id = match &tenant {
        Some(t) => t.tenant_id,
        None => {
            // Every request through the router has been through
            // resolve_tenant; a missing context means a wiring error.
            tracing::warn!("bearer_auth: TenantContext missing from request extensions");
            1
        }
    };

    if user.tenant_id != tenant_id {
        tracing::warn!(
            username = %user.username,
            token_tenant_id = user.tenant_id,
            request_tenant_id = tenant_id,
            "bearer_auth: token tenant mismatch, rejecting"
        );
        return error_response_response(StatusCode::FORBIDDEN, "无效的登录凭证");
    }

    if !user.approved {
        return error_response_response(StatusCode::FORBIDDEN, "账号待管理员审批");
    }

    let mut req = req;
    req.extensions_mut().insert(AuthUser {
        id: user.id,
        username: user.username.clone(),
        is_admin: user.role >= 3,
        role: user.role,
        tenant_id,
    });
    next.run(req).await
}

/// Admin authentication middleware — checks AuthUser.is_admin from bearer_auth.
pub async fn admin_auth(req: Request, next: Next) -> Response {
    let auth_user = req.extensions().get::<AuthUser>().cloned();
    match auth_user {
        Some(user) if user.is_admin => next.run(req).await,
        Some(_) => error_response_response(StatusCode::FORBIDDEN, "需要管理员权限"),
        None => error_response_response(StatusCode::UNAUTHORIZED, "需要登录"),
    }
}

/// Role-based authentication middleware — checks minimum role level.
/// Role levels: 0=readonly, 1=viewer, 2=editor, 3=admin
pub async fn role_auth(req: Request, next: Next, min_role: i16) -> Response {
    let auth_user = req.extensions().get::<AuthUser>().cloned();
    match auth_user {
        Some(user) if user.role >= min_role => next.run(req).await,
        Some(_) => error_response_response(StatusCode::FORBIDDEN, "权限不足"),
        None => error_response_response(StatusCode::UNAUTHORIZED, "需要登录"),
    }
}

mod media;

pub use media::{media_auth, AuthUser};

/// Parse an `Authorization: Bearer <token>` header. The scheme is matched
/// case-insensitively (RFC 7235 §2.1: auth schemes are case-insensitive) and
/// the token is trimmed of stray whitespace.
fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    let (scheme, rest) = auth.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// Extract auth token from HttpOnly cookie (same-origin requests)
pub fn extract_token_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("Cookie")?.to_str().ok()?;
    for pair in cookie.split(';') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()?.trim() == "token" {
            let value = parts.next()?.trim();
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

/// Auth tokens are 256-bit alphanumeric strings (64 chars). Enforcing the
/// exact format before any DB/cache work blocks DoS-style spam of garbage
/// Authorization/Cookie values: each unique bogus token would otherwise
/// trigger a SHA-256 hash and a database lookup.
fn is_valid_auth_token(token: &str) -> bool {
    token.len() == 64
        && token.bytes().all(|b| b.is_ascii_alphanumeric())
        && token.bytes().any(|b| b.is_ascii_alphabetic())
        && token.bytes().any(|b| b.is_ascii_digit())
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
