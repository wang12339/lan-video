//! Recommendation / trending / recent feed SQL for the `videos` table.
//!
//! Lives in a repository rather than in `RecommendationService` for the same
//! reason as `search_repo`: the service used to reach through
//! `VideoRepository::pool()` and hand-write every statement, so this SQL escaped
//! the repository's slow-query instrumentation, index bookkeeping and future
//! optimisation work.
//!
//! The service keeps the *policy* — limit clamping, the preferred-then-fill
//! two-phase strategy, score/reason assignment, fallbacks. This module keeps the
//! *SQL*, and each method documents the index it is written to hit.

use crate::db::log_slow_query;
use sqlx::PgPool;

/// A video as it appears in a feed: just enough to render a card.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecommendationRow {
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
}

/// A trending candidate, carrying the precomputed `trending_score`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TrendingRow {
    pub id: i64,
    pub title: String,
    pub category: Option<String>,
    pub thumb_url: Option<String>,
    pub trending_score: f64,
}

#[derive(Debug, Clone)]
pub struct RecommendationRepository {
    pool: PgPool,
}

impl RecommendationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Distinct categories the user has watched, excluding one video.
    ///
    /// `NULL` categories are kept in the result shape but filtered by the
    /// caller: `v.category = ANY($2)` would never match `NULL`, so including
    /// them would desync the SQL `ORDER BY` from the Rust-side scoring.
    pub async fn watched_categories(
        &self,
        username: &str,
        exclude_video_id: i64,
        owner_id: Option<i64>,
    ) -> Result<Vec<Option<String>>, sqlx::Error> {
        log_slow_query("recommendation_repo::watched_categories", || async {
            sqlx::query_scalar::<_, Option<String>>(
                r#"
                SELECT DISTINCT v.category
                FROM videos v
                INNER JOIN playback_history ph ON v.id = ph.video_id
                WHERE ph.username = $1 AND ph.video_id != $2 AND v.category IS NOT NULL
                  AND ($3::bigint IS NULL OR v.uploader_id = $3)
                LIMIT 10
                "#,
            )
            .bind(username)
            .bind(exclude_video_id)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// Videos in the user's watched categories, most-viewed first.
    ///
    /// Already-watched videos are excluded. The `category = ANY($2)` filter is
    /// selective and uses `idx_videos_category_views_id` (a BitmapOr over the
    /// per-category branches).
    pub async fn preferred_videos(
        &self,
        exclude_video_id: i64,
        watched_categories: &[String],
        username: &str,
        limit: i64,
        owner_id: Option<i64>,
    ) -> Result<Vec<RecommendationRow>, sqlx::Error> {
        log_slow_query("recommendation_repo::preferred_videos", || async {
            sqlx::query_as::<_, RecommendationRow>(
                r#"
                SELECT
                    v.id,
                    v.title,
                    v.category,
                    v.thumb_url
                FROM videos v
                WHERE v.id != $1
                  AND ($5::bigint IS NULL OR v.uploader_id = $5)
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
            .bind(watched_categories)
            .bind(username)
            .bind(limit)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// Hottest remaining videos, used to top up a short preferred batch.
    ///
    /// This is deliberately a *second* query rather than a `UNION` in the
    /// preferred one: a single combined query needs a non-sargable `CASE` in its
    /// `ORDER BY` to keep preferred rows on top, which forces a Seq Scan plus a
    /// top-N heapsort over every video. Two sargable queries let each half use
    /// its own index (`idx_videos_category_views_id` and `idx_videos_views_id`)
    /// while preserving the same ordering.
    pub async fn fill_videos(
        &self,
        exclude_video_id: i64,
        watched_categories: &[String],
        preferred_ids: &[i64],
        username: &str,
        remaining: i64,
        owner_id: Option<i64>,
    ) -> Result<Vec<RecommendationRow>, sqlx::Error> {
        log_slow_query("recommendation_repo::fill_videos", || async {
            sqlx::query_as::<_, RecommendationRow>(
                r#"
                SELECT
                    v.id,
                    v.title,
                    v.category,
                    v.thumb_url
                FROM videos v
                WHERE v.id != $1
                  AND ($6::bigint IS NULL OR v.uploader_id = $6)
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
            .bind(watched_categories)
            .bind(preferred_ids)
            .bind(username)
            .bind(remaining)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// Category of a single video, or `None` if the video does not exist.
    ///
    /// The double `Option` distinguishes "no such video" from "video with no
    /// category", which the caller needs: the first is a 404, the second simply
    /// means "no category filter".
    pub async fn video_category(
        &self,
        video_id: i64,
    ) -> Result<Option<Option<String>>, sqlx::Error> {
        log_slow_query("recommendation_repo::video_category", || async {
            sqlx::query_scalar::<_, Option<String>>("SELECT category FROM videos WHERE id = $1")
                .bind(video_id)
                .fetch_optional(&self.pool)
                .await
        })
        .await
    }

    /// Other videos sharing a category (or, with no category, simply popular).
    pub async fn similar_videos(
        &self,
        video_id: i64,
        category: Option<&str>,
        limit: i64,
        owner_id: Option<i64>,
    ) -> Result<Vec<RecommendationRow>, sqlx::Error> {
        log_slow_query("recommendation_repo::similar_videos", || async {
            sqlx::query_as::<_, RecommendationRow>(
                r#"
                SELECT id, title, category, thumb_url
                FROM videos
                WHERE id != $1 AND ($4::bigint IS NULL OR uploader_id = $4)
                  AND ($2::varchar IS NULL OR category = $2)
                ORDER BY views DESC
                LIMIT $3
                "#,
            )
            .bind(video_id)
            .bind(category)
            .bind(limit)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// Total number of trending videos (for pagination).
    pub async fn trending_count(&self, owner_id: Option<i64>) -> Result<i64, sqlx::Error> {
        log_slow_query("recommendation_repo::trending_count", || async {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM videos WHERE ($1::bigint IS NULL OR uploader_id = $1) AND trending_score > 0 AND source_type = 'local_video'",
            )
            .bind(owner_id)
            .fetch_one(&self.pool)
            .await
        })
        .await
    }

    /// Trending page, ordered by the precomputed decaying score.
    ///
    /// `trending_score` is recomputed every 10 minutes by the background task in
    /// `app.rs` (migration 037 + 059), so this stays an index-friendly sort
    /// rather than recomputing popularity per request.
    pub async fn trending_videos(
        &self,
        limit: i64,
        offset: i64,
        owner_id: Option<i64>,
    ) -> Result<Vec<TrendingRow>, sqlx::Error> {
        log_slow_query("recommendation_repo::trending_videos", || async {
            sqlx::query_as::<_, TrendingRow>(
                r#"
                SELECT id, title, category, thumb_url, trending_score
                FROM videos
                WHERE ($3::bigint IS NULL OR uploader_id = $3)
                  AND trending_score > 0
                  AND source_type = 'local_video'
                ORDER BY trending_score DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// Total number of local videos (for pagination).
    pub async fn recent_count(&self, owner_id: Option<i64>) -> Result<i64, sqlx::Error> {
        log_slow_query("recommendation_repo::recent_count", || async {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM videos WHERE ($1::bigint IS NULL OR uploader_id = $1) AND source_type = 'local_video'",
            )
            .bind(owner_id)
            .fetch_one(&self.pool)
            .await
        })
        .await
    }

    /// Most recently uploaded local videos.
    pub async fn recent_videos(
        &self,
        limit: i64,
        offset: i64,
        owner_id: Option<i64>,
    ) -> Result<Vec<RecommendationRow>, sqlx::Error> {
        log_slow_query("recommendation_repo::recent_videos", || async {
            sqlx::query_as::<_, RecommendationRow>(
                r#"
                SELECT id, title, category, thumb_url
                FROM videos
                WHERE ($3::bigint IS NULL OR uploader_id = $3) AND source_type = 'local_video'
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }
}
