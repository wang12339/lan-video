use serde::{Deserialize, Serialize};

use crate::repositories::video_repo::VideoRepository;
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

#[derive(sqlx::FromRow)]
struct RecommendationRow {
    id: i64,
    title: String,
    category: Option<String>,
    thumb_url: Option<String>,
}

#[derive(sqlx::FromRow)]
struct TrendingRow {
    id: i64,
    title: String,
    category: Option<String>,
    thumb_url: Option<String>,
    trending_score: f64,
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
    ) -> Result<Vec<VideoRecommendation>, ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);
        let pool = self.video_repo.pool();

        // Get user's watched categories (non-NULL only — NULL categories would
        // match nothing in `v.category = ANY($2)` and desync the SQL ORDER BY
        // from the Rust-side score computation).
        let watched_categories = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT DISTINCT v.category
            FROM videos v
            INNER JOIN playback_history ph ON v.id = ph.video_id
            WHERE ph.username = $1 AND ph.video_id != $2 AND v.category IS NOT NULL
            LIMIT 10
            "#,
        )
        .bind(username)
        .bind(exclude_video_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ServiceError::internal(format!("获取观看历史失败: {}", e)))?;

        let watched_categories: Vec<String> = watched_categories.into_iter().flatten().collect();

        if watched_categories.is_empty() {
            let (items, _) = self.get_trending_videos(0, limit).await?;
            return Ok(items);
        }

        // Get recommendations based on watched categories.
        // Videos the user has already watched are excluded to avoid
        // re-recommending consumed content.
        //
        // Two-phase fetch so the planner never has to sort the whole table
        // (the old single query's ORDER BY was a non-sargable CASE expression,
        // forcing a Seq Scan + top-N heapsort of every video):
        //   1. Preferred-category videos first, most-viewed first — the
        //      `category = ANY($2)` filter is selective and hits
        //      idx_videos_category_views_id (BitmapOr per category branch).
        //   2. Top up with the hottest remaining videos (views DESC) using
        //      idx_videos_views_id when the preferred batch is too small.
        //
        // Ordering semantics are preserved: preferred videos always outrank
        // the rest (their score is at least 2.0, everything else at most 1.5),
        // and popularity within each group tracks views.
        let preferred_rows = sqlx::query_as::<_, RecommendationRow>(
            r#"
            SELECT
                v.id,
                v.title,
                v.category,
                v.thumb_url
            FROM videos v
            WHERE v.id != $1
              AND v.category = ANY($2)
              AND v.source_type = 'local_video'
              AND NOT EXISTS (
                  SELECT 1 FROM playback_history ph
                  WHERE ph.username = $3 AND ph.video_id = v.id
              )
            ORDER BY v.views DESC, v.id DESC
            LIMIT $4
            "#,
        )
        .bind(exclude_video_id)
        .bind(&watched_categories)
        .bind(username)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| ServiceError::internal(format!("获取推荐视频失败: {}", e)))?;

        // Cold/edge case: user has watched everything in their categories.
        // Fall back to trending so the feed is never empty.
        if preferred_rows.is_empty() {
            let (items, _) = self.get_trending_videos(0, limit).await?;
            return Ok(items);
        }

        let preferred_ids: Vec<i64> = preferred_rows.iter().map(|r| r.id).collect();

        let mut rows: Vec<RecommendationRow> = preferred_rows;
        let remaining = limit - rows.len() as i64;
        if remaining > 0 {
            let fill_rows = sqlx::query_as::<_, RecommendationRow>(
                r#"
                SELECT
                    v.id,
                    v.title,
                    v.category,
                    v.thumb_url
                FROM videos v
                WHERE v.id != $1
                  AND NOT (v.category = ANY($2))
                  AND NOT (v.id = ANY($3))
                  AND v.source_type = 'local_video'
                  AND NOT EXISTS (
                      SELECT 1 FROM playback_history ph
                      WHERE ph.username = $4 AND ph.video_id = v.id
                  )
                ORDER BY v.views DESC, v.id DESC
                LIMIT $5
                "#,
            )
            .bind(exclude_video_id)
            .bind(&watched_categories)
            .bind(&preferred_ids)
            .bind(username)
            .bind(remaining)
            .fetch_all(pool)
            .await
            .map_err(|e| ServiceError::internal(format!("获取推荐视频失败: {}", e)))?;
            rows.extend(fill_rows);
        }

        let recommendations = rows
            .into_iter()
            .map(|r| {
                let is_preferred = r
                    .category
                    .as_ref()
                    .is_some_and(|c| watched_categories.contains(c));
                let category_score = if is_preferred { 2.0 } else { 1.0 };
                let reason = if is_preferred {
                    "基于你的观看偏好"
                } else {
                    "热门推荐"
                };

                VideoRecommendation {
                    id: r.id,
                    title: r.title,
                    category: r.category,
                    thumb_url: r.thumb_url,
                    score: category_score * 1.0,
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
    ) -> Result<Vec<VideoRecommendation>, ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);
        let pool = self.video_repo.pool();

        let video =
            sqlx::query_scalar::<_, Option<String>>("SELECT category FROM videos WHERE id = $1")
                .bind(video_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ServiceError::internal(format!("获取视频信息失败: {}", e)))?
                .ok_or_else(|| ServiceError::NotFound("视频不存在".into()))?;

        let category = video;

        let rows = sqlx::query_as::<_, RecommendationRow>(
            r#"
            SELECT id, title, category, thumb_url
            FROM videos
            WHERE id != $1 AND ($2::varchar IS NULL OR category = $2)
            ORDER BY views DESC
            LIMIT $3
            "#,
        )
        .bind(video_id)
        .bind(&category)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| ServiceError::internal(format!("获取相似视频失败: {}", e)))?;

        let recommendations = rows
            .into_iter()
            .map(|r| VideoRecommendation {
                id: r.id,
                title: r.title,
                category: r.category,
                thumb_url: r.thumb_url,
                score: if category.is_some() { 1.5 } else { 1.0 },
                reason: "相似视频",
            })
            .collect();

        Ok(recommendations)
    }

    pub async fn get_trending_videos(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<VideoRecommendation>, i64), ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);
        let pool = self.video_repo.pool();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM videos WHERE trending_score > 0 AND source_type = 'local_video'",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ServiceError::internal(format!("获取热门视频总数失败: {}", e)))?;

        let rows = sqlx::query_as::<_, TrendingRow>(
            r#"
            SELECT id, title, category, thumb_url, trending_score
            FROM videos
            WHERE trending_score > 0
              AND source_type = 'local_video'
            ORDER BY trending_score DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
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
        offset: i64,
        limit: i64,
    ) -> Result<(Vec<VideoRecommendation>, i64), ServiceError> {
        let limit = limit.clamp(1, MAX_RECOMMENDATION_LIMIT);
        let pool = self.video_repo.pool();

        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM videos WHERE source_type = 'local_video'")
                .fetch_one(pool)
                .await
                .map_err(|e| ServiceError::internal(format!("获取最新视频总数失败: {}", e)))?;

        let rows = sqlx::query_as::<_, RecommendationRow>(
            r#"
            SELECT id, title, category, thumb_url
            FROM videos
            WHERE source_type = 'local_video'
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
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
