-- M-02: 搜索建议前缀查询改为 `title ILIKE $1 || '%'`，需要 GIN trigram 索引。
-- pg_trgm 为标准 PostgreSQL contrib 扩展（PG >= 9.1，默认随服务器安装）；
-- CREATE EXTENSION 是事务性的，与本迁移在同一事务中执行。
-- 若部署环境禁止安装扩展，退化为普通索引：
--   CREATE INDEX IF NOT EXISTS idx_videos_title_trgm ON videos (lower(title));
-- 但 ILIKE 无法走普通 btree 索引（除非模式是常量），因此退化方案仅在使用
-- `title ILIKE $1 || '%'` 前先转写为 lower() 等值匹配时才有意义。
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_videos_title_trgm
    ON videos USING gin (title gin_trgm_ops);

-- M-01: 个性化推荐两段式的第二段（热门补足）与无过滤列表的默认排序
-- （views DESC, id DESC）都需要按浏览量取序，无索引时为 Seq Scan + Sort。
CREATE INDEX IF NOT EXISTS idx_videos_views_id
    ON videos (views DESC, id DESC);
