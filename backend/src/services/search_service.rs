use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::repositories::video_repo::VideoRepository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub video_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rank: f32,
    pub headline: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchService {
    video_repo: VideoRepository,
}

impl SearchService {
    pub fn new(video_repo: VideoRepository) -> Self {
        Self { video_repo }
    }

    pub async fn full_text_search(
        &self,
        query: &str,
        page: i64,
        size: i64,
    ) -> Result<(Vec<SearchResult>, i64), String> {
        let pool = self.video_repo.pool();

        // SECURITY: we use the built-in 'simple' text-search configuration
        // rather than 'chinese' because the zhparser / pg_jieba extension
        // is not installed on standard PostgreSQL installs. 'simple' does
        // not tokenize Chinese, but it works for the Latin-script portion
        // of titles and avoids a hard 500 on every search.
        let rows = sqlx::query(
            r#"
            SELECT
                id as video_id,
                title,
                description,
                category,
                ts_rank(search_vector, plainto_tsquery('simple', $1)) as rank,
                ts_headline('simple', title, plainto_tsquery('simple', $1),
                    'StartSel=<mark>, StopSel=</mark>, MaxWords=50, MinWords=20') as headline,
                COUNT(*) OVER() AS total
            FROM videos
            WHERE search_vector @@ plainto_tsquery('simple', $1)
            ORDER BY rank DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(query)
        .bind(size)
        .bind(page * size)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("搜索失败: {}", e))?;

        let total: i64 = rows.first().map(|r| r.get("total")).unwrap_or(0);
        let results = rows
            .into_iter()
            .map(|r| {
                // SECURITY (XSS-001): ts_headline can return a string
                // containing `<mark>...</mark>` markers. The webapp currently
                // does not render this, and React would auto-escape it
                // anyway, but stripping the markers here keeps the API
                // surface unambiguous for any future consumer.
                let headline: Option<String> = r
                    .get::<Option<String>, _>("headline")
                    .map(|s| strip_ts_headline_markers(&s));
                SearchResult {
                    video_id: r.get("video_id"),
                    title: r.get("title"),
                    description: r.get("description"),
                    category: r.get("category"),
                    rank: r.get("rank"),
                    headline,
                }
            })
            .collect();

        Ok((results, total))
    }

    pub async fn search_suggest(&self, query: &str, limit: i64) -> Result<Vec<String>, String> {
        let pool = self.video_repo.pool();

        // SECURITY: we use 'simple' (not 'chinese') because the zhparser /
        // pg_jieba extension is not installed on standard PostgreSQL.
        // Group by title to dedupe; use MAX(rank) to pick the best match.
        let rows = sqlx::query_scalar(
            r#"
            SELECT title
            FROM (
                SELECT title,
                       ts_rank(search_vector, plainto_tsquery('simple', $1)) AS rk
                FROM videos
                WHERE search_vector @@ plainto_tsquery('simple', $1)
            ) AS t
            GROUP BY title
            ORDER BY MAX(rk) DESC
            LIMIT $2
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("搜索建议失败: {}", e))?;

        Ok(rows)
    }
}

/// Strip the `<mark>...</mark>` start/stop selectors from a ts_headline result.
/// We just remove the literal substrings; the resulting text is plain.
fn strip_ts_headline_markers(s: &str) -> String {
    s.replace("<mark>", "").replace("</mark>", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ts_headline_markers_removes_mark() {
        assert_eq!(
            strip_ts_headline_markers("matched <mark>keyword</mark> in text"),
            "matched keyword in text"
        );
    }

    #[test]
    fn test_strip_ts_headline_markers_no_markers() {
        assert_eq!(
            strip_ts_headline_markers("plain text without markers"),
            "plain text without markers"
        );
    }

    #[test]
    fn test_strip_ts_headline_markers_multiple() {
        assert_eq!(
            strip_ts_headline_markers("<mark>first</mark> and <mark>second</mark>"),
            "first and second"
        );
    }

    #[test]
    fn test_strip_ts_headline_markers_empty() {
        assert_eq!(strip_ts_headline_markers(""), "");
    }

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult {
            video_id: 1,
            title: "Test Video".to_string(),
            description: Some("Test description".to_string()),
            category: Some("test".to_string()),
            rank: 0.5,
            headline: Some("Test headline".to_string()),
        };

        assert_eq!(result.video_id, 1);
        assert_eq!(result.title, "Test Video");
    }
}
