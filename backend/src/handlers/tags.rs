use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

#[derive(Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Serialize)]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
    pub usage_count: i32,
}

#[derive(Serialize)]
pub struct TagListResponse {
    pub tags: Vec<TagResponse>,
    pub total: i64,
    pub page: i64,
    pub size: i64,
}

#[derive(Deserialize)]
pub struct TagQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
}

impl From<crate::services::tag_service::TagResponse> for TagResponse {
    fn from(t: crate::services::tag_service::TagResponse) -> Self {
        TagResponse {
            id: t.id,
            name: t.name,
            color: t.color,
            usage_count: t.usage_count,
        }
    }
}

/// GET /tags
///
/// List all tags with pagination
pub async fn list_tags(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TagQuery>,
) -> Result<Json<TagListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let page = query.page.unwrap_or(0);
    let size = query.size.unwrap_or(50);

    let tags = state
        .tag_service
        .list_tags(page, size)
        .await
        .map_err(|e| {
            tracing::error!("list_tags failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    let total = state
        .tag_repo
        .count_tags()
        .await
        .map_err(|e| {
            tracing::error!("count_tags failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

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
    Json(req): Json<CreateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tag = state
        .tag_service
        .create_tag(crate::services::tag_service::CreateTagRequest {
            name: req.name,
            color: req.color,
        })
        .await
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

    Ok(Json(tag.into()))
}

/// GET /tags/{id}
///
/// Get a specific tag
pub async fn get_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tag = state
        .tag_service
        .get_tag(tag_id)
        .await
        .map_err(|e| error_response(StatusCode::NOT_FOUND, e))?;

    Ok(Json(tag.into()))
}

/// PUT /tags/{id}
///
/// Update a tag (admin only)
pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
    Json(req): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tag = state
        .tag_service
        .update_tag(
            tag_id,
            crate::services::tag_service::UpdateTagRequest {
                name: req.name,
                color: req.color,
            },
        )
        .await
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

    Ok(Json(tag.into()))
}

/// DELETE /tags/{id}
///
/// Delete a tag (admin only)
pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .tag_service
        .delete_tag(tag_id)
        .await
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

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
        .tag_service
        .get_popular_tags(20)
        .await
        .map_err(|e| {
            tracing::error!("get_popular_tags failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    Ok(Json(tags.into_iter().map(TagResponse::from).collect()))
}

/// POST /videos/{id}/tags
///
/// Add tags to a video
pub async fn add_tags_to_video(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<i64>,
    Json(tag_ids): Json<Vec<i32>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .tag_service
        .add_tags_to_video(video_id, &tag_ids)
        .await
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

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
    Path(video_id): Path<i64>,
    Json(tag_ids): Json<Vec<i32>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .tag_service
        .remove_tags_from_video(video_id, &tag_ids)
        .await
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

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
    Path((video_id, tag_id)): Path<(i64, i32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .tag_service
        .remove_tag_from_video(video_id, tag_id)
        .await
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

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
    Path(video_id): Path<i64>,
) -> Result<Json<Vec<TagResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let tags = state
        .tag_service
        .get_video_tags(video_id)
        .await
        .map_err(|e| {
            tracing::error!("get_video_tags failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;

    Ok(Json(tags.into_iter().map(TagResponse::from).collect()))
}
