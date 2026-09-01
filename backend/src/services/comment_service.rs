use crate::repositories::comment_repo::{CommentRepository, CommentRow};
use crate::repositories::video_repo::VideoRepository;
use crate::util::db_error;
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
    /// 创建一个新的 `CommentService` 实例。
    ///
    /// # 参数
    ///
    /// * `repo` - 评论数据仓库，负责评论的数据库读写操作。
    /// * `video_repo` - 视频数据仓库，用于视频所有权校验等辅助查询。
    pub fn new(repo: CommentRepository, video_repo: VideoRepository) -> Self {
        Self { repo, video_repo }
    }

    /// 分页获取指定视频的顶层评论列表。
    ///
    /// 返回评论列表及符合条件的总评论数，用于前端分页渲染。
    /// 评论按时间倒序排列（最新的在前）。
    ///
    /// # 参数
    ///
    /// * `tenant_id` - 租户 ID（评论隔离）。
    /// * `video_id` - 目标视频的 ID。
    /// * `page` - 页码，从 0 开始；负值会被视为 0。
    /// * `size` - 每页数量，会被限制在 1 ~ 100 之间。
    ///
    /// # 返回值
    ///
    /// * `Ok((comments, total))` - 评论列表和总评论数的元组。
    ///
    /// # 错误
    ///
    /// * `ServiceError` - 数据库查询失败时返回内部错误。
    pub async fn list_comments(
        &self,
        tenant_id: i64,
        video_id: i64,
        page: i64,
        size: i64,
    ) -> Result<(Vec<CommentRow>, i64), ServiceError> {
        let size = size.clamp(1, 100);
        let offset = page.max(1).saturating_sub(1).saturating_mul(size);
        let (comments, total) = tokio::try_join!(
            self.repo.get_comments(tenant_id, video_id, size, offset),
            self.repo.count_comments(tenant_id, video_id),
        )?;
        Ok((comments, total))
    }

    /// 获取指定评论的所有直接回复（子评论）。
    ///
    /// 为防止单次请求返回过多数据，最多返回 `MAX_REPLIES_PER_REQUEST`（50）条回复。
    ///
    /// # 参数
    ///
    /// * `tenant_id` - 租户 ID（评论隔离）。
    /// * `comment_id` - 父评论的 ID。
    ///
    /// # 返回值
    ///
    /// * `Ok(replies)` - 回复评论列表，按时间正序排列。
    ///
    /// # 错误
    ///
    /// * `ServiceError` - 数据库查询失败时返回内部错误。
    pub async fn list_replies(
        &self,
        tenant_id: i64,
        comment_id: i64,
    ) -> Result<Vec<CommentRow>, ServiceError> {
        let replies = self
            .repo
            .get_replies(tenant_id, comment_id, MAX_REPLIES_PER_REQUEST)
            .await?;
        Ok(replies)
    }

    /// 创建新评论或回复已有评论。
    ///
    /// 执行完整的创建流程：权限校验（视频所有者或管理员）、内容清理消毒（去除 HTML 标签、
    /// 防 XSS 攻击、截断超长内容）、父评论解析（确保回复链最多两层：顶层评论 + 回复）、
    /// 持久化到数据库。
    ///
    /// # 参数
    ///
    /// * `video_id` - 评论所属视频的 ID。
    /// * `user_id` - 发表评论的用户 ID。
    /// * `raw_content` - 用户输入的原始评论内容，可能包含 HTML 标签，会被自动清理。
    /// * `parent_id` - 父评论 ID，`None` 表示发表顶层评论，`Some(id)` 表示回复指定评论。
    /// * `is_admin` - 当前用户是否为管理员，管理员可跳过视频所有权检查。
    ///
    /// # 返回值
    ///
    /// * `Ok(comment)` - 成功创建的评论记录。
    ///
    /// # 错误
    ///
    /// * `ServiceError::Forbidden` - 非视频所有者且非管理员时，无权在该视频下评论。
    /// * `ServiceError::BadRequest("视频不存在")` - 指定的视频不存在。
    /// * `ServiceError::BadRequest("评论内容 1-N 字符")` - 内容为空或超过最大长度（2000 字符）。
    /// * `ServiceError::BadRequest("父评论不存在")` - 指定的父评论不存在。
    /// * `ServiceError::BadRequest("父评论不属于该视频")` - 父评论不属于目标视频。
    /// * `ServiceError::BadRequest("评论层级过深")` - 回复链嵌套超过最大深度（10 层）。
    /// * `ServiceError` - 数据库插入失败（如并发场景下视频/评论被删除导致外键冲突）。
    pub async fn create_comment(
        &self,
        tenant_id: i64,
        video_id: i64,
        user_id: i64,
        raw_content: &str,
        parent_id: Option<i64>,
        is_admin: bool,
    ) -> Result<CommentRow, ServiceError> {
        // 视频所有权检查：只有视频所有者或管理员才能评论
        if !is_admin {
            self.check_video_ownership(tenant_id, video_id, user_id)
                .await?;
        }
        let content = sanitize_content(raw_content)?;
        let effective_parent = self
            .resolve_parent_comment(tenant_id, video_id, parent_id)
            .await?;
        let comment = self
            .repo
            .create_comment(tenant_id, video_id, user_id, &content, effective_parent)
            .await
            .map_err(map_create_error)?;
        Ok(comment)
    }

    /// 解析父评论并确保回复链最多两层。
    ///
    /// 当用户提供 `parent_id` 时，沿评论的 `parent_id` 链向上追溯，
    /// 直到找到顶层评论（`parent_id` 为 `None` 的评论），并返回其 ID。
    /// 这确保所有回复都挂在顶层评论下，而非嵌套回复。
    ///
    /// 当 `parent_id` 为 `None` 时，仅验证视频是否存在。
    ///
    /// # 参数
    ///
    /// * `video_id` - 评论所属视频的 ID。
    /// * `parent_id` - 用户指定的父评论 ID，`None` 表示顶层评论。
    ///
    /// # 返回值
    ///
    /// * `Ok(None)` - 发表顶层评论，且视频已确认存在。
    /// * `Ok(Some(top_comment_id))` - 回复评论，返回最终归属的顶层评论 ID。
    ///
    /// # 错误
    ///
    /// * `ServiceError::BadRequest("视频不存在")` - 发表顶层评论时视频不存在。
    /// * `ServiceError::BadRequest("父评论不存在")` - 链路上任意节点评论不存在。
    /// * `ServiceError::BadRequest("父评论不属于该视频")` - 父评论与目标视频不匹配。
    /// * `ServiceError::BadRequest("评论层级过深")` - 向上追溯超过 `MAX_REPLY_DEPTH`（10）层。
    async fn resolve_parent_comment(
        &self,
        tenant_id: i64,
        video_id: i64,
        parent_id: Option<i64>,
    ) -> Result<Option<i64>, ServiceError> {
        let Some(mut current_id) = parent_id else {
            if !self.repo.video_exists(tenant_id, video_id).await? {
                return Err(ServiceError::bad_request("视频不存在".to_string()));
            }
            return Ok(None);
        };

        let mut depth = 0u32;
        loop {
            let (parent_video_id, grandparent_id) = self
                .repo
                .get_comment_meta(tenant_id, current_id)
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

    /// 检查用户是否是指定视频的所有者（上传者）。
    ///
    /// # 参数
    ///
    /// * `video_id` - 视频 ID。
    /// * `user_id` - 用户 ID。
    ///
    /// # 返回值
    ///
    /// * `Ok(())` - 用户是视频所有者。
    ///
    /// # 错误
    ///
    /// * `ServiceError::BadRequest("视频不存在")` - 视频不存在。
    /// * `ServiceError::Forbidden("无权操作")` - 用户不是视频所有者。
    /// * `ServiceError::Internal("数据库错误")` - 数据库查询失败。
    async fn check_video_ownership(
        &self,
        tenant_id: i64,
        video_id: i64,
        user_id: i64,
    ) -> Result<(), ServiceError> {
        let video = self
            .video_repo
            .find_by_id(tenant_id, video_id)
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

    /// 删除指定评论。
    ///
    /// 管理员可以删除任意评论，普通用户只能删除自己发布的评论。
    /// 如果评论不存在，返回 404 错误。
    ///
    /// # 参数
    ///
    /// * `tenant_id` - 租户 ID（评论隔离）。
    /// * `comment_id` - 要删除的评论 ID。
    /// * `user_id` - 请求删除操作的用户 ID。
    /// * `is_admin` - 当前用户是否为管理员。
    ///
    /// # 返回值
    ///
    /// * `Ok(())` - 删除成功。
    ///
    /// # 错误
    ///
    /// * `ServiceError::NotFound("评论不存在")` - 指定评论不存在，或普通用户尝试删除非本人评论。
    /// * `ServiceError` - 数据库操作失败时返回内部错误。
    pub async fn delete_comment(
        &self,
        tenant_id: i64,
        comment_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        let deleted = if is_admin {
            self.repo
                .delete_comment_admin(tenant_id, comment_id)
                .await?
        } else {
            self.repo
                .delete_comment(tenant_id, comment_id, user_id)
                .await?
        };
        if !deleted {
            return Err(ServiceError::not_found("评论不存在"));
        }
        Ok(())
    }
}

/// 去除文本中的 HTML 标签，防止存储型 XSS 攻击。
///
/// 使用 `ammonia` 库处理所有 HTML/XSS 向量，包括标签、属性、URL、
/// 实体编码和 Unicode 规范化攻击。所有 HTML 标签都会被完全移除。
///
/// # 参数
///
/// * `input` - 可能包含 HTML 标签的原始文本。
///
/// # 返回值
///
/// 清理后的纯文本字符串，所有 HTML 标签和脚本内容已被移除。
fn sanitize_text(input: &str) -> String {
    ammonia::Builder::new()
        .tags(std::collections::HashSet::new())
        .clean(input)
        .to_string()
}

/// 对评论内容进行完整的清理和校验。
///
/// 执行三步处理：1) 去除首尾空白字符；2) 清除 HTML 标签（防 XSS）；
/// 3) 验证清理后的内容长度在 1 ~ `MAX_COMMENT_LEN`（2000）字符之间。
///
/// # 参数
///
/// * `raw_content` - 用户输入的原始评论内容。
///
/// # 返回值
///
/// * `Ok(content)` - 清理校验通过的评论内容。
///
/// # 错误
///
/// * `ServiceError::BadRequest` - 清理后内容为空（如仅含空白/HTML 标签），
///   或超过最大长度限制。
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

/// 将数据库层的 INSERT 错误映射为用户友好的业务错误。
///
/// 主要处理外键约束冲突：在 `resolve_parent_comment` 的存在性检查和实际 INSERT 之间
/// 存在竞态窗口（如视频被并发删除），此时数据库会抛出外键违反错误。
/// 该函数将其转换为可理解的 400 错误，而非暴露为 500 内部错误。
///
/// # 参数
///
/// * `e` - SQLx 返回的数据库错误。
///
/// # 返回值
///
/// * `ServiceError::BadRequest` - 外键约束冲突时，根据约束名返回对应的中文提示。
/// * `ServiceError` - 其他数据库错误直接透传。
fn map_create_error(e: sqlx::Error) -> ServiceError {
    if db_error::is_foreign_key_violation(&e) {
        return match db_error::get_constraint_name(&e) {
            Some("comments_video_id_fkey") => ServiceError::bad_request("视频不存在".to_string()),
            Some("comments_parent_id_fkey") => {
                ServiceError::bad_request("父评论不存在".to_string())
            }
            _ => ServiceError::bad_request("评论内容无效".to_string()),
        };
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
