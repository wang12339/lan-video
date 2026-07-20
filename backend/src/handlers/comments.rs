use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::services::comment_service::CommentError;
use crate::state::AppState;
use crate::util::response::{error_response, ErrorResponse};

#[derive(Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    pub parent_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct CommentQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentResponse {
    pub id: i64,
    pub video_id: i64,
    pub user_id: i64,
    pub username: String,
    pub avatar_url: Option<String>,
    pub content: String,
    pub parent_id: Option<i64>,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentListResponse {
    pub comments: Vec<CommentResponse>,
    pub total: i64,
}

fn map_comment(c: crate::repositories::comment_repo::CommentRow) -> CommentResponse {
    CommentResponse {
        id: c.id,
        video_id: c.video_id,
        user_id: c.user_id,
        username: c.username,
        avatar_url: c.avatar_url,
        content: c.content,
        parent_id: c.parent_id,
        created_at: c.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

/// GET /videos/{id}/comments
pub async fn list_comments(
    State(state): State<Arc<AppState>>,
    Path(video_id): Path<i64>,
    Query(q): Query<CommentQuery>,
) -> Result<Json<CommentListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let page = q.page.unwrap_or(0);
    let size = q.size.unwrap_or(20).min(100);
    let (comments, total) = state
        .comment_service
        .list_comments(video_id, page, size)
        .await
        .map_err(|e| {
            if let CommentError::Internal(msg) = &e {
                tracing::error!("list_comments error: {}", msg);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("服务器内部错误: {}", msg))
            } else {
                e.into_response()
            }
        })?;
    Ok(Json(CommentListResponse {
        comments: comments.into_iter().map(map_comment).collect(),
        total,
    }))
}

/// GET /comments/{id}/replies
pub async fn list_replies(
    State(state): State<Arc<AppState>>,
    Path(comment_id): Path<i64>,
) -> Result<Json<Vec<CommentResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let replies = state
        .comment_service
        .list_replies(comment_id)
        .await
        .map_err(|e| match e {
            CommentError::Internal(_) => {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
            }
            other => other.into_response(),
        })?;
    Ok(Json(replies.into_iter().map(map_comment).collect()))
}

/// POST /videos/{id}/comments
pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<i64>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CommentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let comment = state
        .comment_service
        .create_comment(video_id, auth_user.id, &req.content, req.parent_id)
        .await
        .map_err(|e| match e {
            CommentError::Invalid(msg) => error_response(StatusCode::BAD_REQUEST, msg),
            CommentError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            other => other.into_response(),
        })?;
    Ok((StatusCode::CREATED, Json(map_comment(comment))))
}

/// DELETE /comments/{id}
pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(comment_id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state
        .comment_service
        .delete_comment(comment_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| match e {
            CommentError::NotFound => {
                error_response(StatusCode::NOT_FOUND, "评论不存在或无权删除")
            }
            CommentError::Internal(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, msg),
            other => other.into_response(),
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
