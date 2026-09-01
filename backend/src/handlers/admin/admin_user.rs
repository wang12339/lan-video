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
use crate::util::response::{error_response, ErrorResponse, SafeJson};

use super::map_admin_err;

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

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<
    Json<Vec<crate::repositories::user_repo::UserWithStatus>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let users = state
        .services
        .admin
        .list_users(auth_user.tenant_id)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(users))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .delete_user(id, auth_user.id, auth_user.tenant_id)
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

pub async fn reset_user_password(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<AdminResetPasswordRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let pw_len = req.password.chars().count();
    if !(8..=128).contains(&pw_len) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "密码长度需在 8-128 个字符之间",
        ));
    }
    if !crate::services::auth_service::is_password_strong_enough(&req.password) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "密码过于简单，请使用包含大小写字母、数字、特殊字符中至少三种的密码",
        ));
    }
    let outcome = state
        .services
        .admin
        .reset_user_password(id, &req.password, auth_user.tenant_id)
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

pub async fn toggle_user_admin(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .toggle_user_admin(id, auth_user.id, auth_user.tenant_id)
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

pub async fn approve_user(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<ApproveRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let outcome = state
        .services
        .admin
        .approve_user(id, req.approved, auth_user.tenant_id)
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

pub async fn kick_user(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let count = state
        .services
        .admin
        .kick_user(id, auth_user.tenant_id)
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
