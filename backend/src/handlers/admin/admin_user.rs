use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::admin::{AdminResetPasswordRequest, ApproveRequest};
use crate::models::video::OkResponse;
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

/// Convert a `ServiceError` into a tuple response.
fn map_admin_err(e: ServiceError) -> (StatusCode, Json<ErrorResponse>) {
    e.into_tuple()
}

/// Convert an `ActionOutcome` with `ok: false` into the appropriate HTTP error.
/// Maps known error messages to proper status codes:
/// - "用户不存在" → 404
/// - "无权限"/"权限不足" → 403
/// - Anything else → 500
fn outcome_error(msg: Option<String>) -> (StatusCode, Json<ErrorResponse>) {
    let msg = msg.unwrap_or_else(|| "操作失败".into());
    if msg.contains("不存在") {
        error_response(StatusCode::NOT_FOUND, msg)
    } else if msg.contains("权限") || msg.contains("无权") {
        error_response(StatusCode::FORBIDDEN, msg)
    } else {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }
}

/// GET /admin/users
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(_auth_user): Extension<AuthUser>,
) -> Result<
    Json<Vec<crate::repositories::user_repo::UserWithStatus>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let users = state
        .services
        .admin
        .list_users(_auth_user.tenant_id)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(users))
}

/// DELETE /admin/users/{id}
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .delete_user(id, auth_user.id)
        .await
        .map_err(map_admin_err)?;
    if outcome.ok {
        tracing::warn!(
            actor = %auth_user.username,
            target_user_id = id,
            "admin deleted user"
        );
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Err(outcome_error(outcome.error_msg))
    }
}

/// PUT /admin/users/{id}/password — 重置密码

pub async fn reset_user_password(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<AdminResetPasswordRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .reset_user_password(id, &req.password)
        .await
        .map_err(map_admin_err)?;
    if outcome.ok {
        tracing::warn!(
            actor = %auth_user.username,
            target_user_id = id,
            "admin reset user password (all tokens invalidated)"
        );
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Err(outcome_error(outcome.error_msg))
    }
}

/// PUT /admin/users/{id}/admin — 切换管理员权限
pub async fn toggle_user_admin(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .toggle_user_admin(id, auth_user.id)
        .await
        .map_err(map_admin_err)?;
    if outcome.ok {
        tracing::warn!(
            actor = %auth_user.username,
            target_user_id = id,
            new_role = ?outcome.new_role,
            "admin toggled user admin status"
        );
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Err(outcome_error(outcome.error_msg))
    }
}

/// PUT /admin/users/{id}/approve — 审批用户

pub async fn approve_user(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<ApproveRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .approve_user(id, req.approved)
        .await
        .map_err(map_admin_err)?;
    if outcome.ok {
        tracing::info!(
            actor = %auth_user.username,
            target_user_id = id,
            approved = req.approved,
            "admin set user approval"
        );
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Err(outcome_error(outcome.error_msg))
    }
}

/// POST /admin/users/{id}/kick — 强制用户下线（删除所有 token）
pub async fn kick_user(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let count = state
        .services
        .admin
        .kick_user(id)
        .await
        .map_err(map_admin_err)?;
    tracing::warn!(
        actor = %auth_user.username,
        target_user_id = id,
        tokens_deleted = count,
        "admin kicked user offline"
    );
    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: Some(count),
    }))
}
