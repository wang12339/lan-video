use crate::repositories::comment_repo::{CommentRepository, CommentRow};
use crate::util::response::ErrorResponse;
use axum::{http::StatusCode, Json};

/// Comment-related business logic.
/// Encapsulates validation, sanitization, and authorization so that
/// handlers stay thin and consistent.
#[derive(Clone)]
pub struct CommentService {
    repo: CommentRepository,
}

const MAX_COMMENT_LEN: usize = 2000;
const MAX_REPLIES_PER_REQUEST: i64 = 50;

pub enum CommentError {
    NotFound,
    Forbidden,
    Invalid(String),
    Internal(String),
}

impl CommentError {
    pub fn into_response(self) -> (StatusCode, Json<ErrorResponse>) {
        match self {
            CommentError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse { error: "评论不存在".into() }),
            ),
            CommentError::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse { error: "无权操作".into() }),
            ),
            CommentError::Invalid(msg) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: msg }),
            ),
            CommentError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: msg }),
            ),
        }
    }
}

impl From<sqlx::Error> for CommentError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("comment service sqlx error: {}", e);
        CommentError::Internal("数据库错误".into())
    }
}

impl From<String> for CommentError {
    fn from(s: String) -> Self {
        CommentError::Internal(s)
    }
}

impl CommentService {
    pub fn new(repo: CommentRepository) -> Self {
        Self { repo }
    }

    pub async fn list_comments(
        &self,
        video_id: i64,
        page: i64,
        size: i64,
    ) -> Result<(Vec<CommentRow>, i64), CommentError> {
        let size = size.clamp(1, 100);
        let offset = (page.max(0)) * size;
        let comments = self.repo.get_comments(video_id, size, offset).await?;
        let total = self.repo.count_comments(video_id).await?;
        Ok((comments, total))
    }

    pub async fn list_replies(
        &self,
        comment_id: i64,
    ) -> Result<Vec<CommentRow>, CommentError> {
        let replies = self.repo.get_replies(comment_id, MAX_REPLIES_PER_REQUEST).await?;
        Ok(replies)
    }

    /// Validate + sanitize the input, then create a comment.
    pub async fn create_comment(
        &self,
        video_id: i64,
        user_id: i64,
        raw_content: &str,
        parent_id: Option<i64>,
    ) -> Result<CommentRow, CommentError> {
        let content = sanitize_text(raw_content.trim());
        if content.is_empty() || content.len() > MAX_COMMENT_LEN {
            return Err(CommentError::Invalid(format!(
                "评论内容 1-{} 字符",
                MAX_COMMENT_LEN
            )));
        }
        let comment = self
            .repo
            .create_comment(video_id, user_id, &content, parent_id)
            .await?;
        Ok(comment)
    }

    pub async fn delete_comment(
        &self,
        comment_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), CommentError> {
        let deleted = if is_admin {
            self.repo.delete_comment_admin(comment_id).await?
        } else {
            self.repo.delete_comment(comment_id, user_id).await?
        };
        if !deleted {
            return Err(CommentError::NotFound);
        }
        Ok(())
    }
}

/// Strip HTML tags to prevent stored XSS.
/// Uses the `ammonia` crate which handles all HTML/XSS vectors:
/// tags, attributes, URLs, entities, Unicode normalization attacks.
fn sanitize_text(input: &str) -> String {
    ammonia::Builder::new()
        .tags(std::collections::HashSet::new())
        .clean(input)
        .to_string()
}
