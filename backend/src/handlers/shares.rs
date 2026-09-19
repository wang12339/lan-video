use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::share::{CreateShareRequest, CreateShareResponse, ShareListItem};
use crate::services::share_service::is_valid_share_token;
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::hashid;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

/// Build the share URL from the configured PUBLIC_URL.
fn build_share_url(config: &crate::config::AppConfig, token: &str) -> String {
    format!(
        "{}/webapp/player#share={}",
        config.public_url.trim_end_matches('/'),
        token
    )
}

/// Longest a share_token cookie is kept alive (365 days), matching the max
/// lifetime allowed for a share link itself.
const SHARE_COOKIE_MAX_AGE_SECS: i64 = 31_536_000;

/// POST /videos/{id}/share
#[utoipa::path(
    post,
    path = "/videos/{id}/share",
    tag = "shares",
    summary = "Create a share link for a video",
    description = "为视频创建分享链接，返回一次性展示的分享令牌（token 仅创建时返回）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID (hashid or numeric)")),
    request_body = CreateShareRequest,
    responses(
        (status = 201, description = "Share link created", body = CreateShareResponse),
        (status = 400, description = "Invalid video ID or expiry"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not the video owner"),
        (status = 404, description = "Video not found")
    )
)]
pub async fn create_share_link(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
    SafeJson(req): SafeJson<CreateShareRequest>,
) -> Result<(StatusCode, Json<CreateShareResponse>), (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    if let Some(days) = req.expires_in_days {
        if !(1..=365).contains(&days) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "有效期需在 1-365 天之间",
            ));
        }
    }

    // SECURITY (H-02): a share link exposes the video to anonymous access,
    // so only the video's uploader (or an admin) may create one. The
    // ownership lookup doubles as the existence check: a missing video keeps
    // its 404 semantics, and nothing beyond the video's existence is
    // revealed to callers who are not allowed to share it.
    let ownership = state
        .repos
        .share
        .find_video_ownership(video_id)
        .await
        .map_err(|e| internal_error_log("create_share_link failed to look up video ownership", &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    if !auth_user.is_admin && ownership.uploader_id != Some(auth_user.id) {
        return Err(error_response(StatusCode::FORBIDDEN, "无权分享该视频"));
    }

    let (token, share) = state
        .services
        .share
        .create_share_link(video_id, auth_user.id, req.expires_in_days)
        .await
        .map_err(ServiceError::into_tuple)?;

    let share_url = build_share_url(&state.config, &token);

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
#[utoipa::path(
    get,
    path = "/share/{token}",
    tag = "shares",
    summary = "Resolve a shared video",
    description = "Resolve a share token to the shared video. Public — no auth required, but rate-limited per IP to prevent token enumeration. On success a share_token HttpOnly cookie is set so media requests authenticate.",
    params(("token" = String, Path, description = "Un-guessable share token")),
    responses(
        (status = 200, description = "Shared video details", body = serde_json::Value),
        (status = 400, description = "Malformed share token"),
        (status = 404, description = "Share link invalid or expired"),
        (status = 429, description = "Rate limited")
    )
)]
pub async fn get_share_video(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_share_token(&token) {
        return Err(error_response(StatusCode::BAD_REQUEST, "分享链接格式无效"));
    }
    let share = state
        .services
        .share
        .get_share_video(&token)
        .await
        .map_err(|e| match e {
            ServiceError::NotFound(_) => {
                error_response(StatusCode::NOT_FOUND, "分享链接无效或已过期")
            }
            other => other.into_tuple(),
        })?;

    let video = state
        .repos
        .video
        .find_by_id(share.video_id)
        .await
        .map_err(|e| internal_error_log("get_share_video find_by_id failed", &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "视频不存在"))?;

    let body = Json(serde_json::json!({
        "id": hashid::encode_id(video.id),
        "title": video.title,
        "description": video.description,
        "category": video.category,
        "thumbUrl": video.thumb_url,
        "sourceType": video.source_type,
        "streamUrl": video.stream_url,
        "share": {
            "id": hashid::encode_id(share.id),
            "expiresAt": share.expires_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        }
    }));

    let mut resp = body.into_response();
    // Persist the token as an HttpOnly SameSite=Strict cookie so the browser's
    // <video> element can authenticate media range requests without putting the
    // token in the URL. Max-Age tracks the share's remaining lifetime (capped
    // at one year) so playback keeps working for long-lived shares.
    let max_age = match share.expires_at {
        Some(exp) => {
            let remaining = (exp - chrono::Utc::now().naive_utc()).num_seconds();
            remaining.clamp(0, SHARE_COOKIE_MAX_AGE_SECS)
        }
        None => SHARE_COOKIE_MAX_AGE_SECS,
    };
    let cookie = format!(
        "share_token={}; Path=/; Max-Age={}; HttpOnly; SameSite=Strict{}",
        token,
        max_age,
        if state.config.cookie_secure {
            "; Secure"
        } else {
            ""
        }
    );
    if let Ok(val) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(SET_COOKIE, val);
    }
    Ok(resp)
}

/// GET /auth/user/shares
#[utoipa::path(
    get,
    path = "/auth/user/shares",
    tag = "shares",
    summary = "List current user's share links",
    description = "返回当前用户创建的全部分享链接及其有效状态",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Share link list", body = [ShareListItem]),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_my_shares(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Json<Vec<ShareListItem>>, (StatusCode, Json<ErrorResponse>)> {
    let shares = state
        .services
        .share
        .list_my_shares(auth_user.id)
        .await
        .map_err(ServiceError::into_tuple)?;
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
#[utoipa::path(
    delete,
    path = "/videos/{id}/share/{share_id}",
    tag = "shares",
    summary = "Delete a share link",
    description = "删除指定分享链接（创建者本人或管理员）",
    security(("bearerAuth" = [])),
    params(
        ("id" = String, Path, description = "Video ID (hashid or numeric)"),
        ("share_id" = String, Path, description = "分享链接 ID (hashid 或数字)")
    ),
    responses(
        (status = 200, description = "Share link deleted", body = serde_json::Value),
        (status = 400, description = "Invalid video or share ID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Share link not found")
    )
)]
pub async fn delete_share_link(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((video_id, share_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let share_id = hashid::decode_id_or_numeric(&share_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的分享链接ID"))?;
    state
        .services
        .share
        .delete_share_link(video_id, share_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| match e {
            ServiceError::Forbidden(_) => error_response(StatusCode::FORBIDDEN, "无权删除"),
            other => other.into_tuple(),
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
#[utoipa::path(
    delete,
    path = "/auth/user/shares/{share_id}",
    tag = "shares",
    summary = "Revoke one of current user's share links",
    description = "撤销当前用户创建的指定分享链接",
    security(("bearerAuth" = [])),
    params(("share_id" = String, Path, description = "分享链接 ID (hashid 或数字)")),
    responses(
        (status = 200, description = "Share link revoked", body = serde_json::Value),
        (status = 400, description = "Invalid share ID"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Share link not found")
    )
)]
pub async fn revoke_my_share(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(share_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let share_id = hashid::decode_id_or_numeric(&share_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的分享链接ID"))?;
    state
        .services
        .share
        .revoke_my_share(share_id, auth_user.id)
        .await
        .map_err(ServiceError::into_tuple)?;
    tracing::info!(
        actor = %auth_user.username,
        share_id = share_id,
        "share link revoked by owner"
    );
    Ok(Json(serde_json::json!({ "ok": true })))
}
