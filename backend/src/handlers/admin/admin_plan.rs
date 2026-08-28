use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::repositories::plan_repo::{CreatePlanRequest, UpdatePlanRequest};
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

use super::map_admin_err;

#[derive(Debug, Deserialize)]
pub struct TogglePlanRequest {
    pub active: bool,
}

/// 获取所有套餐列表
pub async fn list_plans(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let plans = state
        .services
        .plan
        .list_active_plans()
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "plans": plans })))
}

/// 获取所有套餐列表（包括禁用的）
pub async fn list_all_plans(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let plans = state
        .services
        .plan
        .list_all_plans()
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "plans": plans })))
}

/// 获取单个套餐详情
pub async fn get_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let plan = state
        .services
        .plan
        .get_plan(plan_id)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!(plan)))
}

/// 创建套餐
pub async fn create_plan(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePlanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "套餐名称长度需在 1-100 个字符之间",
        ));
    }
    if req.slug.trim().is_empty()
        || req.slug.len() > 50
        || !req
            .slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "套餐标识需为 1-50 个字符，仅允许字母、数字和连字符",
        ));
    }
    if req.max_users <= 0 || req.max_storage_bytes <= 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最大用户数和存储空间必须大于 0",
        ));
    }
    let plan = state
        .services
        .plan
        .create_plan(req)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true, "plan": plan })))
}

/// 更新套餐
pub async fn update_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<i64>,
    Json(req): Json<UpdatePlanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if plan_id <= 0 {
        return Err(error_response(StatusCode::BAD_REQUEST, "无效的套餐ID"));
    }
    if let Some(ref name) = req.name {
        if name.trim().is_empty() || name.len() > 100 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "套餐名称长度需在 1-100 个字符之间",
            ));
        }
    }
    if let Some(max_users) = req.max_users {
        if max_users <= 0 || max_users > 100_000 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "最大用户数需在 1-100000 之间",
            ));
        }
    }
    if let Some(max_storage) = req.max_storage_bytes {
        if max_storage <= 0 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "最大存储空间必须大于 0",
            ));
        }
    }
    let plan = state
        .services
        .plan
        .update_plan(plan_id, req)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true, "plan": plan })))
}

/// 启用/禁用套餐
pub async fn toggle_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<i64>,
    Json(body): Json<TogglePlanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .services
        .plan
        .set_status(plan_id, body.active)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true })))
}

/// 删除套餐
pub async fn delete_plan(
    State(state): State<Arc<AppState>>,
    Path(plan_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .services
        .plan
        .delete_plan(plan_id)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true })))
}
