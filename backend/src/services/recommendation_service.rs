use serde::{Deserialize, Serialize};

use crate::repositories::recommendation_repo::{RecommendationRepository, RecommendationRow};
use crate::util::error::ServiceError;

const MAX_RECOMMENDATION_LIMIT: i64 = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRecommendation {
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
    pub score: f64,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct RecommendationService {
    repo: RecommendationRepository,
}

impl RecommendationService {
    pub fn new(repo: RecommendationRepository) -> Self {
        Self { repo }
    }

    /// Personalised recommendations: prefer what the user has watched, then top
    /// up with popular content, and never return an empty feed.
    pub async fn get_recommendations(
        &self,
        username: &str,
        owner_id: Option<i64>,
        exclude_video_id: i64,
        limit: i64,
    ) -> Result<Vec<VideoRecommendation>, ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);

        // NULL categories are dropped: `category = ANY($2)` never matches NULL,
        // so keeping them would desync the SQL ordering from the scoring below.
        let watched_categories: Vec<String> = self
            .repo
            .watched_categories(username, exclude_video_id, owner_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取观看历史失败: {}", e)))?
            .into_iter()
            .flatten()
            .collect();

        // Cold start (nothing watched): there is no signal to personalise on.
        if watched_categories.is_empty() {
            let (items, _) = self.get_trending_videos(owner_id, 0, limit).await?;
            return Ok(items);
        }

        let preferred_rows = self
            .repo
            .preferred_videos(
                exclude_video_id,
                &watched_categories,
                username,
                limit,
                owner_id,
            )
            .await
            .map_err(|e| ServiceError::internal(format!("获取推荐视频失败: {}", e)))?;

        // The user has watched everything in their categories: fall back to
        // trending so the feed is never empty.
        if preferred_rows.is_empty() {
            let (items, _) = self.get_trending_videos(owner_id, 0, limit).await?;
            return Ok(items);
        }

        let preferred_ids: Vec<i64> = preferred_rows.iter().map(|r| r.id).collect();
        let mut rows: Vec<RecommendationRow> = preferred_rows;
        let remaining = limit - rows.len() as i64;
        if remaining > 0 {
            let fill_rows = self
                .repo
                .fill_videos(
                    exclude_video_id,
                    &watched_categories,
                    &preferred_ids,
                    username,
                    remaining,
                    owner_id,
                )
                .await
                .map_err(|e| ServiceError::internal(format!("获取推荐视频失败: {}", e)))?;
            rows.extend(fill_rows);
        }

        // Ordering semantics: preferred videos always outrank the rest (2.0 vs
        // 1.0), and popularity within each group already tracks views because
        // both queries sort by `views DESC, id DESC`.
        Ok(rows
            .into_iter()
            .map(|r| {
                let is_preferred = r
                    .category
                    .as_ref()
                    .is_some_and(|c| watched_categories.contains(c));
                VideoRecommendation {
                    id: r.id,
                    title: r.title,
                    category: r.category,
                    thumb_url: r.thumb_url,
                    score: if is_preferred { 2.0 } else { 1.0 },
                    reason: if is_preferred {
                        "基于你的观看偏好"
                    } else {
                        "热门推荐"
                    },
                }
            })
            .collect())
    }

    /// Other videos in the same category as `video_id`.
    ///
    /// A video without a category degrades to "most popular other videos"
    /// rather than returning nothing.
    pub async fn get_similar_videos(
        &self,
        owner_id: Option<i64>,
        video_id: i64,
        limit: i64,
    ) -> Result<Vec<VideoRecommendation>, ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);

        let category = self
            .repo
            .video_category(video_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取视频信息失败: {}", e)))?
            .ok_or_else(|| ServiceError::NotFound("视频不存在".into()))?;

        let rows = self
            .repo
            .similar_videos(video_id, category.as_deref(), limit, owner_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取相似视频失败: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.id,
                title: r.title,
                category: r.category,
                thumb_url: r.thumb_url,
                score: if category.is_some() { 1.5 } else { 1.0 },
                reason: "相似视频",
            })
            .collect())
    }

    pub async fn get_trending_videos(
        &self,
        owner_id: Option<i64>,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<VideoRecommendation>, i64), ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);

        let total = self
            .repo
            .trending_count(owner_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取热门视频总数失败: {}", e)))?;

        let rows = self
            .repo
            .trending_videos(limit, offset, owner_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取热门视频失败: {}", e)))?;

        let recommendations = rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.id,
                title: r.title,
                category: r.category,
                thumb_url: r.thumb_url,
                score: r.trending_score,
                reason: "热门推荐",
            })
            .collect();

        Ok((recommendations, total))
    }

    pub async fn get_recent_videos(
        &self,
        owner_id: Option<i64>,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<VideoRecommendation>, i64), ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);

        let total = self
            .repo
            .recent_count(owner_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取最新视频总数失败: {}", e)))?;

        let rows = self
            .repo
            .recent_videos(limit, offset, owner_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取最新视频失败: {}", e)))?;

        let recommendations = rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.id,
                title: r.title,
                category: r.category,
                thumb_url: r.thumb_url,
                score: 1.0,
                reason: "最新上传",
            })
            .collect();

        Ok((recommendations, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommendation_creation() {
        let rec = VideoRecommendation {
            id: 1,
            title: "Test Video".to_string(),
            category: Some("test".to_string()),
            thumb_url: Some("/thumb.jpg".to_string()),
            score: 1.5,
            reason: "相似视频",
        };

        assert_eq!(rec.id, 1);
        assert_eq!(rec.title, "Test Video");
        assert_eq!(rec.score, 1.5);
    }
}
