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

/// GET /tags
///
/// List all tags with pagination
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

/// POST /tags
///
/// Create a new tag (admin only)
pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    SafeJson(req): SafeJson<CreateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
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

/// GET /tags/{id}
///
/// Get a specific tag
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

/// PUT /tags/{id}
///
/// Update a tag (admin only)
pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
    SafeJson(req): SafeJson<UpdateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
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

/// DELETE /tags/{id}
///
/// Delete a tag (admin only)
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

/// GET /tags/popular
///
/// Get popular tags
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

/// POST /videos/{id}/tags
///
/// Add tags to a video
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

/// DELETE /videos/{id}/tags
///
/// Remove tags from a video
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

/// DELETE /videos/{id}/tags/{tag_id}
///
/// Remove a single tag from a video
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

/// GET /videos/{id}/tags
///
/// Get tags for a video
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
