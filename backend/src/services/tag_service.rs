use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::repositories::tag_repo::TagRepository;
use crate::repositories::video_repo::VideoRepository;

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
fn validate_color(color: &str) -> Result<(), String> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err("颜色格式无效，应为#RRGGBB".to_string())
    }
}

/// True if the SQLx error is a unique-constraint violation (SQLSTATE 23505).
fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.is_unique_violation())
}

/// Validate a tag name: normalized, non-empty, within length limits and free
/// of control characters. Returns the normalized name.
fn validate_tag_name(name: &str) -> Result<String, String> {
    let normalized = normalize_tag_name(name);
    if normalized.is_empty() || normalized.len() > MAX_TAG_NAME_LEN {
        return Err(format!("标签名长度必须在1-{}之间", MAX_TAG_NAME_LEN));
    }
    if normalized.chars().any(|c| c.is_control()) {
        return Err("标签名包含非法控制字符".to_string());
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

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

#[derive(Debug, Clone)]
pub struct TagService {
    tag_repo: TagRepository,
    video_repo: VideoRepository,
}

impl TagService {
    pub fn new(tag_repo: TagRepository, video_repo: VideoRepository) -> Self {
        Self {
            tag_repo,
            video_repo,
        }
    }

    pub async fn create_tag(&self, req: CreateTagRequest) -> Result<TagResponse, String> {
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
                if is_unique_violation(&e) {
                    return Err("标签已存在".to_string());
                }
                Err(format!("创建标签失败: {}", e))
            }
        }
    }

    pub async fn update_tag(&self, id: i32, req: UpdateTagRequest) -> Result<TagResponse, String> {
        // Check if tag exists
        self.tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| format!("查询标签失败: {}", e))?
            .ok_or_else(|| "标签不存在".to_string())?;

        // Validate new name if provided
        let name = match req.name {
            Some(raw) => {
                let normalized = validate_tag_name(&raw)?;

                // Check if the normalized name is taken by another tag
                if let Some(other) = self
                    .tag_repo
                    .find_tag_by_name(&normalized)
                    .await
                    .map_err(|e| format!("查询标签失败: {}", e))?
                {
                    if other.id != id {
                        return Err("标签名已存在".to_string());
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
                if is_unique_violation(&e) {
                    return Err("标签名已存在".to_string());
                }
                Err(format!("更新标签失败: {}", e))
            }
        }
    }

    pub async fn delete_tag(&self, id: i32) -> Result<(), String> {
        // Check if tag exists
        let _existing = self
            .tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| format!("查询标签失败: {}", e))?
            .ok_or_else(|| "标签不存在".to_string())?;

        self.tag_repo
            .delete_tag(id)
            .await
            .map_err(|e| format!("删除标签失败: {}", e))?;

        Ok(())
    }

    pub async fn get_tag(&self, id: i32) -> Result<TagResponse, String> {
        let tag = self
            .tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| format!("查询标签失败: {}", e))?
            .ok_or_else(|| "标签不存在".to_string())?;

        Ok(tag.into())
    }

    pub async fn list_tags(&self, page: i64, size: i64) -> Result<Vec<TagResponse>, String> {
        let tags = self
            .tag_repo
            .list_tags(size, page.saturating_mul(size))
            .await
            .map_err(|e| format!("查询标签列表失败: {}", e))?;

        Ok(tags.into_iter().map(TagResponse::from).collect())
    }

    pub async fn get_popular_tags(&self, limit: i64) -> Result<Vec<TagResponse>, String> {
        let tags = self
            .tag_repo
            .get_popular_tags(limit)
            .await
            .map_err(|e| format!("查询热门标签失败: {}", e))?;

        Ok(tags.into_iter().map(TagResponse::from).collect())
    }

    pub async fn add_tags_to_video(
        &self,
        video_id: i64,
        tag_ids: &[i32],
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), String> {
        let ids = dedupe_tag_ids(tag_ids);
        if ids.is_empty() {
            return Ok(());
        }
        if ids.len() > MAX_TAGS_PER_VIDEO {
            return Err(format!("单次最多添加{}个标签", MAX_TAGS_PER_VIDEO));
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
            .map_err(|e| format!("查询标签失败: {}", e))?;

        let existing_ids: HashSet<i32> = existing_tags.iter().map(|t| t.id).collect();

        for &tag_id in &ids {
            if !existing_ids.contains(&tag_id) {
                return Err(format!("标签ID {} 不存在", tag_id));
            }
        }

        self.tag_repo
            .add_tags_to_video_batch(video_id, &ids)
            .await
            .map_err(|e| format!("添加标签到视频失败: {}", e))?;

        Ok(())
    }

    pub async fn remove_tags_from_video(
        &self,
        video_id: i64,
        tag_ids: &[i32],
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), String> {
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
            .map_err(|e| format!("从视频移除标签失败: {}", e))?;
        Ok(())
    }

    pub async fn remove_tag_from_video(
        &self,
        video_id: i64,
        tag_id: i32,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), String> {
        if !is_admin {
            self.check_video_ownership(video_id, user_id).await?;
        }
        self.tag_repo
            .remove_tag_from_video(video_id, tag_id)
            .await
            .map_err(|e| format!("从视频移除标签失败: {}", e))?;
        Ok(())
    }

    pub async fn get_video_tags(&self, video_id: i64) -> Result<Vec<TagResponse>, String> {
        let tags = self
            .tag_repo
            .get_video_tags(video_id)
            .await
            .map_err(|e| format!("查询视频标签失败: {}", e))?;

        Ok(tags.into_iter().map(TagResponse::from).collect())
    }
    async fn check_video_ownership(&self, video_id: i64, user_id: i64) -> Result<(), String> {
        let video = self
            .video_repo
            .find_by_id(video_id)
            .await
            .map_err(|e| format!("查询视频失败: {}", e))?
            .ok_or_else(|| "视频不存在".to_string())?;
        if video.uploader_id != Some(user_id) {
            return Err("无权操作".to_string());
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
