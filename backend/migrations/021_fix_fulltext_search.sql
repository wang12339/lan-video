-- 021: 修复全文搜索词典不一致。
-- 历史：016 的触发器用 'simple' 而查询端曾用 'chinese'。本迁移原本把触发器
-- 切换到 'chinese'，但 vanilla PostgreSQL 没有 'chinese' 配置（此前依赖带外
-- 手工创建），导致全新数据库（包括 CI 的 postgres 容器）在迁移时失败；
-- 运行时查询端（search_service.rs）也因 zhparser/pg_jieba 不可用最终统一
-- 回 'simple'（039 迁移已把触发器改回 'simple' 并回填）。
-- 因此这里保持 'simple'，使迁移链自包含；最终 schema 状态与 039 后一致。

CREATE OR REPLACE FUNCTION update_video_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('simple', coalesce(NEW.title, '')), 'A') ||
        setweight(to_tsvector('simple', coalesce(NEW.description, '')), 'B') ||
        setweight(to_tsvector('simple', coalesce(NEW.category, '')), 'C');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Rebuild search vectors for existing videos using the same dictionary as queries
UPDATE videos SET search_vector =
    setweight(to_tsvector('simple', coalesce(title, '')), 'A') ||
    setweight(to_tsvector('simple', coalesce(description, '')), 'B') ||
    setweight(to_tsvector('simple', coalesce(category, '')), 'C');
