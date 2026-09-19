-- 059_optimizations.sql
-- 本次优化涉及三块内容：
--   1. 中文搜索走 pg_trgm：为 description 补 GIN trgm 索引
--   2. trending_score 函数 volatility 修正（IMMUTABLE -> STABLE），
--      让后台任务重算时分数能随时间衰减
--   3. video_variants / transcoding_jobs / video_tags 的 video_id 由
--      INTEGER 提升为 BIGINT，与 videos.id (BIGSERIAL) 对齐，避免大 ID 溢出
--
-- 迁移在单个事务中执行；以下语句均为幂等写法，可安全重复执行。

-- ============================================
-- 1. 中文搜索：description 的 trigram 索引
-- ============================================
-- pg_trgm 为标准 contrib 扩展；迁移 040 已安装，这里补一次保证独立可执行。
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- 中文查询由 search_service 走 ILIKE '%关键词%' 模糊匹配（CJK 无法被
-- 'simple' 词典分词，tsquery 命中不了）。title 的 trgm 索引迁移 040
-- 已有（idx_videos_title_trgm），这里为 description 补齐，避免每查一次
-- description 都全表扫描。
CREATE INDEX IF NOT EXISTS idx_videos_description_trgm
    ON videos USING gin (description gin_trgm_ops);

-- ============================================
-- 2. calculate_trending_score：volatility 修正
-- ============================================
-- 函数体依赖 CURRENT_TIMESTAMP（随时间变化），却被声明为 IMMUTABLE：
-- 这既违反 PostgreSQL 语义（IMMUTABLE 表示任何时候调用结果都相同，
-- 并可能被查询规划器提前折叠），也让迁移 037 的触发器只在
-- INSERT/UPDATE views/created_at 时计算一次、之后永不衰减。
-- 这里保持原有计算逻辑不变，仅：
--   - 改写为纯 SQL 函数（原 plpgsql 的 IF/DECLARE 用 CASE + 重复表达式表达）
--   - volatility 由 IMMUTABLE 改为 STABLE（同一事务内结果一致，可随
--     语句开始时间变化，规划器不会跨事务缓存）
CREATE OR REPLACE FUNCTION calculate_trending_score(
    views BIGINT,
    created_at TIMESTAMP WITH TIME ZONE
) RETURNS FLOAT AS $$
    SELECT CASE
        WHEN created_at IS NULL THEN 0::float
        ELSE (
            (LN(GREATEST(views, 0) + 1) * 100
             + GREATEST(views, 0)
               / GREATEST(EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) / 86400, 0.1)
             + CASE
                   WHEN GREATEST(EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) / 86400, 0.1) < 7
                   THEN 30 ELSE 0
               END
            )
            / POWER(
                  GREATEST(EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) / 86400, 0.1) + 2,
                  0.4
              )
        )
    END
$$ LANGUAGE sql STABLE;

-- ============================================
-- 3. video_id 列类型：INTEGER -> BIGINT
-- ============================================
-- videos.id 是 BIGSERIAL(BIGINT)，这三张子表的 video_id 仍是 INTEGER，
-- 视频 ID 超过 2^31 时插入/关联会溢出。外键约束会阻止 ALTER TYPE，
-- 因此先删外键，改完类型后按原 ON DELETE CASCADE 语义重建。
-- 索引 / 主键 / 唯一约束由 PostgreSQL 在改类型时自动重建，无需手动处理。

ALTER TABLE video_variants DROP CONSTRAINT IF EXISTS video_variants_video_id_fkey;
ALTER TABLE transcoding_jobs DROP CONSTRAINT IF EXISTS transcoding_jobs_video_id_fkey;
ALTER TABLE video_tags DROP CONSTRAINT IF EXISTS video_tags_video_id_fkey;

ALTER TABLE video_variants
    ALTER COLUMN video_id TYPE BIGINT USING video_id::bigint;
ALTER TABLE transcoding_jobs
    ALTER COLUMN video_id TYPE BIGINT USING video_id::bigint;
ALTER TABLE video_tags
    ALTER COLUMN video_id TYPE BIGINT USING video_id::bigint;

ALTER TABLE video_variants
    ADD CONSTRAINT video_variants_video_id_fkey
    FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE;
ALTER TABLE transcoding_jobs
    ADD CONSTRAINT transcoding_jobs_video_id_fkey
    FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE;
ALTER TABLE video_tags
    ADD CONSTRAINT video_tags_video_id_fkey
    FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE;
