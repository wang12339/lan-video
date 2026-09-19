use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::tag::{
    CreateTagRequest, TagListResponse, TagQuery, TagResponse, UpdateTagRequest,
};
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::hashid;
use crate::util::response::{error_response, internal_error_log, ErrorResponse, SafeJson};

const MAX_TAG_NAME_LEN: usize = 50;

fn is_valid_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_tag_name(name: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if name.trim().is_empty() || name.len() > MAX_TAG_NAME_LEN {
        Err(error_response(
            StatusCode::BAD_REQUEST,
            "标签名称长度需在 1-50 个字符之间",
        ))
    } else {
        Ok(())
    }
}

fn validate_tag_color(color: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_hex_color(color) {
        Err(error_response(
            StatusCode::BAD_REQUEST,
            "标签颜色格式无效，需为 #RRGGBB 格式",
        ))
    } else {
        Ok(())
    }
}

#[utoipa::path(
    get,
    path = "/tags",
    tag = "tags",
    summary = "List all tags",
    description = "Get all available tags with usage count",
    params(
        ("page" = Option<i64>, Query, description = "Page number (0-indexed, default 0)"),
        ("size" = Option<i64>, Query, description = "Page size (1-100, default 50)")
    ),
    responses(
        (status = 200, description = "Tag list", body = TagListResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_tags(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TagQuery>,
) -> Result<Json<TagListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let page = query.page.unwrap_or(0).max(0);
    // Clamp size so `page * size` cannot overflow i64 and the query stays bounded.
    let size = query.size.unwrap_or(50).clamp(1, 100);

    let tags = state
        .services
        .tag
        .list_tags(page, size)
        .await
        .map_err(|e| internal_error_log("list_tags failed", &e))?;

    let total = state
        .repos
        .tag
        .count_tags()
        .await
        .map_err(|e| internal_error_log("count_tags failed", &e))?;

    Ok(Json(TagListResponse {
        tags: tags.into_iter().map(TagResponse::from).collect(),
        total,
        page,
        size,
    }))
}

/// `/admin/tags` 与公开的 `/tags` 共用 `list_tags` 逻辑（见 app.rs），但
/// utoipa 一个函数只能携带一条 `#[utoipa::path]`，故用薄包装单独登记
/// admin 路径的文档。
#[utoipa::path(
    get,
    path = "/admin/tags",
    tag = "tags",
    summary = "List all tags (admin)",
    description = "Admin view of all tags with usage count",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(
        ("page" = Option<i64>, Query, description = "Page number (0-indexed, default 0)"),
        ("size" = Option<i64>, Query, description = "Page size (1-100, default 50)")
    ),
    responses(
        (status = 200, description = "Tag list", body = TagListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_tags_admin(
    state: State<Arc<AppState>>,
    query: Query<TagQuery>,
) -> Result<Json<TagListResponse>, (StatusCode, Json<ErrorResponse>)> {
    list_tags(state, query).await
}

#[utoipa::path(
    post,
    path = "/admin/tags",
    tag = "tags",
    summary = "Create a tag",
    description = "Create a new tag. Name must be unique and 1-50 characters.",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    request_body = CreateTagRequest,
    responses(
        (status = 201, description = "Tag created", body = TagResponse),
        (status = 400, description = "Invalid tag name or color"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Tag name already exists")
    )
)]
pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<CreateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_tag_name(&req.name)?;
    if let Some(ref color) = req.color {
        validate_tag_color(color)?;
    }
    let tag = state
        .services
        .tag
        .create_tag(crate::services::tag_service::CreateTagRequest {
            name: req.name,
            color: req.color,
        })
        .await
        .map_err(|e| {
            tracing::error!("create_tag failed: {}", e);
            match e {
                ServiceError::Duplicate(_) => error_response(StatusCode::CONFLICT, e.to_string()),
                _ => error_response(StatusCode::BAD_REQUEST, "创建标签失败"),
            }
        })?;

    Ok(Json(tag.into()))
}

#[utoipa::path(
    get,
    path = "/tags/{id}",
    tag = "tags",
    summary = "Get tag by ID",
    description = "Get a single tag with its usage count",
    params(("id" = i32, Path, description = "Tag ID")),
    responses(
        (status = 200, description = "Tag details", body = TagResponse),
        (status = 404, description = "Tag not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tag = state.services.tag.get_tag(tag_id).await.map_err(|e| {
        tracing::error!("get_tag failed: {}", e);
        match e {
            ServiceError::NotFound(_) => error_response(StatusCode::NOT_FOUND, "标签不存在"),
            _ => error_response(StatusCode::INTERNAL_SERVER_ERROR, "获取标签失败"),
        }
    })?;

    Ok(Json(tag.into()))
}

#[utoipa::path(
    put,
    path = "/admin/tags/{id}",
    tag = "tags",
    summary = "Update a tag",
    description = "Update tag name and/or color",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(("id" = i32, Path, description = "Tag ID")),
    request_body = UpdateTagRequest,
    responses(
        (status = 200, description = "Tag updated", body = TagResponse),
        (status = 400, description = "Invalid tag name or color"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Tag not found"),
        (status = 409, description = "Tag name already exists")
    )
)]
pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
    SafeJson(req): SafeJson<UpdateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(ref name) = req.name {
        validate_tag_name(name)?;
    }
    if let Some(ref color) = req.color {
        validate_tag_color(color)?;
    }
    let tag = state
        .services
        .tag
        .update_tag(
            tag_id,
            crate::services::tag_service::UpdateTagRequest {
                name: req.name,
                color: req.color,
            },
        )
        .await
        .map_err(|e| {
            tracing::error!("update_tag failed: {}", e);
            match e {
                ServiceError::Duplicate(_) => error_response(StatusCode::CONFLICT, e.to_string()),
                ServiceError::NotFound(_) => error_response(StatusCode::NOT_FOUND, "标签不存在"),
                _ => error_response(StatusCode::BAD_REQUEST, "更新标签失败"),
            }
        })?;

    Ok(Json(tag.into()))
}

#[utoipa::path(
    delete,
    path = "/admin/tags/{id}",
    tag = "tags",
    summary = "Delete a tag",
    description = "Delete a tag and all its video associations",
    security(("bearerAuth" = []), ("adminAuth" = [])),
    params(("id" = i32, Path, description = "Tag ID")),
    responses(
        (status = 200, description = "Tag deleted", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Tag not found")
    )
)]
pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state.services.tag.delete_tag(tag_id).await.map_err(|e| {
        tracing::error!("delete_tag failed: {}", e);
        match e {
            ServiceError::NotFound(_) => error_response(StatusCode::NOT_FOUND, "标签不存在"),
            _ => error_response(StatusCode::BAD_REQUEST, "删除标签失败"),
        }
    })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "标签已删除",
    })))
}

#[utoipa::path(
    get,
    path = "/tags/popular",
    tag = "tags",
    summary = "Get popular tags",
    description = "Get most frequently used tags (limit 20)",
    responses(
        (status = 200, description = "Popular tags", body = [TagResponse]),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_popular_tags(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let tags = state
        .services
        .tag
        .get_popular_tags(20)
        .await
        .map_err(|e| internal_error_log("get_popular_tags failed", &e))?;

    Ok(Json(tags.into_iter().map(TagResponse::from).collect()))
}

#[utoipa::path(
    post,
    path = "/videos/{id}/tags",
    tag = "tags",
    summary = "Add tags to a video",
    description = "Add one or more tags to a video by tag IDs",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID (hashid or numeric)")),
    request_body = Vec<i32>,
    responses(
        (status = 200, description = "Tags added", body = serde_json::Value),
        (status = 400, description = "Invalid video ID or tag IDs"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Video or tag not found")
    )
)]
pub async fn add_tags_to_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
    SafeJson(tag_ids): SafeJson<Vec<i32>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    state
        .services
        .tag
        .add_tags_to_video(video_id, &tag_ids, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| {
            tracing::error!("add_tags_to_video failed: {}", e);
            match e {
                ServiceError::Forbidden(_) => error_response(StatusCode::FORBIDDEN, e.to_string()),
                ServiceError::NotFound(_) => error_response(StatusCode::NOT_FOUND, e.to_string()),
                ServiceError::Validation(_) => {
                    error_response(StatusCode::BAD_REQUEST, e.to_string())
                }
                _ => error_response(StatusCode::BAD_REQUEST, "添加标签失败"),
            }
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "标签已添加",
    })))
}

#[utoipa::path(
    delete,
    path = "/videos/{id}/tags",
    tag = "tags",
    summary = "Remove tags from a video",
    description = "按标签 ID 数组从视频上移除标签",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID (hashid or numeric)")),
    request_body = Vec<i32>,
    responses(
        (status = 200, description = "Tags removed", body = serde_json::Value),
        (status = 400, description = "Invalid video ID or tag IDs"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn remove_tags_from_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
    SafeJson(tag_ids): SafeJson<Vec<i32>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    state
        .services
        .tag
        .remove_tags_from_video(video_id, &tag_ids, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| {
            tracing::error!("remove_tags_from_video failed: {}", e);
            match e {
                ServiceError::Forbidden(_) => error_response(StatusCode::FORBIDDEN, e.to_string()),
                _ => error_response(StatusCode::BAD_REQUEST, "移除标签失败"),
            }
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "标签已移除",
    })))
}

#[utoipa::path(
    delete,
    path = "/videos/{id}/tags/{tag_id}",
    tag = "tags",
    summary = "Remove a tag from a video",
    description = "Remove a specific tag from a video",
    security(("bearerAuth" = [])),
    params(
        ("id" = String, Path, description = "Video ID (hashid or numeric)"),
        ("tag_id" = i32, Path, description = "Tag ID")
    ),
    responses(
        (status = 200, description = "Tag removed", body = serde_json::Value),
        (status = 400, description = "Invalid video ID or tag ID"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn remove_tag_from_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path((video_id, tag_id)): Path<(String, i32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    state
        .services
        .tag
        .remove_tag_from_video(video_id, tag_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| {
            tracing::error!("remove_tag_from_video failed: {}", e);
            match e {
                ServiceError::Forbidden(_) => error_response(StatusCode::FORBIDDEN, e.to_string()),
                _ => error_response(StatusCode::BAD_REQUEST, "移除标签失败"),
            }
        })?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "标签已移除",
    })))
}

#[utoipa::path(
    get,
    path = "/videos/{id}/tags",
    tag = "tags",
    summary = "Get tags for a video",
    description = "Get all tags assigned to a video",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID (hashid or numeric)")),
    responses(
        (status = 200, description = "Video tags", body = [TagResponse]),
        (status = 400, description = "Invalid video ID"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_video_tags(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<String>,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let tags = state
        .services
        .tag
        .get_video_tags(video_id)
        .await
        .map_err(|e| internal_error_log("get_video_tags failed", &e))?;

    Ok(Json(tags.into_iter().map(TagResponse::from).collect()))
}
