use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::RwLock;

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

/// zhparser `chinese` 配置探测缓存（进程级）。
///
/// 外层 `OnceLock` 只初始化一次；内层 `Option` 区分“未探测”与探测结果，
/// 探测失败不写缓存（下一次调用重试），避免临时的数据库抖动把进程永久
/// 固定在回退路径上。测试可用 `reset_chinese_fulltext_cache_for_tests` 清空。
static CHINESE_FTS_CACHE: OnceLock<RwLock<Option<bool>>> = OnceLock::new();

fn chinese_fts_cache() -> &'static RwLock<Option<bool>> {
    CHINESE_FTS_CACHE.get_or_init(|| RwLock::new(None))
}

/// 当前数据库是否提供由 zhparser 驱动的 `chinese` 文本搜索配置。
///
/// 只有配置的 parser 名严格为 `zhparser` 才算可用：本地/测试环境可能用
/// `CREATE TEXT SEARCH CONFIGURATION chinese (COPY = simple)` 建同名占位
/// 配置，它不是原生中文分词，必须继续走 pg_trgm 回退，否则中文子串命中率
/// 会下降。探测结果进程内缓存一次。
async fn chinese_fulltext_enabled(pool: &PgPool) -> bool {
    if let Some(enabled) = *chinese_fts_cache().read().await {
        return enabled;
    }
    let mut guard = chinese_fts_cache().write().await;
    if let Some(enabled) = *guard {
        return enabled;
    }
    match probe_chinese_fts(pool).await {
        Ok(enabled) => {
            *guard = Some(enabled);
            enabled
        }
        Err(e) => {
            tracing::warn!("chinese 全文配置探测失败，本次回退 simple/pg_trgm: {}", e);
            false
        }
    }
}

/// 按 `search_path` 解析 `chinese`（PostgreSQL 没有 `to_regconfig()`，只能走
/// 目录 + `current_schemas(true)`），与运行时 `'chinese'` 的名字解析规则一致，
/// 取第一个命中的配置；没有配置时返回 false。
async fn probe_chinese_fts(pool: &PgPool) -> Result<bool, sqlx::Error> {
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
    .fetch_one(pool)
    .await
}

/// 清空 zhparser 探测缓存。仅供 lib 单元测试使用；集成测试里 `COPY = simple`
/// 的 `chinese` 占位配置不会被探测命中（parser 不是 zhparser），无需重置。
#[cfg(test)]
pub(crate) async fn reset_chinese_fulltext_cache_for_tests() {
    *chinese_fts_cache().write().await = None;
}

/// 查询是否包含非 ASCII 字符（中文 / 日文 / 韩文等）。
///
/// PostgreSQL 内置的 'simple' 词典不会对 CJK 文本分词（`plainto_tsquery`
/// 会把整段中文当成一个词元），tsquery 路径基本命中不了中文查询。
/// 无 zhparser 时这类查询改走 pg_trgm 的 ILIKE 模糊匹配分支；
/// 有 zhparser 时走 `chinese` 配置的原生分词（见 `full_text_search`）。
fn contains_non_ascii(s: &str) -> bool {
    !s.is_ascii()
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
        owner_id: Option<i64>,
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

        // 中文（CJK）查询：
        // - 装了 zhparser（060 迁移创建 chinese 配置 + 表达式索引）：原生
        //   分词的 tsquery 与 pg_trgm 子串匹配取并集，命中率只增不减；
        // - 未装：保持既有 pg_trgm ILIKE 回退，行为不变。
        // 纯 ASCII 查询始终走 search_vector + 'simple' tsquery（search_vector
        // 由触发器用 simple 维护，见 039 迁移），英文 rank/排序语义不变。
        let non_ascii = contains_non_ascii(&query);
        let use_chinese_native = non_ascii && chinese_fulltext_enabled(pool).await;

        let rows = if use_chinese_native {
            search_videos_chinese(pool, owner_id, &query, size, offset).await?
        } else if non_ascii {
            search_videos_trigram(pool, owner_id, &query, size, offset).await?
        } else {
            search_videos_simple(pool, owner_id, &query, size, offset).await?
        };

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
        owner_id: Option<i64>,
        query: &str,
        limit: i64,
    ) -> Result<Vec<String>, ServiceError> {
        let query = normalize_query(query);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, MAX_SIZE);

        let cache_key = format!("{}|{}|{}", owner_id.unwrap_or(0), query, limit);
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
        .bind(&query)
        .bind(&pattern)
        .bind(limit)
        .bind(owner_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ServiceError::Internal(format!("搜索建议失败: {}", e)))?;

        suggest_cache().insert(cache_key, rows.clone());
        Ok(rows)
    }
}

/// 中文（CJK）查询 + zhparser：原生分词 tsquery OR pg_trgm 子串匹配。
///
/// 与 pg_trgm 分支取并集而非二选一，保证启用 zhparser 后中文命中率只增不减
/// （原来 ILIKE 能命中的子串仍然命中，另加按词命中的结果）。
///
/// 表达式与迁移 060 的 `idx_videos_search_zhparser` 逐字一致，且配置名以
/// SQL 字面量出现（两个固定分支之一，非用户输入）：若把配置作为
/// `$N::regconfig` 绑定参数传入，规划器无法把参数折叠为常量，表达式索引
/// 匹配不上，中文查询会退化为顺序扫描。
async fn search_videos_chinese(
    pool: &PgPool,
    owner_id: Option<i64>,
    query: &str,
    size: i64,
    offset: i64,
) -> Result<Vec<SearchRow>, ServiceError> {
    let pattern = format!("%{}%", escape_like_pattern(query));
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
    .bind(&pattern)
    .fetch_all(pool)
    .await
    .map_err(|e| ServiceError::Internal(format!("搜索失败: {}", e)))
}

/// 中文（CJK）查询且无 zhparser：pg_trgm ILIKE 子串匹配（既有回退路径）。
///
/// 通配符转义：查询里的 % / _ / \ 按字面匹配（如 "100%" 不能变成通配），
/// pattern 作为绑定参数传入，无注入面。
async fn search_videos_trigram(
    pool: &PgPool,
    owner_id: Option<i64>,
    query: &str,
    size: i64,
    offset: i64,
) -> Result<Vec<SearchRow>, ServiceError> {
    let pattern = format!("%{}%", escape_like_pattern(query));
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
    .bind(&pattern)
    .fetch_all(pool)
    .await
    .map_err(|e| ServiceError::Internal(format!("搜索失败: {}", e)))
}

/// 纯 ASCII 查询：search_vector + 内置 'simple' tsquery（行为保持不变）。
///
/// SECURITY: 使用内置 'simple' 配置而非 'chinese'：search_vector 由触发器
/// 用 'simple' 维护（039 迁移），video_repo.rs 的列表过滤同样硬编码
/// 'simple'；标准 PostgreSQL 未装 zhparser 时也不会硬 500。
async fn search_videos_simple(
    pool: &PgPool,
    owner_id: Option<i64>,
    query: &str,
    size: i64,
    offset: i64,
) -> Result<Vec<SearchRow>, ServiceError> {
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
    .fetch_all(pool)
    .await
    .map_err(|e| ServiceError::Internal(format!("搜索失败: {}", e)))
}

/// Strip the `<mark>...</mark>` start/stop selectors from a ts_headline result.
/// We just remove the literal substrings; the resulting text is plain.
fn strip_ts_headline_markers(s: &str) -> String {
    s.replace("<mark>", "").replace("</mark>", "")
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
    fn test_contains_non_ascii_ascii_only() {
        assert!(!contains_non_ascii(""));
        assert!(!contains_non_ascii("hello world"));
        assert!(!contains_non_ascii("100%_\\--"));
    }

    #[test]
    fn test_contains_non_ascii_detects_cjk() {
        assert!(contains_non_ascii("中文"));
        assert!(contains_non_ascii("hello 世界"));
        assert!(contains_non_ascii("カタカナ"));
        assert!(contains_non_ascii("한글"));
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

    /// 探测失败（死池）必须当次回退，且不得写入缓存——否则临时的数据库
    /// 故障会把进程永久固定在 pg_trgm 回退路径上。
    #[tokio::test]
    async fn test_chinese_probe_failure_falls_back_without_caching() {
        reset_chinese_fulltext_cache_for_tests().await;
        let state = crate::test_support::test_state("http://localhost:3000");
        let pool = state.repos.video.pool();

        assert!(!chinese_fulltext_enabled(pool).await);
        assert!(
            chinese_fts_cache().read().await.is_none(),
            "探测失败不得写入缓存"
        );

        reset_chinese_fulltext_cache_for_tests().await;
    }

    /// 测试用重置辅助：写入缓存后可被清空（保证测试隔离）。
    #[tokio::test]
    async fn test_chinese_probe_cache_can_be_reset() {
        *chinese_fts_cache().write().await = Some(true);
        reset_chinese_fulltext_cache_for_tests().await;
        assert!(chinese_fts_cache().read().await.is_none());
    }
}
