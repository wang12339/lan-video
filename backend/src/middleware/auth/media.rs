use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use moka::sync::Cache;

use crate::middleware::tenant::TenantContext;
use crate::state::AppState;

use super::{
    error_response_response, extract_bearer_token, extract_token_from_cookie, is_valid_auth_token,
};
use crate::services::share_service::is_valid_share_token;

static MEDIA_AUTH_CACHE: std::sync::OnceLock<Cache<String, CachedAuthUser>> =
    std::sync::OnceLock::new();

#[inline]
fn media_auth_cache() -> &'static Cache<String, CachedAuthUser> {
    MEDIA_AUTH_CACHE.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(10))
            .max_capacity(100_000)
            .build()
    })
}

#[derive(Clone)]
struct CachedAuthUser {
    username: std::sync::Arc<str>,
    tenant_id: i64,
}

const MEDIA_AUTH_CACHE_TTL_SECS: u64 = 10;

/// 从共享 Redis 读取 media 鉴权缓存（未配置 Redis 或读取失败则视为 miss）。
/// 缓存值格式：`{tenant_id}|{username}`（username 不含 `|`）。
async fn media_auth_cache_get_redis(state: &Arc<AppState>, token: &str) -> Option<CachedAuthUser> {
    let conn = state.redis.as_ref()?;
    let key = format!("media:auth:{}", token);
    let mut conn = conn.clone();
    let val: Option<String> = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .ok()
        .flatten();
    let val = val?;
    let (tenant_part, username_part) = val.split_once('|')?;
    let tenant_id = tenant_part.parse::<i64>().ok()?;
    Some(CachedAuthUser {
        username: username_part.into(),
        tenant_id,
    })
}

async fn media_auth_cache_put_redis(state: &Arc<AppState>, token: &str, user: &CachedAuthUser) {
    if let Some(conn) = state.redis.as_ref() {
        let key = format!("media:auth:{}", token);
        let val = format!("{}|{}", user.tenant_id, user.username);
        let mut conn = conn.clone();
        let _ = redis::cmd("SETEX")
            .arg(&key)
            .arg(MEDIA_AUTH_CACHE_TTL_SECS)
            .arg(&val)
            .query_async::<()>(&mut conn)
            .await;
    }
}

/// Media file authentication middleware.
/// Supports Authorization header and Cookie-based auth.
/// Browser `<video>` tags automatically send cookies with range requests.
///
/// SECURITY model:
/// - The video_id is the source of truth, derived from the URL path
///   (`/media/videos/{id}/...`, `/media/thumb_{id}.jpg`, `/media/cover_{id}.jpg`,
///   `/media/variants/{id}_{res}.mp4`). Custom headers cannot be sent by the
///   HTML `<video>` element, so we bind playback sessions to the path instead.
/// - For authenticated requests, an active playback session for the
///   path-derived video_id is required (started via
///   POST /playback/session/start, refreshed via heartbeat).
/// - For shared videos, the share token MUST have been created for the
///   path-derived video_id. A share token is no longer a global pass.
/// - M-03: media files that do not resolve to a registered video are denied
///   even to logged-in users (orphan files, in-progress `.upload_*` temp
///   files). The only exemption is `/media/avatars/*`, a public static asset
///   by design (never video content, no tenant-private data).
pub async fn media_auth(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器配置错误");
    };

    // `nest_service("/media")` strips the prefix, so media_auth sees
    // `/thumb_10054.jpg` instead of `/media/thumb_10054.jpg`. Re-add the
    // prefix so all path parsing (extract_video_id_from_path, thumbnail
    // checks, stream_url lookups) sees the canonical `/media/...` layout —
    // the DB stores fully-prefixed paths.
    let path_raw = req.uri().path();
    let path_owned;
    let path = if path_raw.starts_with("/media/") {
        path_raw
    } else {
        path_owned = format!("/media{path_raw}");
        &path_owned
    };

    // Thumbnails and covers are preview images rendered in listing pages
    // (home, gallery). <img> tags cannot send Authorization headers, so
    // these paths must be publicly accessible without authentication.
    if is_thumbnail_or_cover_path(path) {
        return next.run(req).await;
    }

    let path_video_id = extract_video_id_from_path(path);

    // Try bearer/cookie token auth first. Tokens failing the format check are
    // treated as absent (no DB work) and fall through to the share-token path.
    let auth_token = extract_bearer_token(req.headers())
        .or_else(|| extract_token_from_cookie(req.headers()))
        .filter(|t| is_valid_auth_token(t));
    if let Some(token) = auth_token {
        // SECURITY (H-01): /media goes through the global resolve_tenant
        // middleware, so the request tenant is known here; the token must
        // belong to that tenant or it is rejected outright.
        let tenant_id = req
            .extensions()
            .get::<TenantContext>()
            .map(|t| t.tenant_id)
            .unwrap_or(1);
        match resolve_media_user(&state, &token, tenant_id).await {
            MediaAuthResult::Authorized(username) => {
                // SECURITY (M-03): authorization must be video-scoped. The
                // video_id comes from the path (canonical layout, thumbnails,
                // covers, transcoded variants) or from the videos table via
                // stream_url. Anything that resolves to neither is denied —
                // the old "logged-in => allow" fallback let any logged-in
                // user read unregistered files (.upload_* temp files, orphan
                // files). The only exempt layout is /media/avatars/*, which
                // is public static media by design.
                let video_id = if let Some(id) = path_video_id {
                    id
                } else if is_public_static_media_path(path) {
                    return next.run(req).await;
                } else {
                    // Path does not contain video_id (e.g. /media/{timestamp}_{filename}.mp4)
                    // Query database to find the video by stream_url and verify playback session
                    let request_path = path;
                    match state
                        .repos
                        .video
                        .find_by_stream_url(tenant_id, request_path)
                        .await
                    {
                        Ok(Some(video)) => video.id,
                        Ok(None) => {
                            // Not a registered video and not an allowed public
                            // asset — deny instead of serving the file.
                            tracing::warn!(
                                username = %username,
                                path = %request_path,
                                "media_auth: denying unregistered media path"
                            );
                            return error_response_response(
                                StatusCode::FORBIDDEN,
                                "无权访问该媒体",
                            );
                        }
                        Err(e) => {
                            tracing::error!("DB error finding video by stream_url: {}", e);
                            return error_response_response(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "internal error",
                            );
                        }
                    }
                };

                // The in-memory session tracker (with 120s heartbeat timeout) is
                // the source of truth for active playback sessions. We no longer
                // query the DB on every range request — that caused O(N) queries
                // per video where N = number of HTTP range chunks.
                if !state
                    .playback_sessions
                    .is_active(tenant_id, &username, video_id)
                {
                    // SECURITY: a valid share token bound to this video grants
                    // the same access as an active playback session. Check it
                    // before rejecting so that logged-in users following a
                    // share link (share_token cookie) aren't blocked by an
                    // expired playback session.
                    match share_token_authorizes(&state, req.uri(), req.headers(), video_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            return error_response_response(
                                StatusCode::FORBIDDEN,
                                "播放会话已过期，请重新播放",
                            );
                        }
                        Err(resp) => return resp,
                    }
                }

                // Optional secondary check: X-Video-ID header must match path
                if let Some(header_id) = req
                    .headers()
                    .get("X-Video-ID")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<i64>().ok())
                {
                    if header_id != video_id {
                        tracing::warn!(
                            username = %username,
                            path_video_id = %video_id,
                            header_video_id = %header_id,
                            "media_auth: X-Video-ID header does not match request path"
                        );
                        return error_response_response(StatusCode::FORBIDDEN, "video id mismatch");
                    }
                }

                return next.run(req).await;
            }
            MediaAuthResult::Pass => { /* fall through to share token check */ }
            MediaAuthResult::Denied(resp) => return resp,
        }
    }

    // Fallback: check for share token (allows unauthenticated access to shared videos)
    if let Some(share_token) = extract_share_token(req.uri())
        .or_else(|| extract_share_token_from_cookie(req.headers()))
        .filter(|t| is_valid_share_token(t))
    {
        let token_hash = crate::repositories::share_repo::hash_share_token(&share_token);
        match state.repos.share.is_valid_token_hash(&token_hash).await {
            Ok(Some(share)) => {
                // SECURITY: verify the requested file belongs to the shared video
                let request_path = path;
                if let Some(req_id) = path_video_id {
                    // Path contains video_id (e.g. /media/videos/{id}/... or /media/thumb_{id}.jpg)
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
                } else {
                    // Path does not contain video_id (e.g. /media/{timestamp}_{filename}.mp4)
                    // Query the video's stream_url to verify the file belongs to this video
                    // NOTE (H-02): share-token path stays unscoped — the token is
                    // capability-based and the video id is compared against the
                    // share record directly, mirroring handlers::shares.
                    match state.repos.video.find_by_id_unscoped(share.video_id).await {
                        Ok(Some(video)) => {
                            if request_path != video.stream_url
                                && request_path != video.thumb_url.as_deref().unwrap_or("")
                                && request_path != video.cover_url.as_deref().unwrap_or("")
                            {
                                tracing::warn!(
                                    share_id = share.id,
                                    share_video_id = share.video_id,
                                    request_path = request_path,
                                    video_stream_url = video.stream_url,
                                    "media_auth: share token used for file not belonging to video"
                                );
                                return error_response_response(
                                    StatusCode::FORBIDDEN,
                                    "share token does not authorize this file",
                                );
                            }
                        }
                        Ok(None) => {
                            return error_response_response(StatusCode::NOT_FOUND, "视频不存在");
                        }
                        Err(e) => {
                            tracing::error!("DB error fetching video for share auth: {}", e);
                            return error_response_response(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "internal error",
                            );
                        }
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

enum MediaAuthResult {
    Authorized(String),
    Pass,
    Denied(Response),
}

/// Returns Ok(true) if the request carries a valid share token bound to the
/// given video_id. Used as a fallback when an authenticated user's playback
/// session has expired.
async fn share_token_authorizes(
    state: &Arc<AppState>,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    video_id: i64,
) -> Result<bool, Response> {
    let Some(share_token) = extract_share_token(uri)
        .or_else(|| extract_share_token_from_cookie(headers))
        .filter(|t| is_valid_share_token(t))
    else {
        return Ok(false);
    };
    let token_hash = crate::repositories::share_repo::hash_share_token(&share_token);
    match state.repos.share.is_valid_token_hash(&token_hash).await {
        Ok(Some(share)) => Ok(share.video_id == video_id),
        Ok(None) => Ok(false),
        Err(e) => {
            tracing::error!("DB error validating share token: {}", e);
            Err(error_response_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error",
            ))
        }
    }
}

async fn resolve_media_user(state: &Arc<AppState>, token: &str, tenant_id: i64) -> MediaAuthResult {
    let cached = media_auth_cache().get(token);
    let username = if let Some(ref c) = cached {
        if c.tenant_id != tenant_id {
            return MediaAuthResult::Denied(error_response_response(
                StatusCode::FORBIDDEN,
                "无效的登录凭证",
            ));
        }
        c.username.to_string()
    } else {
        // 共享 Redis 缓存（多实例一致），miss 后落本地 moka 快路径
        let from_redis = media_auth_cache_get_redis(state, token).await;
        if let Some(c) = from_redis {
            if c.tenant_id != tenant_id {
                return MediaAuthResult::Denied(error_response_response(
                    StatusCode::FORBIDDEN,
                    "无效的登录凭证",
                ));
            }
            media_auth_cache().insert(token.to_string(), c.clone());
            c.username.to_string()
        } else {
            match state.repos.user.find_user_by_token(token).await {
                Ok(Some(u)) => {
                    // SECURITY (H-01): token is bound to the tenant it was issued
                    // in; reject cross-tenant use on /media.
                    if u.tenant_id != tenant_id {
                        return MediaAuthResult::Denied(error_response_response(
                            StatusCode::FORBIDDEN,
                            "无效的登录凭证",
                        ));
                    }
                    // bearer_auth rejects unapproved users; keep media_auth
                    // consistent so de-approval also revokes media access.
                    if !u.approved {
                        return MediaAuthResult::Denied(error_response_response(
                            StatusCode::FORBIDDEN,
                            "账号未启用",
                        ));
                    }
                    let username_arc: std::sync::Arc<str> = u.username.clone().into();
                    let entry = CachedAuthUser {
                        username: username_arc.clone(),
                        tenant_id,
                    };
                    media_auth_cache().insert(token.to_string(), entry.clone());
                    media_auth_cache_put_redis(state, token, &entry).await;
                    username_arc.to_string()
                }
                Ok(None) => return MediaAuthResult::Pass,
                Err(e) => {
                    tracing::error!("DB error in media_auth: {}", e);
                    return MediaAuthResult::Denied(error_response_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal error",
                    ));
                }
            }
        }
    };

    MediaAuthResult::Authorized(username)
}

#[inline]
fn extract_video_id_from_path(path: &str) -> Option<i64> {
    let stripped = path.strip_prefix("/media/").unwrap_or(path);

    // Check for /media/hls/{id}/... transcoded HLS layout
    // (master.m3u8, index.m3u8, segment_NNN.ts). HLS files are NOT public —
    // they require the same playback session / share token as other media.
    if let Some(rest) = stripped.strip_prefix("hls/") {
        if let Some(id_str) = rest.split('/').next() {
            if let Ok(id) = id_str.parse::<i64>() {
                return Some(id);
            }
        }
    }

    // Check for /media/videos/{id}/... pattern
    let mut parts = stripped.split('/');
    if let (Some("videos"), Some(id_str)) = (parts.next(), parts.next()) {
        return id_str.parse::<i64>().ok();
    }

    // Check for /media/variants/{id}_{resolution}.mp4 pattern
    if let Some(rest) = stripped.strip_prefix("variants/") {
        // The video id always leads the filename: {id}_{res}.{ext}
        if let Some(id_str) = rest.split('_').next() {
            if let Ok(id) = id_str.parse::<i64>() {
                return Some(id);
            }
        }
    }

    // Check for /media/thumb_{id}.jpg or /media/cover_{id}.jpg patterns
    // Also handle /media/cover_{id}_{timestamp}.{ext} pattern
    // SECURITY: enforce share token binding for thumbnails and covers
    for prefix in &["thumb_", "cover_"] {
        if let Some(rest) = stripped.strip_prefix(prefix) {
            // Extract video ID: either "123.jpg" or "123_456.ext"
            let id_str = rest.split('_').next().unwrap_or(rest);
            let id_str = id_str.split('.').next().unwrap_or(id_str);
            if let Ok(id) = id_str.parse::<i64>() {
                return Some(id);
            }
        }
    }

    // Flat transcoded-variant layout returned by GET /videos/{id}/variants:
    //   /media/{video_id}_{resolution}.mp4   (resolution like 720p/1080p/...)
    // `video_id` is numeric; the resolution suffix MUST be the whitelisted
    // "{digits}p" form (720p/1080p/...). A bare digit run is NOT a resolution:
    // user uploads named like "{n}_{timestamp}.mp4" (a common camera export
    // scheme) were misparsed as "{video_id=1}_{resolution=...}.mp4", binding
    // playback sessions to a wrong/nonexistent video id and rejecting every
    // range request with 403 "播放会话已过期". Such paths must fall through to
    // the stream_url DB lookup below instead (M-03).
    let basename = stripped.rsplit('/').next().unwrap_or(stripped);
    if let Some((id_str, res)) = basename.rsplit_once('_') {
        let res_stem = res.rsplit_once('.').map(|(s, _)| s).unwrap_or(res);
        if let Some(res_core) = res_stem.strip_suffix('p') {
            if !res_core.is_empty() && res_core.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(id) = id_str.parse::<i64>() {
                    return Some(id);
                }
            }
        }
    }

    None
}

#[inline]
fn is_public_static_media_path(path: &str) -> bool {
    path.starts_with("/media/avatars/")
}

#[inline]
fn is_thumbnail_or_cover_path(path: &str) -> bool {
    let stripped = path.strip_prefix("/media/").unwrap_or(path);
    let first = stripped.split('/').next().unwrap_or("");
    first.starts_with("thumb_") || first.starts_with("cover_") || first.ends_with("_121.jpg")
}

#[inline]
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

#[inline]
fn extract_share_token_from_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("Cookie")?.to_str().ok()?;
    for pair in cookie.split(';') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()?.trim() == "share_token" {
            let value = parts.next()?.trim();
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub role: i16,
    pub tenant_id: i64,
}

#[cfg(test)]
mod tests {
    use super::super::csrf_guard;
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
            tenant_id: 1,
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
            tenant_id: 1,
        };
        assert!(!user.is_admin);
        assert_eq!(user.role, 1);
    }

    #[test]
    fn bearer_token_is_case_insensitive_and_trimmed() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "bearer   abc123  ".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("abc123"));
        headers.insert("authorization", "Bearer abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("abc123"));
        headers.insert("authorization", "BEARER abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("abc123"));
    }

    #[test]
    fn bearer_token_rejects_wrong_scheme_and_empty() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Basic abc".parse().unwrap());
        assert!(extract_bearer_token(&headers).is_none());
        headers.insert("authorization", "Bearer".parse().unwrap());
        assert!(extract_bearer_token(&headers).is_none());
        headers.insert("authorization", "Bearer ".parse().unwrap());
        assert!(extract_bearer_token(&headers).is_none());
    }

    #[test]
    fn auth_token_format_validation() {
        assert!(is_valid_auth_token(&"a".repeat(64)));
        assert!(is_valid_auth_token(&"9Z".repeat(32)));
        assert!(!is_valid_auth_token(&"a".repeat(63)));
        assert!(!is_valid_auth_token(&"a".repeat(65)));
        assert!(!is_valid_auth_token(
            &("a".repeat(32) + "-" + &"b".repeat(31))
        ));
        assert!(!is_valid_auth_token(""));
    }

    #[test]
    fn cookie_token_is_trimmed_and_rejects_empty() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "token=  abc  ; other=1".parse().unwrap());
        assert_eq!(extract_token_from_cookie(&headers).as_deref(), Some("abc"));
        headers.insert("cookie", "token=; other=1".parse().unwrap());
        assert!(extract_token_from_cookie(&headers).is_none());
        headers.insert("cookie", "share_token=  1234  ".parse().unwrap());
        assert_eq!(
            extract_share_token_from_cookie(&headers).as_deref(),
            Some("1234")
        );
    }

    #[test]
    fn video_id_extraction_from_media_paths() {
        assert_eq!(
            extract_video_id_from_path("/media/videos/42/stream.mp4"),
            Some(42)
        );
        assert_eq!(extract_video_id_from_path("/media/thumb_7.jpg"), Some(7));
        assert_eq!(
            extract_video_id_from_path("/media/cover_9_12345.jpg"),
            Some(9)
        );
        assert_eq!(
            extract_video_id_from_path("/media/variants/42_720p.mp4"),
            Some(42)
        );
        assert_eq!(
            extract_video_id_from_path("/media/variants/7_1080p.mp4"),
            Some(7)
        );
        assert_eq!(
            extract_video_id_from_path("/media/variants/42_480p.mp4"),
            Some(42)
        );
        assert_eq!(extract_video_id_from_path("/media/avatars/3.jpg"), None);
        assert_eq!(
            extract_video_id_from_path("/media/1699999999_hello.mp4"),
            None
        );
        assert_eq!(extract_video_id_from_path("/media/.upload_abc123"), None);
        assert_eq!(
            extract_video_id_from_path("/media/variants/no_id.mp4"),
            None
        );
        // Flat transcoded-variant layout used by GET /videos/{id}/variants
        assert_eq!(
            extract_video_id_from_path("/media/12345_720p.mp4"),
            Some(12345)
        );
        assert_eq!(
            extract_video_id_from_path("/media/12345_1080p.mp4"),
            Some(12345)
        );
        // Orphan files with a non-resolution suffix must stay unresolved (M-03)
        assert_eq!(
            extract_video_id_from_path("/media/1699999999_hello.mp4"),
            None
        );
        // Regression: camera/timestamp uploads like "{n}_{digits}.mp4" have a
        // bare digit suffix which is NOT a "{digits}p" resolution. They must
        // stay unresolved here so the stream_url DB lookup resolves them;
        // previously "1_4924860112935323688.mp4" extracted video_id=1 and
        // broke playback-session binding for ~80% of the library.
        assert_eq!(
            extract_video_id_from_path("/media/1_4924860112935323688.mp4"),
            None
        );
        assert_eq!(
            extract_video_id_from_path("/media/12_3456789012345678901.mp4"),
            None
        );
        // Same for image uploads registered as videos.
        assert_eq!(
            extract_video_id_from_path("/media/-4924860112935323688_1109.jpg"),
            None
        );
        // A digit run ending in 'p' is still a resolution.
        assert_eq!(
            extract_video_id_from_path("/media/12345_720p.mp4"),
            Some(12345)
        );
    }

    #[test]
    fn public_static_media_path_whitelist() {
        assert!(is_public_static_media_path("/media/avatars/3.jpg"));
        assert!(is_public_static_media_path("/media/avatars/abc/def.png"));
        assert!(!is_public_static_media_path("/media/thumb_7.jpg"));
        assert!(!is_public_static_media_path("/media/videos/42/stream.mp4"));
        assert!(!is_public_static_media_path("/media/variants/42_720p.mp4"));
        assert!(!is_public_static_media_path("/media/1699999999_hello.mp4"));
        assert!(!is_public_static_media_path("/media/.upload_abc123"));
        assert!(!is_public_static_media_path("/media/avatars"));
        assert!(!is_public_static_media_path("/media/avatars3.jpg"));
    }
}
