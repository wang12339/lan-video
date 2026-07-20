use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::repositories::video_repo::VideoRepository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRecommendation {
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
    pub score: f32,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RecommendationService {
    video_repo: VideoRepository,
}

impl RecommendationService {
    pub fn new(video_repo: VideoRepository) -> Self {
        Self { video_repo }
    }

    pub async fn get_recommendations(
        &self,
        username: &str,
        exclude_video_id: i64,
        limit: i64,
    ) -> Result<Vec<VideoRecommendation>, String> {
        let pool = self.video_repo.pool();

        // Get user's watched categories
        let watched_categories = sqlx::query_scalar(
            r#"
            SELECT DISTINCT v.category
            FROM videos v
            INNER JOIN playback_history ph ON v.id = ph.video_id
            WHERE ph.username = $1 AND ph.video_id != $2
            LIMIT 10
            "#,
        )
        .bind(username)
        .bind(exclude_video_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("获取观看历史失败: {}", e))?;

        if watched_categories.is_empty() {
            return self.get_trending_videos(limit).await;
        }

        // Get recommendations based on watched categories
        let rows = sqlx::query(
            r#"
            SELECT 
                v.id,
                v.title,
                v.category,
                v.thumb_url,
                v.views,
                v.duration,
                CASE 
                    WHEN v.category = ANY($1) THEN 2.0::float4
                    ELSE 1.0::float4
                END as category_score,
                CASE 
                    WHEN v.views > 1000 THEN 1.5::float4
                    WHEN v.views > 100 THEN 1.2::float4
                    ELSE 1.0::float4
                END as popularity_score
            FROM videos v
            WHERE v.id != $2
            ORDER BY (category_score * popularity_score) DESC
            LIMIT $3
            "#,
        )
        .bind(&watched_categories)
        .bind(exclude_video_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("获取推荐视频失败: {}", e))?;

        let recommendations = rows
            .into_iter()
            .map(|r| {
                let category: Option<String> = r.get("category");
                let score: f32 = r.get("category_score");
                let reason = if watched_categories.contains(&category) {
                    "基于你的观看偏好".to_string()
                } else {
                    "热门推荐".to_string()
                };

                VideoRecommendation {
                    id: r.get("id"),
                    title: r.get("title"),
                    category,
                    thumb_url: r.get("thumb_url"),
                    score,
                    reason,
                }
            })
            .collect();

        Ok(recommendations)
    }

    pub async fn get_similar_videos(
        &self,
        video_id: i64,
        limit: i64,
    ) -> Result<Vec<VideoRecommendation>, String> {
        let pool = self.video_repo.pool();

        // Get the video's category
        let video = sqlx::query("SELECT category FROM videos WHERE id = $1")
            .bind(video_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("获取视频信息失败: {}", e))?
            .ok_or_else(|| "视频不存在".to_string())?;

        let category: Option<String> = video.get("category");

        let rows = if let Some(ref cat) = category {
            sqlx::query(
                r#"
                SELECT 
                    id,
                    title,
                    category,
                    thumb_url,
                    views,
                    1.5::float4 as score
                FROM videos
                WHERE id != $1 AND category = $2
                ORDER BY views DESC
                LIMIT $3
                "#,
            )
            .bind(video_id)
            .bind(cat)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("获取相似视频失败: {}", e))?
        } else {
            sqlx::query(
                r#"
                SELECT 
                    id,
                    title,
                    category,
                    thumb_url,
                    views,
                    1.0::float4 as score
                FROM videos
                WHERE id != $1
                ORDER BY views DESC
                LIMIT $2
                "#,
            )
            .bind(video_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("获取相似视频失败: {}", e))?
        };

        let recommendations = rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.get("id"),
                title: r.get("title"),
                category: r.get("category"),
                thumb_url: r.get("thumb_url"),
                score: r.get("score"),
                reason: "相似视频".to_string(),
            })
            .collect();

        Ok(recommendations)
    }

    pub async fn get_trending_videos(
        &self,
        limit: i64,
    ) -> Result<Vec<VideoRecommendation>, String> {
        let pool = self.video_repo.pool();

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                title,
                category,
                thumb_url,
                views,
                CASE 
                    WHEN views > 1000 THEN 2.0::float4
                    WHEN views > 100 THEN 1.5::float4
                    ELSE 1.0::float4
                END as score
            FROM videos
            ORDER BY views DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("获取热门视频失败: {}", e))?;

        let recommendations = rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.get("id"),
                title: r.get("title"),
                category: r.get("category"),
                thumb_url: r.get("thumb_url"),
                score: r.get("score"),
                reason: "热门推荐".to_string(),
            })
            .collect();

        Ok(recommendations)
    }

    pub async fn get_recent_videos(&self, limit: i64) -> Result<Vec<VideoRecommendation>, String> {
        let pool = self.video_repo.pool();

        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                title,
                category,
                thumb_url,
                created_at,
                1.0::float4 as score
            FROM videos
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("获取最新视频失败: {}", e))?;

        let recommendations = rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.get("id"),
                title: r.get("title"),
                category: r.get("category"),
                thumb_url: r.get("thumb_url"),
                score: r.get("score"),
                reason: "最新上传".to_string(),
            })
            .collect();

        Ok(recommendations)
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
            reason: "相似视频".to_string(),
        };

        assert_eq!(rec.id, 1);
        assert_eq!(rec.title, "Test Video");
        assert_eq!(rec.score, 1.5);
    }
}
