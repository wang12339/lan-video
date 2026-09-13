use std::sync::Arc;

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode},
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
        // L1 修复：严格 double-submit——`X-CSRF-Token` 头必须存在且与
        // `csrf_token` cookie 一致。此前只验头存在不比对，攻击者若能在
        // 目标域种任意 cookie/值即可绕过。
        let header_val = req
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let cookie_val = extract_csrf_from_cookie(req.headers());
        let matched = matches!((header_val, cookie_val), (Some(h), Some(c)) if h == c);
        if !matched {
            return Err(Box::new(error_response_response(
                StatusCode::FORBIDDEN,
                "CSRF protection: token mismatch or missing",
            )));
        }
    }
    Ok(())
}

/// 老 cookie 会话自愈：只带 token cookie、缺 `csrf_token` cookie 的客户端
/// （早于 L1 修复登录、换设备恢复、浏览器清理等）会在任何写操作上被
/// csrf_guard 卡成 403 —— 前端静默吞掉后表现为“退出登录没反应/退了又自动
/// 登回来”。在 cookie 认证的 GET 上检测到 csrf 缺失时补发一个新 csrf
/// cookie，客户端下一轮 JS 即可读到并走正常的 double-submit。
pub fn ensure_csrf_cookie_from_parts(
    method: &axum::http::Method,
    headers: &HeaderMap,
    response: &mut axum::response::Response,
) {
    if method != axum::http::Method::GET {
        return; // 只在幂等读上补发，避免与写路径的 Set-Cookie 竞争
    }
    if extract_token_from_cookie(headers).is_none() {
        return; // 非 cookie 认证（游客/纯 Bearer）不涉及
    }
    if extract_csrf_from_cookie(headers).is_some() {
        return; // 已有，不覆盖（值必须与客户端回传头一致，轮换会打断在途请求）
    }
    if let Ok(val) = HeaderValue::from_str(&set_csrf_cookie(
        crate::services::auth_service::COOKIE_MAX_AGE,
        true,
    )) {
        response
            .headers_mut()
            .append(axum::http::header::SET_COOKIE, val);
    }
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
                // revoked=true 现在有两种来源：管理员强制下线，或该账号
                // 在其他设备重新登录（单会话“后登录优先”策略）。token 层
                // 无法区分，使用合并文案。
                return error_response_response(
                    StatusCode::UNAUTHORIZED,
                    "你的账号已在其他设备登录或被管理员强制下线",
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
        is_guest: user.is_guest,
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
pub(crate) fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
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

/// CSRF double-submit cookie（L1 修复配套）：非 HttpOnly，前端 JS 读取后
/// 以 `X-CSRF-Token` 头回传，`csrf_guard` 严格比对两者。
#[inline]
pub fn set_csrf_cookie(max_age_secs: i64, secure: bool) -> String {
    let secure_flag = if secure { "; Secure" } else { "" };
    let token = random_alnum_csrf();
    format!(
        "csrf_token={}; SameSite=Strict; Path=/; Max-Age={}{}",
        token, max_age_secs, secure_flag
    )
}

/// 供 csrf cookie 生成的随机串（32 字符字母数字）。
#[inline]
fn random_alnum_csrf() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..32)
        .map(|_| CHARS[rand::thread_rng().gen_range(0..CHARS.len())] as char)
        .collect()
}

/// 读取 double-submit 的 cookie 值。
#[inline]
pub fn extract_csrf_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("Cookie")?.to_str().ok()?;
    for pair in cookie.split(';') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()?.trim() == "csrf_token" {
            let value = parts.next()?.trim();
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
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
