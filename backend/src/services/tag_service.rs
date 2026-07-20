use serde::{Deserialize, Serialize};

use crate::repositories::tag_repo::TagRepository;

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
}

impl TagService {
    pub fn new(tag_repo: TagRepository) -> Self {
        Self { tag_repo }
    }

    pub async fn create_tag(&self, req: CreateTagRequest) -> Result<TagResponse, String> {
        // Validate tag name
        let name = req.name.trim().to_string();
        if name.is_empty() || name.len() > 100 {
            return Err("标签名长度必须在1-100之间".to_string());
        }

        // Check if tag already exists
        if let Ok(Some(_)) = self.tag_repo.find_tag_by_name(&name).await {
            return Err("标签已存在".to_string());
        }

        // Validate color format
        if let Some(ref color) = req.color {
            if !color.starts_with('#') || color.len() != 7 {
                return Err("颜色格式无效，应为#RRGGBB".to_string());
            }
        }

        let tag = self
            .tag_repo
            .create_tag(&name, req.color.as_deref())
            .await
            .map_err(|e| format!("创建标签失败: {}", e))?;

        Ok(tag.into())
    }

    pub async fn update_tag(&self, id: i32, req: UpdateTagRequest) -> Result<TagResponse, String> {
        // Check if tag exists
        let _existing = self
            .tag_repo
            .find_tag_by_id(id)
            .await
            .map_err(|e| format!("查询标签失败: {}", e))?
            .ok_or_else(|| "标签不存在".to_string())?;

        // Validate new name if provided
        if let Some(ref name) = req.name {
            let name = name.trim().to_string();
            if name.is_empty() || name.len() > 100 {
                return Err("标签名长度必须在1-100之间".to_string());
            }

            // Check if new name already exists (excluding current tag)
            if let Ok(Some(other)) = self.tag_repo.find_tag_by_name(&name).await {
                if other.id != id {
                    return Err("标签名已存在".to_string());
                }
            }
        }

        // Validate color format
        if let Some(ref color) = req.color {
            if !color.starts_with('#') || color.len() != 7 {
                return Err("颜色格式无效，应为#RRGGBB".to_string());
            }
        }

        let tag = self
            .tag_repo
            .update_tag(id, req.name.as_deref(), req.color.as_deref())
            .await
            .map_err(|e| format!("更新标签失败: {}", e))?;

        Ok(tag.into())
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
            .list_tags(size, page * size)
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

    pub async fn add_tags_to_video(&self, video_id: i64, tag_ids: &[i32]) -> Result<(), String> {
        // Batch verify tags exist
        let existing_tags = self
            .tag_repo
            .find_tags_by_ids(tag_ids)
            .await
            .map_err(|e| format!("查询标签失败: {}", e))?;

        let existing_ids: std::collections::HashSet<i32> =
            existing_tags.iter().map(|t| t.id).collect();

        for &tag_id in tag_ids {
            if !existing_ids.contains(&tag_id) {
                return Err(format!("标签ID {} 不存在", tag_id));
            }

            self.tag_repo
                .add_tag_to_video(video_id, tag_id)
                .await
                .map_err(|e| format!("添加标签到视频失败: {}", e))?;
        }

        Ok(())
    }

    pub async fn remove_tags_from_video(
        &self,
        video_id: i64,
        tag_ids: &[i32],
    ) -> Result<(), String> {
        for &tag_id in tag_ids {
            self.remove_tag_from_video(video_id, tag_id).await?;
        }
        Ok(())
    }

    pub async fn remove_tag_from_video(&self, video_id: i64, tag_id: i32) -> Result<(), String> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_request_validation() {
        let req = CreateTagRequest {
            name: "test".to_string(),
            color: Some("#FF5733".to_string()),
        };

        assert!(!req.name.is_empty());
        assert!(req.name.len() <= 100);
    }

    #[test]
    fn test_color_validation() {
        let valid_colors = vec!["#FF5733", "#00FF00", "#000000"];
        let invalid_colors = vec!["FF5733", "#FFF", "#12345"];

        for color in valid_colors {
            assert!(
                color.starts_with('#') && color.len() == 7,
                "Color {} should be valid",
                color
            );
        }

        for color in invalid_colors {
            let is_valid = color.starts_with('#') && color.len() == 7;
            assert!(!is_valid, "Color {} should be invalid", color);
        }
    }
}
