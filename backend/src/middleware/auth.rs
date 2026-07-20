use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::state::AppState;
use crate::util::response::error_response;

fn error_response_response(status: StatusCode, msg: &str) -> Response {
    error_response(status, msg).into_response()
}

/// CSRF protection: when using cookie auth for state-changing requests,
/// require a custom header (not sent by browsers automatically on cross-origin requests).
fn csrf_guard(req: &Request) -> Result<(), Box<Response>> {
    let method = req.method();
    let is_mutation = method == axum::http::Method::POST
        || method == axum::http::Method::PUT
        || method == axum::http::Method::DELETE;

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
        return error_response_response(StatusCode::UNAUTHORIZED, "authentication failed");
    };

    if let Err(resp) = csrf_guard(&req) {
        return *resp;
    }

    // First try normal lookup (excludes revoked tokens)
    let user = match state.user_repo.find_user_by_token(&token).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Check if the token exists but was revoked (admin kicked)
            match state.user_repo.find_token_detail(&token).await {
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

    if !user.approved {
        return error_response_response(StatusCode::FORBIDDEN, "账号待管理员审批");
    }

    let mut req = req;
    req.extensions_mut().insert(AuthUser {
        id: user.id,
        username: user.username.clone(),
        is_admin: user.role >= 3,
        role: user.role,
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

/// Media file authentication middleware.
/// Supports Authorization header and Cookie-based auth.
/// Browser `<video>` tags automatically send cookies with range requests.
///
/// SECURITY model:
/// - The video_id is the source of truth, derived from the URL path
///   (`/media/videos/{id}/...`). Custom headers cannot be sent by the HTML
///   `<video>` element, so we bind playback sessions to the path instead.
/// - For authenticated requests, an active playback session for the
///   path-derived video_id is required (started via
///   POST /playback/session/start, refreshed via heartbeat).
/// - For shared videos, the share token MUST have been created for the
///   path-derived video_id. A share token is no longer a global pass.
/// - Non-video assets (thumbnails, covers, avatars, scan previews) are
///   served if any of the above conditions hold for any of the user's
///   active sessions, or for any valid share token, since they are
///   public-facing media by design.
pub async fn media_auth(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器配置错误");
    };

    let path = req.uri().path();
    let path_video_id = extract_video_id_from_path(path);

    // Try bearer/cookie token auth first
    if let Some(token) =
        extract_bearer_token(req.headers()).or_else(|| extract_token_from_cookie(req.headers()))
    {
        match state.user_repo.find_user_by_token(&token).await {
            Ok(Some(user)) => {
                // For non-video assets (thumbnails, covers, avatars): allow if
                // the user is simply logged in. Gating thumbnails behind
                // a playback session would break the browse experience.
                let path_video_id = match path_video_id {
                    Some(id) => id,
                    None => {
                        return next.run(req).await;
                    }
                };

                let session_active = state
                    .playback_sessions
                    .is_active(&user.username, path_video_id);
                let db_verified = state
                    .playback_repo
                    .get_playback_data(&user.username, path_video_id)
                    .await
                    .ok()
                    .flatten()
                    .is_some();
                if session_active && !db_verified {
                    tracing::warn!(
                        username = %user.username,
                        video_id = %path_video_id,
                        "media_auth: in-memory session active but no DB record — possible misuse"
                    );
                }
                if !session_active {
                    return error_response_response(
                        StatusCode::FORBIDDEN,
                        "播放会话已过期，请重新播放",
                    );
                }

                // Optional secondary check: if the client also sent an
                // X-Video-ID header, it must agree with the path.
                if let Some(header_id) = req
                    .headers()
                    .get("X-Video-ID")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<i64>().ok())
                {
                    if header_id != path_video_id {
                        tracing::warn!(
                            username = %user.username,
                            path_video_id = %path_video_id,
                            header_video_id = %header_id,
                            "media_auth: X-Video-ID header does not match request path"
                        );
                        return error_response_response(StatusCode::FORBIDDEN, "video id mismatch");
                    }
                }

                return next.run(req).await;
            }
            Ok(None) => { /* fall through to share token check */ }
            Err(e) => {
                tracing::error!("DB error in media_auth: {}", e);
                return error_response_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error",
                );
            }
        }
    }

    // Fallback: check for share token (allows unauthenticated access to shared videos)
    if let Some(share_token) =
        extract_share_token(req.uri()).or_else(|| extract_share_token_from_cookie(req.headers()))
    {
        let token_hash = crate::repositories::share_repo::hash_share_token(&share_token);
        match state.share_repo.is_valid_token_hash(&token_hash).await {
            Ok(Some(share)) => {
                // SECURITY (C-01): Bind share tokens to the specific video they
                // were created for. Without this check, a single share token
                // acts as a global pass to /media/videos/*. For non-video
                // assets, allow access since they are public thumbnails.
                if let Some(req_id) = path_video_id {
                    if req_id != share.video_id {
                        tracing::warn!(
                            share_id = share.id,
                            share_video_id = share.video_id,
                            requested_video_id = req_id,
                            "media_auth: share token used for different video"
                        );
                        return error_response_response(
                            StatusCode::FORBIDDEN,
                            "share token does not authorize this video",
                        );
                    }
                }
                return next.run(req).await;
            }
            Ok(None) => {
                return error_response_response(StatusCode::UNAUTHORIZED, "分享链接无效或已过期");
            }
            Err(e) => {
                tracing::error!("DB error validating share token: {}", e);
                return error_response_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error",
                );
            }
        }
    }

    error_response_response(StatusCode::UNAUTHORIZED, "需要登录")
}

/// Extract a video id (i64) from a `/media/...` path if present.
/// Recognises the canonical layout `videos/{id}/...`.
/// Returns None for thumbnails/covers/avatars/etc. that are not video-scoped.
fn extract_video_id_from_path(path: &str) -> Option<i64> {
    let stripped = path.strip_prefix("/media/").unwrap_or(path);
    let mut parts = stripped.split('/');
    match (parts.next(), parts.next()) {
        (Some("videos"), Some(id_str)) => id_str.parse::<i64>().ok(),
        _ => None,
    }
}

fn extract_share_token(uri: &axum::http::Uri) -> Option<String> {
    let query = uri.query()?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == "share_token" {
            return Some(parts.next()?.to_string());
        }
    }
    None
}

/// SECURITY (H-08): also read the share token from a SameSite=Strict cookie
/// so that media requests don't need the token in the URL. The cookie is
/// set on the first GET /share/{token} call and used by the browser to
/// authenticate subsequent media range requests without leaking the token
/// into server access logs or browser history.
fn extract_share_token_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("Cookie")?.to_str().ok()?;
    for pair in cookie.split(';') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()?.trim() == "share_token" {
            return Some(parts.next()?.to_string());
        }
    }
    None
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(|s| s.to_string())
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
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub role: i16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    /// Build a router with just the CSRF guard middleware around a no-op handler.
    fn csrf_router() -> Router {
        async fn ok() -> &'static str {
            "ok"
        }
        Router::new()
            .route("/test", get(ok).post(ok).put(ok).delete(ok))
            .layer(from_fn(csrf_guard_as_middleware))
    }

    async fn csrf_guard_as_middleware(
        req: Request<Body>,
        next: axum::middleware::Next,
    ) -> Response {
        match csrf_guard(&req) {
            Ok(()) => next.run(req).await,
            Err(boxed) => *boxed,
        }
    }

    #[tokio::test]
    async fn csrf_get_passes_without_headers() {
        let res = csrf_router()
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_post_passes_with_bearer_token() {
        let res = csrf_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("authorization", "Bearer some-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_post_blocked_without_bearer_or_xrw() {
        let res = csrf_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn csrf_post_passes_with_x_requested_with() {
        let res = csrf_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_post_passes_with_csrf_token() {
        let res = csrf_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("x-csrf-token", "token-value")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[test]
    fn auth_user_struct_holds_admin_flag() {
        let user = AuthUser {
            id: 1,
            username: "alice".into(),
            is_admin: true,
            role: 3,
        };
        assert!(user.is_admin);
        assert_eq!(user.role, 3);
    }

    #[test]
    fn auth_user_default_role_is_viewer() {
        let user = AuthUser {
            id: 2,
            username: "bob".into(),
            is_admin: false,
            role: 1,
        };
        assert!(!user.is_admin);
        assert_eq!(user.role, 1);
    }
}
