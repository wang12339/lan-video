use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::repositories::tenant_repo::TenantSettings;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

use super::map_admin_err;

#[derive(Debug, Deserialize)]
pub struct ToggleRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
    pub custom_domain: Option<String>,
    #[serde(default = "default_plan")]
    pub plan: String,
    #[serde(default = "default_max_users")]
    pub max_users: i32,
    #[serde(default = "default_max_storage")]
    pub max_storage_bytes: i64,
    #[serde(default)]
    pub settings: TenantSettings,
}

fn default_plan() -> String {
    "free".to_string()
}

fn default_max_users() -> i32 {
    10
}

fn default_max_storage() -> i64 {
    53687091200 // 50 GB
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
    if tenant_id <= 0 {
        return Err(error_response(StatusCode::BAD_REQUEST, "无效的租户ID"));
    }
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

/// 创建新租户
pub async fn create_tenant(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTenantRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "租户名称长度需在 1-100 个字符之间",
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
            "租户标识需为 1-50 个字符，仅允许字母、数字和连字符",
        ));
    }
    if req.max_users <= 0 || req.max_users > 100_000 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最大用户数需在 1-100000 之间",
        ));
    }
    if req.max_storage_bytes <= 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "最大存储空间必须大于 0",
        ));
    }
    let tenant = state
        .services
        .tenant
        .create_tenant(
            &req.name,
            &req.slug,
            req.custom_domain.as_deref(),
            &req.plan,
            req.max_users,
            req.max_storage_bytes,
            req.settings,
        )
        .await
        .map_err(map_admin_err)?;
    Ok(Json(json!({ "ok": true, "tenant": tenant })))
}
