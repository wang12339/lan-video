use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::sync::RwLock;
use std::time::Instant;

use crate::repositories::user_repo::{UserRepository, UserRecord};
use crate::config::AppConfig;
use crate::services::video_service::VideoService;
use crate::util::response::ErrorResponse;

use std::time::Duration;
use moka::sync::Cache as MokaCache;

pub type VideoListCache = MokaCache<String, String>;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: UserRepository,
    pub video_service: VideoService,
    pub config: AppConfig,
    pub rate_limiter: RateLimiter,
    pub video_cache: VideoListCache,
}

fn error_response(status: StatusCode, msg: &str) -> Response {
    (status, Json(ErrorResponse { error: msg.into() })).into_response()
}

/// Bearer token authentication middleware.
/// Also supports X-Password + X-Username as an alternative (webapp access gate).
pub async fn bearer_auth(
    req: Request,
    next: Next,
) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "server config not found");
    };

    // Try Bearer token first
    if let Some(token) = extract_bearer_token(req.headers()) {
        let user = match state.user_repo.find_user_by_token(&token).await {
            Ok(Some(u)) => u,
            Ok(None) => return error_response(StatusCode::UNAUTHORIZED, "invalid token"),
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
        return next.run(req).await;
    }

    // No Bearer token — try X-Password + X-Username (webapp access gate)
    let headers = req.headers();
    let x_pwd = headers
        .get("X-Password")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let x_user = headers
        .get("X-Username")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !x_pwd.is_empty() && !x_user.is_empty() && x_pwd == state.config.access_password {
        // Find or auto-create device user
        let user: Option<UserRecord> = match state.user_repo.find_by_username(x_user).await {
            Ok(Some(u)) => Some(u),
            Ok(None) => {
                // Auto-register device user with a dummy password hash
                let hash = crate::util::password::hash("auto_device_pwd").unwrap_or_default();
                match state.user_repo.create_user(x_user, &hash, false).await {
                    Ok(id) => state.user_repo.find_by_id(id).await.ok().flatten(),
                    Err(e) => {
                        tracing::error!("auto-register error: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!("DB error in auth: {}", e);
                None
            }
        };

        if let Some(u) = user {
            let mut req = req;
            req.extensions_mut().insert(AuthUser {
                id: u.id,
                username: u.username.clone(),
                is_admin: u.is_admin,
            });
            return next.run(req).await;
        }
    }

    error_response(StatusCode::UNAUTHORIZED, "missing or invalid Authorization header, or incorrect access password")
}

/// Admin token authentication middleware (X-Admin-Token header)
pub async fn admin_auth(
    req: Request,
    next: Next,
) -> Response {
    let state = req.extensions().get::<Arc<AppState>>().cloned();
    let Some(state) = state else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "server config not found");
    };

    if !state.config.enforce_admin_token {
        // If admin token enforcement is disabled, skip check
        return next.run(req).await;
    }

    let admin_token = req
        .headers()
        .get("X-Admin-Token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if admin_token != state.config.admin_token {
        return error_response(StatusCode::FORBIDDEN, "invalid admin token");
    }

    next.run(req).await
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("Authorization")?.to_str().ok()?;
    auth.strip_prefix("Bearer ")
        .map(|s| s.to_string())
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

// ── Rate Limiting ──

#[derive(Clone)]
pub struct RateLimiter {
    attempts: Arc<RwLock<HashMap<IpAddr, RateLimitState>>>,
}

struct RateLimitState {
    count: AtomicU32,
    blocked_until: Option<Instant>,
}

impl Clone for RateLimitState {
    fn clone(&self) -> Self {
        RateLimitState {
            count: AtomicU32::new(self.count.load(Ordering::SeqCst)),
            blocked_until: self.blocked_until,
        }
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check(&self, ip: IpAddr) -> Result<(), ()> {
        let mut map = self.attempts.write().await;
        let state = map.entry(ip).or_insert(RateLimitState {
            count: AtomicU32::new(0),
            blocked_until: None,
        });

        // Check if blocked
        if let Some(until) = state.blocked_until {
            if Instant::now() < until {
                return Err(());
            } else {
                state.blocked_until = None;
                state.count.store(0, Ordering::SeqCst);
            }
        }

        let count = state.count.fetch_add(1, Ordering::SeqCst);
        if count >= 10 {
            state.blocked_until = Some(Instant::now() + std::time::Duration::from_secs(60));
            return Err(());
        }

        Ok(())
    }

    pub async fn reset(&self, ip: IpAddr) {
        let mut map = self.attempts.write().await;
        if let Some(state) = map.get_mut(&ip) {
            state.count.store(0, Ordering::SeqCst);
            state.blocked_until = None;
        }
    }

    /// Remove entries that are no longer blocked and have zero count.
    pub async fn cleanup_expired(&self) {
        let mut map = self.attempts.write().await;
        map.retain(|_, state| {
            let now = Instant::now();
            if let Some(until) = state.blocked_until {
                if now >= until {
                    return false; // block expired, remove
                }
                return true; // still blocked
            }
            state.count.load(Ordering::SeqCst) > 0 // keep only if still tracking
        });
    }
}
