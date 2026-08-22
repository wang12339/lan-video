use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::response::ErrorResponse;

#[derive(Debug, Deserialize)]
pub struct ToggleRequest {
    pub status: String,
}

/// 获取所有租户列表
pub async fn list_tenants(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let tenants = state
        .services
        .tenant
        .list_tenants()
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "tenants": tenants })))
}

/// 获取单个租户详情
pub async fn get_tenant(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let tenant = state
        .services
        .tenant
        .get_tenant(tenant_id)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!(tenant)))
}

/// 更新租户配置
pub async fn update_tenant(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<i64>,
    Json(settings): Json<crate::repositories::tenant_repo::TenantSettings>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    tracing::info!("update_tenant received settings: {:?}", settings);
    state
        .services
        .tenant
        .update_settings(tenant_id, settings)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true, "message": "租户配置已更新" })))
}

/// 获取租户使用统计
pub async fn get_tenant_stats(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let stats = state
        .services
        .tenant
        .get_stats(tenant_id)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!(stats)))
}

/// 禁用/启用租户
pub async fn toggle_tenant(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<i64>,
    Json(body): Json<ToggleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let active = body.status == "active";
    state
        .services
        .tenant
        .set_status(tenant_id, active)
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true })))
}

/// Convert a `ServiceError` into a tuple response.
fn map_admin_err(e: ServiceError) -> (StatusCode, Json<ErrorResponse>) {
    e.into_tuple()
}
