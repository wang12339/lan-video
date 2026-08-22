use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::repositories::tag_repo::TagRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::util::db_error;
use crate::util::error::ServiceError;

/// Maximum length of a tag name (bytes, mirrors the `tags.name VARCHAR(100)` column).
const MAX_TAG_NAME_LEN: usize = 100;
/// Maximum number of tags that can be attached to a video in one request.
const MAX_TAGS_PER_VIDEO: usize = 100;

/// Normalize a tag name: trim surrounding whitespace and collapse internal
/// runs of whitespace into a single space ("  Rust   Lang " -> "Rust Lang").
fn normalize_tag_name(name: &str) -> String {
    let mut words = name.split_whitespace();
    let mut normalized = words.next().unwrap_or_default().to_string();
    for word in words {
        normalized.push(' ');
        normalized.push_str(word);
    }
    normalized
}

/// Validate a `#RRGGBB` color value (hex digits only, case-insensitive).
fn validate_color(color: &str) -> Result<(), ServiceError> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(ServiceError::validation("颜色格式无效，应为#RRGGBB"))
    }
}

/// Validate a tag name: normalized, non-empty, within length limits and free
/// of control characters. Returns the normalized name.
fn validate_tag_name(name: &str) -> Result<String, ServiceError> {
    let normalized = normalize_tag_name(name);
    if normalized.is_empty() || normalized.len() > MAX_TAG_NAME_LEN {
        return Err(ServiceError::validation(format!(
            "标签名长度必须在1-{}之间",
            MAX_TAG_NAME_LEN
        )));
    }
    if normalized.chars().any(|c| c.is_control()) {
        return Err(ServiceError::validation("标签名包含非法控制字符"));
    }
    Ok(normalized)
}

/// Deduplicate a slice of tag IDs while preserving ascending order.
fn dedupe_tag_ids(tag_ids: &[i32]) -> Vec<i32> {
    let mut ids: Vec<i32> = tag_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// 创建标签的请求参数。
///
/// `name` 是必填项，会自动进行标准化处理（去除首尾空白、合并内部空格）。
/// `color` 为可选的 `#RRGGBB` 格式颜色值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

/// 更新标签的请求参数。
///
/// 所有字段均为可选，仅更新提供的字段。
/// `name` 为新名称（会自动标准化），`color` 为新的 `#RRGGBB` 颜色值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

/// 标签的 API 响应结构。
///
/// `usage_count` 表示当前有多少视频使用了该标签。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
    pub usage_count: i32,
}

impl From<crate::repositories::tag_repo::Tag> for TagResponse {
    fn from(t: crate::repositories::tag_repo::Tag) -> Self {
        TagResponse {
            id: t.id,
            name: t.name,
            color: t.color,
            usage_count: t.usage_count,
        }
    }
}

/// 标签服务，处理标签的 CRUD 操作以及视频与标签的关联管理。
///
/// 职责包括：
/// - 标签的创建、更新、删除和查询
/// - 标签名称和颜色的验证与标准化
/// - 视频与标签的关联/取消关联（含权限校验）
#[derive(Debug, Clone)]
pub struct TagService {
    tag_repo: TagRepository,
    video_repo: VideoRepository,
}

impl TagService {
    /// 创建一个新的 `TagService` 实例。
    ///
    /// # 参数
    /// - `tag_repo`：标签数据仓库，负责标签的数据库操作
    /// - `video_repo`：视频数据仓库，用于视频所有权校验
    pub fn new(tag_repo: TagRepository, video_repo: VideoRepository) -> Self {
        Self {
            tag_repo,
            video_repo,
        }
    }

    /// 创建新标签。
    ///
    /// 标签名称会被自动标准化（去首尾空白、合并内部空格），颜色值可选。
    /// 利用数据库唯一约束保证名称不重复，避免并发创建导致的竞态条件。
    ///
    /// # 错误
    /// - `ServiceError::validation`：名称或颜色格式无效
    /// - `ServiceError::Conflict`：标签名已存在（唯一约束冲突）
    ///
    /// # 参数
    /// - `req`：创建标签的请求参数
    pub async fn create_tag(&self, req: CreateTagRequest) -> Result<TagResponse, ServiceError> {
        let name = validate_tag_name(&req.name)?;

        // Trim + validate color
        let color = req.color.as_deref().map(str::trim).map(str::to_owned);
        if let Some(ref color) = color {
            validate_color(color)?;
        }

        // Insert directly and rely on the unique constraint on tags.name — this
        // is race-safe (check-then-insert would allow concurrent duplicates).
        match self.tag_repo.create_tag(&name, color.as_deref()).await {
            Ok(tag) => Ok(tag.into()),
            Err(e) => {
                if db_error::is_unique_violation(&e) {
                    return Err(ServiceError::Conflict("标签已存在".into()));
                }
                Err(ServiceError::Internal(format!("标签操作失败: {}", e)))
            }
        }
    }

    /// 更新现有标签。
    ///
    /// 仅更新请求中提供的字段（部分更新）。如果更新名称，会检查新名称是否已被其他标签占用。
    ///
    /// # 错误
    /// - `ServiceError::NotFound`：指定 ID 的标签不存在
    /// - `ServiceError::validation`：新名称或颜色格式无效
    /// - `ServiceError::Conflict`：新标签名已被其他标签使用
    ///
    /// # 参数
    /// - `id`：要更新的标签 ID
    /// - `req`：更新请求参数（所有字段可选）
    pub async fn update_tag(
        &self,
        id: i32,
        req: UpdateTagRequest,
    ) -> Result<TagResponse, ServiceError> {
        // Check if tag exists
        self.tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?
            .ok_or_else(|| ServiceError::NotFound("标签不存在".into()))?;

        // Validate new name if provided
        let name = match req.name {
            Some(raw) => {
                let normalized = validate_tag_name(&raw)?;

                // Check if the normalized name is taken by another tag
                if let Some(other) = self
                    .tag_repo
                    .find_tag_by_name(&normalized)
                    .await
                    .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?
                {
                    if other.id != id {
                        return Err(ServiceError::Conflict("标签名已存在".into()));
                    }
                }
                Some(normalized)
            }
            None => None,
        };

        // Trim + validate color
        let color = req.color.as_deref().map(str::trim).map(str::to_owned);
        if let Some(ref color) = color {
            validate_color(color)?;
        }

        match self
            .tag_repo
            .update_tag(id, name.as_deref(), color.as_deref())
            .await
        {
            Ok(tag) => Ok(tag.into()),
            Err(e) => {
                if db_error::is_unique_violation(&e) {
                    return Err(ServiceError::Conflict("标签名已存在".into()));
                }
                Err(ServiceError::Internal(format!("标签操作失败: {}", e)))
            }
        }
    }

    /// 删除指定标签。
    ///
    /// 删除前会验证标签是否存在。与该标签关联的视频标签关系会被级联删除。
    ///
    /// # 错误
    /// - `ServiceError::NotFound`：指定 ID 的标签不存在
    ///
    /// # 参数
    /// - `id`：要删除的标签 ID
    pub async fn delete_tag(&self, id: i32) -> Result<(), ServiceError> {
        // Check if tag exists
        let _existing = self
            .tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?
            .ok_or_else(|| ServiceError::NotFound("标签不存在".into()))?;

        self.tag_repo
            .delete_tag(id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;

        Ok(())
    }

    /// 获取指定 ID 的标签详情。
    ///
    /// # 错误
    /// - `ServiceError::NotFound`：指定 ID 的标签不存在
    ///
    /// # 参数
    /// - `id`：标签 ID
    pub async fn get_tag(&self, id: i32) -> Result<TagResponse, ServiceError> {
        let tag = self
            .tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?
            .ok_or_else(|| ServiceError::NotFound("标签不存在".into()))?;

        Ok(tag.into())
    }

    /// 分页获取标签列表。
    ///
    /// # 参数
    /// - `page`：页码（从 0 开始）
    /// - `size`：每页数量
    pub async fn list_tags(&self, page: i64, size: i64) -> Result<Vec<TagResponse>, ServiceError> {
        let tags = self
            .tag_repo
            .list_tags(size, page.saturating_mul(size))
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;

        Ok(tags.into_iter().map(TagResponse::from).collect())
    }

    /// 获取热门标签列表（按使用次数降序排列）。
    ///
    /// # 参数
    /// - `limit`：返回的最大标签数量
    pub async fn get_popular_tags(&self, limit: i64) -> Result<Vec<TagResponse>, ServiceError> {
        let tags = self
            .tag_repo
            .get_popular_tags(limit)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;

        Ok(tags.into_iter().map(TagResponse::from).collect())
    }

    /// 批量为视频添加标签。
    ///
    /// 会自动去重并验证所有标签 ID 是否存在。非管理员用户只能为自己上传的视频添加标签。
    ///
    /// # 错误
    /// - `ServiceError::validation`：标签数量超过上限（`MAX_TAGS_PER_VIDEO`）
    /// - `ServiceError::Forbidden`：非视频所有者尝试操作
    /// - `ServiceError::NotFound`：视频或某个标签 ID 不存在
    ///
    /// # 参数
    /// - `video_id`：目标视频 ID
    /// - `tag_ids`：要添加的标签 ID 列表
    /// - `user_id`：当前操作用户 ID
    /// - `is_admin`：当前用户是否为管理员
    pub async fn add_tags_to_video(
        &self,
        video_id: i64,
        tag_ids: &[i32],
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        let ids = dedupe_tag_ids(tag_ids);
        if ids.is_empty() {
            return Ok(());
        }
        if ids.len() > MAX_TAGS_PER_VIDEO {
            return Err(ServiceError::validation(format!(
                "单次最多添加{}个标签",
                MAX_TAGS_PER_VIDEO
            )));
        }

        // 视频所有权检查：只有视频所有者或管理员才能添加标签
        if !is_admin {
            self.check_video_ownership(video_id, user_id).await?;
        }

        // Batch verify tags exist
        let existing_tags = self
            .tag_repo
            .find_tags_by_ids(&ids)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;

        let existing_ids: HashSet<i32> = existing_tags.iter().map(|t| t.id).collect();

        for &tag_id in &ids {
            if !existing_ids.contains(&tag_id) {
                return Err(ServiceError::NotFound(format!("标签ID {} 不存在", tag_id)));
            }
        }

        self.tag_repo
            .add_tags_to_video_batch(video_id, &ids)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;

        Ok(())
    }

    /// 批量从视频移除标签。
    ///
    /// 非管理员用户只能从自己上传的视频移除标签。
    ///
    /// # 错误
    /// - `ServiceError::Forbidden`：非视频所有者尝试操作
    ///
    /// # 参数
    /// - `video_id`：目标视频 ID
    /// - `tag_ids`：要移除的标签 ID 列表
    /// - `user_id`：当前操作用户 ID
    /// - `is_admin`：当前用户是否为管理员
    pub async fn remove_tags_from_video(
        &self,
        video_id: i64,
        tag_ids: &[i32],
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        let ids = dedupe_tag_ids(tag_ids);
        if ids.is_empty() {
            return Ok(());
        }
        if !is_admin {
            self.check_video_ownership(video_id, user_id).await?;
        }
        self.tag_repo
            .remove_tags_from_video_batch(video_id, &ids)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;
        Ok(())
    }

    /// 从视频移除单个标签。
    ///
    /// 非管理员用户只能从自己上传的视频移除标签。
    ///
    /// # 错误
    /// - `ServiceError::Forbidden`：非视频所有者尝试操作
    ///
    /// # 参数
    /// - `video_id`：目标视频 ID
    /// - `tag_id`：要移除的标签 ID
    /// - `user_id`：当前操作用户 ID
    /// - `is_admin`：当前用户是否为管理员
    pub async fn remove_tag_from_video(
        &self,
        video_id: i64,
        tag_id: i32,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        if !is_admin {
            self.check_video_ownership(video_id, user_id).await?;
        }
        self.tag_repo
            .remove_tag_from_video(video_id, tag_id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;
        Ok(())
    }

    /// 获取指定视频的所有标签。
    ///
    /// # 参数
    /// - `video_id`：视频 ID
    pub async fn get_video_tags(&self, video_id: i64) -> Result<Vec<TagResponse>, ServiceError> {
        let tags = self
            .tag_repo
            .get_video_tags(video_id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?;

        Ok(tags.into_iter().map(TagResponse::from).collect())
    }

    async fn check_video_ownership(&self, video_id: i64, user_id: i64) -> Result<(), ServiceError> {
        let video = self
            .video_repo
            .find_by_id(video_id)
            .await
            .map_err(|e| ServiceError::Internal(format!("标签操作失败: {}", e)))?
            .ok_or_else(|| ServiceError::NotFound("视频不存在".into()))?;
        if video.uploader_id != Some(user_id) {
            return Err(ServiceError::Forbidden("无权操作".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_tag_name() {
        assert_eq!(normalize_tag_name("rust"), "rust");
        assert_eq!(normalize_tag_name("  Rust  "), "Rust");
        assert_eq!(normalize_tag_name("  Rust   Lang "), "Rust Lang");
        assert_eq!(normalize_tag_name("\tC++\n"), "C++");
        assert_eq!(normalize_tag_name("   "), "");
    }

    #[test]
    fn test_color_validation() {
        for color in ["#FF5733", "#00FF00", "#000000", "#ffffff", "#ABC123"] {
            assert!(validate_color(color).is_ok(), "{} should be valid", color);
        }
        for color in [
            "FF5733", "#FFF", "#12345", "#GGGGGG", "#1234567", "#12345g", "",
        ] {
            assert!(
                validate_color(color).is_err(),
                "{} should be invalid",
                color
            );
        }
    }

    #[test]
    fn test_validate_tag_name() {
        assert_eq!(validate_tag_name("rust").unwrap(), "rust");
        assert_eq!(validate_tag_name("  Rust  ").unwrap(), "Rust");
        assert!(validate_tag_name("").is_err());
        assert!(validate_tag_name("   ").is_err());
        assert!(validate_tag_name(&"a".repeat(101)).is_err());
        assert!(validate_tag_name(&"a".repeat(100)).is_ok());
        assert!(validate_tag_name("bad\u{0001}name").is_err());
    }

    #[test]
    fn test_dedupe_tag_ids() {
        assert_eq!(dedupe_tag_ids(&[]), Vec::<i32>::new());
        assert_eq!(dedupe_tag_ids(&[3, 1, 3, 2, 1]), vec![1, 2, 3]);
        assert_eq!(dedupe_tag_ids(&[5]), vec![5]);
    }
}
