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

#[inline]
pub(super) fn error_response_response(status: StatusCode, msg: &str) -> Response {
    let mut response = error_response(status, msg).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
}

#[inline]
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

    if !is_valid_auth_token(&token) {
        return error_response_response(StatusCode::UNAUTHORIZED, "authentication failed");
    }

    if let Err(resp) = csrf_guard(&req) {
        return *resp;
    }

    let user = match state.repos.user.find_user_by_token(&token).await {
        Ok(Some(u)) => u,
        Ok(None) => match state.repos.user.find_token_detail(&token).await {
            Ok(Some((_, true, _))) => {
                return error_response_response(
                    StatusCode::UNAUTHORIZED,
                    "你的账号已被管理员强制下线",
                );
            }
            Ok(Some((_, false, false))) => {
                return error_response_response(StatusCode::UNAUTHORIZED, "登录已过期，请重新登录");
            }
            _ => {
                return error_response_response(StatusCode::UNAUTHORIZED, "authentication failed");
            }
        },
        Err(e) => {
            tracing::error!("DB error in auth: {}", e);
            return error_response_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };

    let tenant = req.extensions().get::<TenantContext>().cloned();
    let tenant_id = match &tenant {
        Some(t) => t.tenant_id,
        None => {
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
        username: user.username,
        is_admin: user.role >= 3,
        role: user.role,
        tenant_id,
    });
    next.run(req).await
}

/// Admin authentication middleware — checks AuthUser.is_admin from bearer_auth.
/// 判断请求 IP 是否命中管理员白名单（空列表 = 未启用，恒放行）。
fn ip_in_admin_whitelist(ip: &str, whitelist: &[std::net::IpAddr]) -> bool {
    if whitelist.is_empty() {
        return true;
    }
    match ip.parse::<std::net::IpAddr>() {
        Ok(addr) => whitelist.contains(&addr),
        Err(_) => false,
    }
}

pub async fn admin_auth(req: Request, next: Next) -> Response {
    let auth_user = req.extensions().get::<AuthUser>().cloned();
    let Some(user) = auth_user else {
        return error_response_response(StatusCode::UNAUTHORIZED, "需要登录");
    };
    if !user.is_admin {
        return error_response_response(StatusCode::FORBIDDEN, "需要管理员权限");
    }

    // ADMIN_IP_WHITELIST (opt-in): 配置后仅白名单来源可访问管理接口
    if let Some(state) = req.extensions().get::<Arc<AppState>>() {
        if !ip_in_admin_whitelist(
            &crate::util::net::client_ip(&req),
            &state.config.admin_ip_whitelist,
        ) {
            tracing::warn!(
                ip = %crate::util::net::client_ip(&req),
                "admin_auth: IP not in admin whitelist, rejecting"
            );
            return error_response_response(StatusCode::FORBIDDEN, "请求来源不在管理员白名单");
        }
    }

    next.run(req).await
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

#[inline]
fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    let (scheme, rest) = auth.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim();
    (!token.is_empty()).then(|| token.to_string())
}

#[inline]
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

#[inline]
fn is_valid_auth_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[inline]
pub fn set_token_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "token={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        token, max_age_secs, secure_flag
    )
}

#[inline]
pub fn clear_token_cookie(secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    format!(
        "token=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        secure_flag
    )
}

#[cfg(test)]
mod tests {
    use super::ip_in_admin_whitelist;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn whitelist_disabled_passes_anything() {
        assert!(ip_in_admin_whitelist("192.168.1.1", &[]));
        assert!(ip_in_admin_whitelist("not-an-ip", &[]));
    }

    #[test]
    fn whitelist_rejects_outside_and_accepts_member() {
        let wl = [IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))];
        assert!(ip_in_admin_whitelist("10.0.0.5", &wl));
        assert!(!ip_in_admin_whitelist("10.0.0.6", &wl));
        assert!(!ip_in_admin_whitelist("unparseable", &wl));
    }
}
