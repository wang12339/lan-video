use crate::repositories::comment_repo::{CommentRepository, CommentRow};
use crate::repositories::video_repo::VideoRepository;
use crate::util::error::ServiceError;

/// Comment-related business logic.
/// Encapsulates validation, sanitization, and authorization so that
/// handlers stay thin and consistent.
#[derive(Clone)]
pub struct CommentService {
    repo: CommentRepository,
    video_repo: VideoRepository,
}

const MAX_COMMENT_LEN: usize = 2000;
const MAX_REPLIES_PER_REQUEST: i64 = 50;
const MAX_REPLY_DEPTH: u32 = 10;

impl CommentService {
    pub fn new(repo: CommentRepository, video_repo: VideoRepository) -> Self {
        Self { repo, video_repo }
    }

    pub async fn list_comments(
        &self,
        video_id: i64,
        page: i64,
        size: i64,
    ) -> Result<(Vec<CommentRow>, i64), ServiceError> {
        let size = size.clamp(1, 100);
        let offset = page.max(0).saturating_mul(size);
        let comments = self.repo.get_comments(video_id, size, offset).await?;
        let total = self.repo.count_comments(video_id).await?;
        Ok((comments, total))
    }

    pub async fn list_replies(&self, comment_id: i64) -> Result<Vec<CommentRow>, ServiceError> {
        let replies = self
            .repo
            .get_replies(comment_id, MAX_REPLIES_PER_REQUEST)
            .await?;
        Ok(replies)
    }

    /// Validate + sanitize the input, then create a comment.
    pub async fn create_comment(
        &self,
        video_id: i64,
        user_id: i64,
        raw_content: &str,
        parent_id: Option<i64>,
        is_admin: bool,
    ) -> Result<CommentRow, ServiceError> {
        // 视频所有权检查：只有视频所有者或管理员才能评论
        if !is_admin {
            self.check_video_ownership(video_id, user_id).await?;
        }
        let content = sanitize_content(raw_content)?;
        let effective_parent = self.resolve_parent_comment(video_id, parent_id).await?;
        let comment = self
            .repo
            .create_comment(video_id, user_id, &content, effective_parent)
            .await
            .map_err(map_create_error)?;
        Ok(comment)
    }

    /// Validate the reply target and normalize it to a top-level comment so
    /// threads stay at most two levels deep (top-level comment + replies).
    /// Returns `Ok(None)` for a root comment (after verifying the video
    /// exists) and `Ok(Some(top_comment_id))` for a reply.
    async fn resolve_parent_comment(
        &self,
        video_id: i64,
        parent_id: Option<i64>,
    ) -> Result<Option<i64>, ServiceError> {
        let Some(mut current_id) = parent_id else {
            if !self.repo.video_exists(video_id).await? {
                return Err(ServiceError::bad_request("视频不存在".to_string()));
            }
            return Ok(None);
        };

        let mut depth = 0u32;
        loop {
            let (parent_video_id, grandparent_id) =
                self.repo
                    .get_comment_meta(current_id)
                    .await?
                    .ok_or_else(|| ServiceError::bad_request("父评论不存在".to_string()))?;
            if parent_video_id != video_id {
                return Err(ServiceError::bad_request("父评论不属于该视频".to_string()));
            }
            match grandparent_id {
                Some(ancestor_id) => {
                    current_id = ancestor_id;
                    depth += 1;
                    if depth >= MAX_REPLY_DEPTH {
                        return Err(ServiceError::bad_request("评论层级过深".to_string()));
                    }
                }
                None => return Ok(Some(current_id)),
            }
        }
    }

    /// 检查用户是否是视频所有者
    async fn check_video_ownership(&self, video_id: i64, user_id: i64) -> Result<(), ServiceError> {
        let video = self
            .video_repo
            .find_by_id(video_id)
            .await
            .map_err(|e| {
                tracing::error!("查询视频失败: {}", e);
                ServiceError::internal("数据库错误")
            })?
            .ok_or_else(|| ServiceError::bad_request("视频不存在".to_string()))?;
        if video.uploader_id != Some(user_id) {
            return Err(ServiceError::forbidden("无权操作"));
        }
        Ok(())
    }

    pub async fn delete_comment(
        &self,
        comment_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        let deleted = if is_admin {
            self.repo.delete_comment_admin(comment_id).await?
        } else {
            self.repo.delete_comment(comment_id, user_id).await?
        };
        if !deleted {
            return Err(ServiceError::not_found("评论不存在"));
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

/// Trim, strip HTML and enforce the length limit before persisting.
fn sanitize_content(raw_content: &str) -> Result<String, ServiceError> {
    let content = sanitize_text(raw_content.trim());
    if content.is_empty() || content.chars().count() > MAX_COMMENT_LEN {
        return Err(ServiceError::bad_request(format!(
            "评论内容 1-{} 字符",
            MAX_COMMENT_LEN
        )));
    }
    Ok(content)
}

/// Map DB errors from the comment INSERT to user-friendly errors. FK
/// violations can still occur in the race between the existence checks in
/// `resolve_parent_comment` and the insert (e.g. the video is deleted
/// concurrently), and otherwise surface as a confusing 500.
fn map_create_error(e: sqlx::Error) -> ServiceError {
    if let Some(db) = e.as_database_error() {
        if db.code().is_some_and(|code| code == "23503") {
            return match db.constraint() {
                Some("comments_video_id_fkey") => {
                    ServiceError::bad_request("视频不存在".to_string())
                }
                Some("comments_parent_id_fkey") => {
                    ServiceError::bad_request("父评论不存在".to_string())
                }
                _ => ServiceError::bad_request("评论内容无效".to_string()),
            };
        }
    }
    e.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_html_tags() {
        let out = sanitize_text("<script>alert(1)</script>hello <b>world</b>");
        assert!(!out.contains('<'));
        assert!(!out.contains("alert(1)"));
        assert!(out.contains("hello"));
        assert!(out.contains("world"));
    }

    #[test]
    fn sanitize_preserves_plain_text() {
        let out = sanitize_text(" 你好，世界！ ");
        assert!(out.contains("你好，世界！"));
        assert!(!out.contains('<'));
    }

    #[test]
    fn empty_content_rejected() {
        assert!(matches!(
            sanitize_content("   "),
            Err(ServiceError::BadRequest(_))
        ));
        assert!(matches!(
            sanitize_content("\n\t"),
            Err(ServiceError::BadRequest(_))
        ));
        assert!(matches!(
            sanitize_content("<br>"),
            Err(ServiceError::BadRequest(_))
        ));
    }

    #[test]
    fn content_too_long_rejected() {
        let long = "a".repeat(MAX_COMMENT_LEN + 1);
        assert!(matches!(
            sanitize_content(&long),
            Err(ServiceError::BadRequest(_))
        ));
    }

    #[test]
    fn content_at_max_length_accepted() {
        let max = "a".repeat(MAX_COMMENT_LEN);
        assert_eq!(sanitize_content(&max).unwrap(), max);
    }

    #[test]
    fn html_padding_does_not_bypass_length_limit() {
        let mut padded = String::new();
        padded.push_str(&"<div>".repeat(MAX_COMMENT_LEN));
        padded.push_str("hi");
        assert_eq!(sanitize_content(&padded).unwrap(), "hi");
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let padded = format!("  {}  ", "a".repeat(MAX_COMMENT_LEN));
        assert_eq!(sanitize_content(&padded).unwrap().len(), MAX_COMMENT_LEN);
    }
}
