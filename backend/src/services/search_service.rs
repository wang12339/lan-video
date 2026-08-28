use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

use crate::repositories::video_repo::VideoRepository;
use crate::util::error::ServiceError;

/// Hard limits applied defensively inside the service; the handlers already
/// enforce the same bounds, this is just defense in depth so a future caller
/// can never send a pathological LIMIT/OFFSET (which PostgreSQL would reject
/// or which could overflow i64).
const MAX_QUERY_LEN: usize = 200;
const MAX_PAGE: i64 = 1_000_000;
const MAX_SIZE: i64 = 100;

/// Suggest results are cached briefly — the underlying data changes only
/// when videos are added, and repeating the same rank/aggregate query on
/// every keystroke is wasteful.
const SUGGEST_CACHE_TTL_SECS: u64 = 60;
const SUGGEST_CACHE_MAX_ENTRIES: u64 = 1_000;

static SUGGEST_CACHE: OnceLock<Cache<String, Vec<String>>> = OnceLock::new();

fn suggest_cache() -> &'static Cache<String, Vec<String>> {
    SUGGEST_CACHE.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(SUGGEST_CACHE_TTL_SECS))
            .max_capacity(SUGGEST_CACHE_MAX_ENTRIES)
            .build()
    })
}

/// Trim the query and cap its length so a pathological input can never reach
/// the database as a giant bound value.
fn normalize_query(query: &str) -> String {
    let normalized: String = query
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    normalized.chars().take(MAX_QUERY_LEN).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub video_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rank: f32,
    pub headline: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SearchRow {
    video_id: i64,
    title: String,
    description: Option<String>,
    category: Option<String>,
    rank: f32,
    headline: Option<String>,
    total: i64,
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
    ) -> Result<(Vec<SearchResult>, i64), ServiceError> {
        // Empty/whitespace query: nothing matches a tsquery, short-circuit
        // instead of running a pointless scan (handlers do this too, this is
        // defense in depth).
        let query = normalize_query(query);
        if query.is_empty() {
            return Ok((Vec::new(), 0));
        }

        // Defense in depth: a negative/zero/oversized LIMIT or a negative
        // OFFSET is a PostgreSQL error; saturating arithmetic guarantees the
        // OFFSET can never overflow i64.
        let page = page.clamp(0, MAX_PAGE);
        let size = size.clamp(1, MAX_SIZE);
        let offset = page.saturating_mul(size);

        let pool = self.video_repo.pool();

        // SECURITY: we use the built-in 'simple' text-search configuration
        // rather than 'chinese' because the zhparser / pg_jieba extension
        // is not installed on standard PostgreSQL installs. 'simple' does
        // not tokenize Chinese, but it works for the Latin-script portion
        // of titles and avoids a hard 500 on every search.
        let rows = sqlx::query_as::<_, SearchRow>(
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
            ORDER BY rank DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&query)
        .bind(size)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| ServiceError::Internal(format!("搜索失败: {}", e)))?;

        let total: i64 = rows.first().map(|r| r.total).unwrap_or(0);
        let results = rows
            .into_iter()
            .map(|r| {
                let headline = r.headline.map(|s| strip_ts_headline_markers(&s));
                SearchResult {
                    video_id: r.video_id,
                    title: r.title,
                    description: r.description,
                    category: r.category,
                    rank: r.rank,
                    headline,
                }
            })
            .collect();

        Ok((results, total))
    }

    pub async fn search_suggest(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<String>, ServiceError> {
        let query = normalize_query(query);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, MAX_SIZE);

        let cache_key = format!("{}|{}", query, limit);
        if let Some(cached) = suggest_cache().get(&cache_key) {
            return Ok(cached);
        }

        let pool = self.video_repo.pool();

        // SECURITY: we use 'simple' (not 'chinese') because the zhparser /
        // pg_jieba extension is not installed on standard PostgreSQL.
        //
        // Suggestions are the tsvector matches (ranked) UNIONed with
        // case-insensitive title prefix matches. The prefix branch makes
        // suggestions useful mid-keystroke ("hello wo" → "hello world"),
        // which a full-token tsquery match alone cannot provide.
        //
        // The prefix branch uses `title ILIKE $2 || '%'` instead of the old
        // `lower(left(title, length($1))) = lower($1)`: the latter is a
        // non-sargable expression that always triggers a Seq Scan, while
        // ILIKE on a prefix can use the GIN trigram index
        // (idx_videos_title_trgm, migration 040). The pattern is bound as a
        // parameter — no SQL injection — and `%`/`_`/`\` are escaped in
        // `pattern` so user input stays literal-safe (a bare ILIKE would
        // otherwise treat them as wildcards). Group by title to dedupe; use
        // MAX(rank) to pick the best match; `title` is a sort tiebreaker so
        // equal-rank results have a stable order across pages/callers.
        let pattern = escape_like_pattern(&query);
        let rows = sqlx::query_scalar(
            r#"
            SELECT title
            FROM (
                SELECT title,
                       ts_rank(search_vector, plainto_tsquery('simple', $1)) AS rk
                FROM videos
                WHERE search_vector @@ plainto_tsquery('simple', $1)
                UNION ALL
                SELECT title, 0::real AS rk
                FROM videos
                WHERE title ILIKE $2 || '%'
            ) AS t
            GROUP BY title
            ORDER BY max(rk) DESC, title ASC
            LIMIT $3
            "#,
        )
        .bind(&query)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| ServiceError::Internal(format!("搜索建议失败: {}", e)))?;

        suggest_cache().insert(cache_key, rows.clone());
        Ok(rows)
    }
}

/// Strip the `<mark>...</mark>` start/stop selectors from a ts_headline result.
/// We just remove the literal substrings; the resulting text is plain.
fn strip_ts_headline_markers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut remaining = s;
    while let Some(start) = remaining.find("<mark>") {
        out.push_str(&remaining[..start]);
        remaining = &remaining[start + 6..];
        if let Some(end) = remaining.find("</mark>") {
            remaining = &remaining[end + 7..];
        }
    }
    out.push_str(remaining);
    out
}

/// Escape LIKE wildcards (`%`, `_`, `\`) with the default backslash escape so
/// user input to the suggest prefix branch stays literal: a query of `100%`
/// must not become a wildcard match. The escaped string is still bound as a
/// query parameter, so this is data escaping, not SQL injection surface.
fn escape_like_pattern(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
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
    fn test_escape_like_pattern_plain_text() {
        assert_eq!(escape_like_pattern("hello"), "hello");
    }

    #[test]
    fn test_escape_like_pattern_escapes_wildcards() {
        assert_eq!(escape_like_pattern("100%"), "100\\%");
        assert_eq!(escape_like_pattern("a_b"), "a\\_b");
        assert_eq!(escape_like_pattern("a\\b"), "a\\\\b");
    }

    #[test]
    fn test_escape_like_pattern_empty() {
        assert_eq!(escape_like_pattern(""), "");
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
