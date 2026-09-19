use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;

use crate::middleware::auth::{invalidate_media_auth_user, AuthUser};
use crate::models::admin::{AdminResetPasswordRequest, AdminUsersQuery, ApproveRequest};
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

#[utoipa::path(
    get,
    path = "/admin/users",
    tag = "admin",
    summary = "List users",
    description = "返回用户列表（含审批状态、角色、活跃令牌等），支持服务端搜索/筛选/分页",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("search" = Option<String>, Query, description = "用户名或邮箱模糊搜索"),
        ("status" = Option<String>, Query, description = "审批状态：active（已通过，默认）| pending | all"),
        ("role" = Option<String>, Query, description = "角色：all（默认）| admin | user"),
        ("page" = Option<i64>, Query, description = "页码（0 基，默认 0）"),
        ("size" = Option<i64>, Query, description = "每页条数（默认 20，最大 200）")
    ),
    responses(
        (status = 200, description = "User list", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AdminUsersQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let page = params.page.unwrap_or(0).clamp(0, 1_000_000);
    let size = params.size.unwrap_or(20).clamp(1, 200);
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    // 默认只看已通过用户（待审批在独立区块展示）；status=all 时不过滤
    let status = match params.status.as_deref() {
        Some("pending") => Some("pending"),
        Some("all") => None,
        _ => Some("active"),
    };
    let role = match params.role.as_deref() {
        Some("admin") => Some("admin"),
        Some("user") => Some("user"),
        _ => None,
    };

    let (items, total) = state
        .services
        .admin
        .list_users_paged(search, status, role, page, size)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "size": size,
    })))
}

/// GET /admin/users/pending/count — 待审批注册用户数。
///
/// 管理端导航徽标轮询用：只返回计数，避免轮询时反复传输整个用户列表。
#[utoipa::path(
    get,
    path = "/admin/users/pending/count",
    tag = "admin",
    description = "返回待审批注册用户数（管理员导航徽标轮询用，仅计数）",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    responses(
        (status = 200, description = "Pending count", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn pending_user_count(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let count = state
        .services
        .admin
        .count_pending_users()
        .await
        .map_err(map_admin_err)?;
    Ok(Json(serde_json::json!({ "count": count })))
}

#[utoipa::path(
    delete,
    path = "/admin/users/{id}",
    tag = "admin",
    summary = "Delete a user",
    description = "删除用户及其关联数据（不能删除自己）",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID")
    ),
    responses(
        (status = 200, description = "Delete result", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found")
    )
)]
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
        // 目标用户已删除：同步失效其媒体鉴权缓存（Redis 可用时精确，
        // 否则本地 10s TTL 兜底）
        invalidate_media_auth_user(&state, id).await;
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Err(outcome_error(outcome.error_msg))
    }
}

#[utoipa::path(
    put,
    path = "/admin/users/{id}/password",
    tag = "admin",
    summary = "Reset a user's password",
    description = "管理员重置指定用户密码，同时吊销该用户全部令牌",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID")
    ),
    request_body = AdminResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found")
    )
)]
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
        .reset_user_password(id, &req.password)
        .await
        .map_err(map_admin_err)?;
    if outcome.ok {
        tracing::warn!(
            actor = %auth_user.username,
            target_user_id = id,
            "admin reset user password (all tokens invalidated)"
        );
        // 密码已重置且旧 token 全部吊销：同步失效媒体鉴权缓存
        invalidate_media_auth_user(&state, id).await;
        Ok(Json(OkResponse {
            ok: true,
            error: None,
            deleted: None,
        }))
    } else {
        Err(outcome_error(outcome.error_msg))
    }
}

#[utoipa::path(
    put,
    path = "/admin/users/{id}/admin",
    tag = "admin",
    summary = "Toggle admin privileges",
    description = "切换用户的管理员权限（不能操作自己）",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID")
    ),
    responses(
        (status = 200, description = "Admin status toggled", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found")
    )
)]
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

#[utoipa::path(
    put,
    path = "/admin/users/{id}/approve",
    tag = "admin",
    summary = "Approve a pending user",
    description = "审批新用户（注册审批制下用户需审批后才能登录）",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID")
    ),
    request_body = ApproveRequest,
    responses(
        (status = 200, description = "Approval updated", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found")
    )
)]
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

#[utoipa::path(
    post,
    path = "/admin/users/{id}/kick",
    tag = "admin",
    summary = "Force logout a user",
    description = "强制用户下线：删除该用户全部认证令牌",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID")
    ),
    responses(
        (status = 200, description = "User kicked", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found")
    )
)]
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
    // 踢人 = 吊销全部会话：同步失效媒体鉴权缓存，否则旧 token 在
    // 缓存 TTL 内仍可读取媒体文件
    invalidate_media_auth_user(&state, id).await;
    Ok(Json(OkResponse {
        ok: true,
        error: None,
        deleted: Some(count),
    }))
}
