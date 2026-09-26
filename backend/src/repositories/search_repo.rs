//! Full-text / trigram search SQL for the `videos` table.
//!
//! This lives in a repository rather than in `SearchService` on purpose. The
//! service used to reach through `VideoRepository::pool()` to run its own
//! hand-written queries, which inverted the dependency direction: SQL lived in
//! the service layer while every other query lived here, so the search
//! statements escaped the repository's indexing, slow-query instrumentation and
//! future optimisation work.
//!
//! The service keeps the *policy* (which strategy to use, caching, LIKE
//! escaping, result mapping); this module keeps the *SQL*.

use crate::db::log_slow_query;
use sqlx::PgPool;

/// One search hit, plus the total row count for pagination.
///
/// `total` comes from `COUNT(*) OVER()` so a page of results and the total
/// count cost one round trip instead of two.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SearchRow {
    pub video_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rank: f32,
    pub headline: Option<String>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct SearchRepository {
    pool: PgPool,
}

impl SearchRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Whether this database actually provides a `chinese` text search
    /// configuration driven by the `zhparser` parser.
    ///
    /// Resolved through `search_path` exactly like a runtime `'chinese'`
    /// reference would be (PostgreSQL has no `to_regconfig()`), taking the
    /// first match. A configuration merely *named* `chinese` but backed by
    /// `COPY = simple` — common on stock PostgreSQL — is deliberately reported
    /// as unavailable, because it is not real Chinese word segmentation and
    /// would silently lower substring-match quality.
    pub async fn chinese_fts_available(&self) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT COALESCE(
                (
                    SELECT p.prsname = 'zhparser'
                    FROM unnest(current_schemas(true)) WITH ORDINALITY AS s(nspname, ord)
                    JOIN pg_namespace n ON n.nspname = s.nspname
                    JOIN pg_ts_config c ON c.cfgnamespace = n.oid AND c.cfgname = 'chinese'
                    JOIN pg_ts_parser p ON p.oid = c.cfgparser
                    ORDER BY s.ord
                    LIMIT 1
                ),
                false
            ) AS enabled
            "#,
        )
        .fetch_one(&self.pool)
        .await
    }

    /// ASCII query: `search_vector` + the built-in `'simple'` tsquery.
    ///
    /// SECURITY: `'simple'` is the dictionary the `search_vector` trigger
    /// maintains (migration 039) and the one `video_repo` hardcodes for list
    /// filtering. Using stock PostgreSQL without zhparser therefore cannot 500.
    pub async fn search_simple(
        &self,
        owner_id: Option<i64>,
        query: &str,
        size: i64,
        offset: i64,
    ) -> Result<Vec<SearchRow>, sqlx::Error> {
        log_slow_query("search_repo::search_simple", || async {
            sqlx::query_as::<_, SearchRow>(
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
                WHERE ($4::bigint IS NULL OR uploader_id = $4)
                  AND search_vector @@ plainto_tsquery('simple', $1)
                ORDER BY rank DESC, id DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(query)
            .bind(size)
            .bind(offset)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// CJK query without zhparser: pg_trgm ILIKE substring match.
    ///
    /// `pattern` must already be `%`-wrapped and LIKE-escaped by the caller.
    pub async fn search_trigram(
        &self,
        owner_id: Option<i64>,
        query: &str,
        pattern: &str,
        size: i64,
        offset: i64,
    ) -> Result<Vec<SearchRow>, sqlx::Error> {
        log_slow_query("search_repo::search_trigram", || async {
            sqlx::query_as::<_, SearchRow>(
                r#"
                SELECT
                    id as video_id,
                    title,
                    description,
                    category,
                    similarity(title, $1) as rank,
                    title as headline,
                    COUNT(*) OVER() AS total
                FROM videos
                WHERE ($4::bigint IS NULL OR uploader_id = $4)
                  AND (title ILIKE $5 OR COALESCE(description, '') ILIKE $5)
                ORDER BY similarity(title, $1) DESC, id DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(query)
            .bind(size)
            .bind(offset)
            .bind(owner_id)
            .bind(pattern)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// CJK query with zhparser: native segmentation tsquery OR trigram match.
    ///
    /// The union (rather than an either/or) means enabling zhparser can only add
    /// hits, never remove the substrings the ILIKE branch used to find.
    ///
    /// The expression is kept byte-identical to the `idx_videos_search_zhparser`
    /// expression index from migration 060, and the configuration name appears
    /// as a SQL *literal* (one of two fixed branches, never user input): binding
    /// it as `$N::regconfig` would stop the planner folding it to a constant,
    /// the expression index would not match, and CJK queries would silently
    /// degrade to sequential scans.
    pub async fn search_chinese(
        &self,
        owner_id: Option<i64>,
        query: &str,
        pattern: &str,
        size: i64,
        offset: i64,
    ) -> Result<Vec<SearchRow>, sqlx::Error> {
        log_slow_query("search_repo::search_chinese", || async {
            sqlx::query_as::<_, SearchRow>(
                r#"
                SELECT
                    id as video_id,
                    title,
                    description,
                    category,
                    GREATEST(
                        ts_rank(
                            to_tsvector('chinese', title || ' ' || COALESCE(description, '')),
                            plainto_tsquery('chinese', $1)
                        ),
                        similarity(title, $1)
                    ) as rank,
                    ts_headline(
                        'chinese',
                        title,
                        plainto_tsquery('chinese', $1),
                        'StartSel=<mark>, StopSel=</mark>, MaxWords=50, MinWords=20'
                    ) as headline,
                    COUNT(*) OVER() AS total
                FROM videos
                WHERE ($4::bigint IS NULL OR uploader_id = $4)
                  AND (
                      to_tsvector('chinese', title || ' ' || COALESCE(description, ''))
                          @@ plainto_tsquery('chinese', $1)
                      OR title ILIKE $5
                      OR COALESCE(description, '') ILIKE $5
                  )
                ORDER BY rank DESC, id DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(query)
            .bind(size)
            .bind(offset)
            .bind(owner_id)
            .bind(pattern)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }

    /// Ranked `tsvector` matches UNIONed with case-insensitive title-prefix
    /// matches.
    ///
    /// The prefix branch is what makes suggestions useful mid-keystroke
    /// ("hello wo" → "hello world"); a full-token tsquery match alone cannot.
    ///
    /// It uses `title ILIKE $2 || '%'` rather than the older
    /// `lower(left(title, length($1))) = lower($1)`: the latter is a
    /// non-sargable expression that always sequential-scans, while a prefix
    /// ILIKE can use the GIN trigram index (`idx_videos_title_trgm`,
    /// migration 040). `pattern` is bound as a parameter — no SQL injection —
    /// and the caller has already escaped `%` / `_` / `\` so user input stays
    /// literal. Grouping by title dedupes the union; `MAX(rk)` picks the best
    /// match and `title` is a tiebreaker so equal ranks order stably across
    /// pages and callers.
    pub async fn suggest_titles(
        &self,
        owner_id: Option<i64>,
        query: &str,
        pattern: &str,
        limit: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        log_slow_query("search_repo::suggest_titles", || async {
            sqlx::query_scalar(
                r#"
                SELECT title
                FROM (
                    SELECT title,
                           ts_rank(search_vector, plainto_tsquery('simple', $1)) AS rk
                    FROM videos
                    WHERE ($4::bigint IS NULL OR uploader_id = $4)
                      AND search_vector @@ plainto_tsquery('simple', $1)
                    UNION ALL
                    SELECT title, 0::real AS rk
                    FROM videos
                    WHERE ($4::bigint IS NULL OR uploader_id = $4)
                      AND title ILIKE $2 || '%'
                ) AS t
                GROUP BY title
                ORDER BY max(rk) DESC, title ASC
                LIMIT $3
                "#,
            )
            .bind(query)
            .bind(pattern)
            .bind(limit)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await
        })
        .await
    }
}
