use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::services::share_service::{is_valid_share_token, ShareError};
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

#[derive(Deserialize)]
pub struct CreateShareRequest {
    pub expires_in_days: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShareResponse {
    pub id: i64,
    pub video_id: i64,
    /// Raw share token — shown ONCE on creation. Never returned by any other endpoint.
    pub token: String,
    pub share_url: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareListItem {
    pub id: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub active: bool,
}

/// Build the share URL.
/// SECURITY (A08-01): prefer the configured `PUBLIC_URL` over request headers.
fn build_share_url(
    headers: &HeaderMap,
    config: &crate::config::AppConfig,
    token: &str,
) -> String {
    let base = if !config.public_url.is_empty() {
        config.public_url.clone()
    } else {
        let scheme = headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| s == "http" || s == "https")
            .unwrap_or_else(|| if config.cookie_secure { "https" } else { "http" }.into());
        let host = headers
            .get("Host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost:8082");
        format!("{}://{}", scheme, host)
    };
    format!("{}/player#share={}", base.trim_end_matches('/'), token)
}

/// POST /videos/{id}/share
pub async fn create_share_link(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<i64>,
    headers: HeaderMap,
    Json(req): Json<CreateShareRequest>,
) -> Result<(StatusCode, Json<CreateShareResponse>), (StatusCode, Json<ErrorResponse>)> {
    let _video = state
        .video_repo
        .find_by_id(video_id)
        .await
        .map_err(|e| {
            tracing::error!("create_share_link failed to find video: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    let (token, share) = state
        .share_service
        .create_share_link(video_id, auth_user.id, req.expires_in_days)
        .await
        .map_err(|e| match e {
            ShareError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            other => other.into_response(),
        })?;

    let share_url = build_share_url(&headers, &state.config, &token);

    tracing::info!(
        actor = %auth_user.username,
        share_id = share.id,
        video_id = share.video_id,
        expires_at = ?share.expires_at,
        "share link created"
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateShareResponse {
            id: share.id,
            video_id: share.video_id,
            token,
            share_url,
            expires_at: share
                .expires_at
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
            created_at: share.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }),
    ))
}

/// GET /share/{token}
pub async fn get_share_video(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_share_token(&token) {
        return Err(error_response(StatusCode::BAD_REQUEST, "分享链接格式无效"));
    }
    let share = state
        .share_service
        .get_share_video(&token)
        .await
        .map_err(|e| match e {
            ShareError::Invalid(msg) => error_response(StatusCode::BAD_REQUEST, msg),
            ShareError::NotFound => error_response(StatusCode::NOT_FOUND, "分享链接无效或已过期"),
            ShareError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            _ => error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"),
        })?;

    let video = state
        .video_repo
        .find_by_id(share.video_id)
        .await
        .map_err(|e| {
            tracing::error!("get_share_video find_by_id failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    let body = Json(serde_json::json!({
        "id": video.id,
        "title": video.title,
        "description": video.description,
        "category": video.category,
        "thumbUrl": video.thumb_url,
        "sourceType": video.source_type,
        "streamUrl": video.stream_url,
        "share": {
            "id": share.id,
            "expiresAt": share.expires_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        }
    }));

    let mut resp = body.into_response();
    use axum::http::header::{HeaderName, HeaderValue};
    let cookie_name = HeaderName::from_static("set-cookie");
    let cookie = format!(
        "share_token={}; Path=/; Max-Age=2592000; HttpOnly; SameSite=Strict{}",
        token,
        if state.config.cookie_secure { "; Secure" } else { "" }
    );
    if let Ok(val) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(cookie_name, val);
    }
    Ok(resp)
}

/// GET /auth/user/shares
pub async fn list_my_shares(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<ShareListItem>>, (StatusCode, Json<ErrorResponse>)> {
    let shares = state
        .share_service
        .list_my_shares(auth_user.id)
        .await
        .map_err(|e| match e {
            ShareError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            other => other.into_response(),
        })?;
    let now = chrono::Utc::now().naive_utc();
    let items: Vec<ShareListItem> = shares
        .into_iter()
        .map(|s| {
            let active = match s.expires_at {
                Some(exp) => exp > now,
                None => true,
            };
            ShareListItem {
                id: s.id,
                expires_at: s
                    .expires_at
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                created_at: s.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                active,
            }
        })
        .collect();
    Ok(Json(items))
}

/// DELETE /videos/{id}/share/{share_id}
pub async fn delete_share_link(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((video_id, share_id)): Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .share_service
        .delete_share_link(video_id, share_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| match e {
            ShareError::NotFound => error_response(StatusCode::NOT_FOUND, "分享链接不存在"),
            ShareError::Forbidden => error_response(StatusCode::FORBIDDEN, "无权删除"),
            ShareError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            _ => error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"),
        })?;

    tracing::info!(
        actor = %auth_user.username,
        share_id = share_id,
        video_id = video_id,
        "share link revoked"
    );

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /auth/user/shares/{share_id}
pub async fn revoke_my_share(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(share_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .share_service
        .revoke_my_share(share_id, auth_user.id)
        .await
        .map_err(|e| match e {
            ShareError::NotFound => error_response(StatusCode::NOT_FOUND, "分享链接不存在"),
            ShareError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            _ => error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"),
        })?;
    tracing::info!(
        actor = %auth_user.username,
        share_id = share_id,
        "share link revoked by owner"
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}
