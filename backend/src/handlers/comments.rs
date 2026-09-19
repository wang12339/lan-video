use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthUser;
use crate::models::comment::{
    CommentListResponse, CommentQuery, CommentResponse, CreateCommentRequest,
};
use crate::state::AppState;
use crate::util::error::ServiceError;
use crate::util::hashid;
use crate::util::pagination::PaginationParams;
use crate::util::response::{error_response, ErrorResponse, SafeJson};

const MAX_COMMENT_LENGTH: usize = 2000;

fn sanitize_comment_content(content: &str) -> String {
    content
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .trim()
        .to_string()
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
#[utoipa::path(
    get,
    path = "/videos/{id}/comments",
    tag = "comments",
    summary = "List comments for a video",
    description = "分页返回视频的顶级评论",
    security(("bearerAuth" = [])),
    params(
        ("id" = String, Path, description = "Video ID (hashid or numeric)"),
        ("page" = Option<i64>, Query, description = "Page number (0-indexed)"),
        ("size" = Option<i64>, Query, description = "Page size (max 100)")
    ),
    responses(
        (status = 200, description = "Comment list", body = CommentListResponse),
        (status = 400, description = "Invalid video ID"),
        (status = 500, description = "Internal server error")
    )
)]
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
#[utoipa::path(
    get,
    path = "/comments/{id}/replies",
    tag = "comments",
    summary = "List replies to a comment",
    description = "返回某条评论的全部回复",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "评论 ID (hashid 或数字)")),
    responses(
        (status = 200, description = "Reply list", body = [CommentResponse]),
        (status = 400, description = "Invalid comment ID"),
        (status = 500, description = "Internal server error")
    )
)]
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
#[utoipa::path(
    post,
    path = "/videos/{id}/comments",
    tag = "comments",
    summary = "Create a comment",
    description = "发表评论（可指定 parent_id 回复某条评论）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "Video ID (hashid or numeric)")),
    request_body = CreateCommentRequest,
    responses(
        (status = 201, description = "Comment created", body = CommentResponse),
        (status = 400, description = "Empty or oversized comment"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_comment(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    Path(video_id): Path<String>,
    SafeJson(req): SafeJson<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CommentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let video_id = hashid::decode_id_or_numeric(&video_id)
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "无效的视频ID"))?;
    let sanitized = sanitize_comment_content(&req.content);
    if sanitized.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "评论内容不能为空"));
    }
    if sanitized.len() > MAX_COMMENT_LENGTH {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "评论内容不能超过 2000 个字符",
        ));
    }
    let comment = state
        .services
        .comment
        .create_comment(
            video_id,
            auth_user.id,
            &sanitized,
            req.parent_id,
            auth_user.is_admin,
        )
        .await
        .map_err(ServiceError::into_tuple)?;
    Ok((StatusCode::CREATED, Json(map_comment(comment))))
}

/// DELETE /comments/{id}
#[utoipa::path(
    delete,
    path = "/comments/{id}",
    tag = "comments",
    summary = "Delete a comment",
    description = "删除评论（作者本人或管理员）",
    security(("bearerAuth" = [])),
    params(("id" = String, Path, description = "评论 ID (hashid 或数字)")),
    responses(
        (status = 200, description = "Comment deleted", body = serde_json::Value),
        (status = 400, description = "Invalid comment ID"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Comment not found or not permitted")
    )
)]
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
