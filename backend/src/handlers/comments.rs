use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::comment::{CommentListResponse, CommentQuery, CommentResponse, CreateCommentRequest};
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::hashid;
use crate::util::pagination::PaginationParams;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

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
    Path(video_id): Path<String>,
    Query(q): Query<CommentQuery>,
) -> Result<Json<CommentListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let pagination = PaginationParams::new(q.page, q.size);
    let page = pagination.page;
    let size = pagination.page_size;
    let (comments, total) = state
        .services
        .comment
        .list_comments(video_id, page, size)
        .await
        .map_err(ServiceError::into_tuple)?;
    Ok(Json(CommentListResponse {
        comments: comments.into_iter().map(map_comment).collect(),
        total,
    }))
}

/// GET /comments/{id}/replies
pub async fn list_replies(
    State(state): State<Arc<AppState>>,
    Path(comment_id): Path<String>,
) -> Result<Json<Vec<CommentResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let comment_id = hashid::decode_id_or_numeric(&comment_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的评论ID"))?;
    let replies = state
        .services
        .comment
        .list_replies(comment_id)
        .await
        .map_err(ServiceError::into_tuple)?;
    Ok(Json(replies.into_iter().map(map_comment).collect()))
}

/// POST /videos/{id}/comments
pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
    SafeJson(req): SafeJson<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CommentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let comment = state
        .services
        .comment
        .create_comment(
            video_id,
            auth_user.id,
            &req.content,
            req.parent_id,
            auth_user.is_admin,
        )
        .await
        .map_err(ServiceError::into_tuple)?;
    Ok((StatusCode::CREATED, Json(map_comment(comment))))
}

/// DELETE /comments/{id}
pub async fn delete_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(comment_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let comment_id = hashid::decode_id_or_numeric(&comment_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的评论ID"))?;
    state
        .services
        .comment
        .delete_comment(comment_id, auth_user.id, auth_user.is_admin)
        .await
        .map_err(|e| match e {
            ServiceError::NotFound(_) => {
                error_response(StatusCode::NOT_FOUND, "评论不存在或无权删除")
            }
            other => other.into_tuple(),
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
