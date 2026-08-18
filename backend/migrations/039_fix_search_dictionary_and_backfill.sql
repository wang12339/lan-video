-- 039: 修复已知数据库问题
-- 1) 全文搜索词典不一致：021 迁移把触发器函数改为 'chinese'，但查询端
--    (search_service.rs) 因 zhparser/pg_jieba 未安装而使用 'simple'。
--    两者不一致导致 search_vector 内容与查询词典不匹配、搜索结果异常。
--    重建触发器函数为 'simple' 并回填 search_vector。
-- 2) 回填 auth_tokens.tenant_id：034 迁移新增的列默认 1，多租户用户的
--    旧 token 需按 users.tenant_id 对齐。
-- 3) 补充 videos(created_at) 索引：推荐服务 get_recent_videos 按
--    created_at DESC 排序且当前无任何 created_at 索引。

-- 1a. 重建全文搜索触发器与函数（先删触发器解除依赖，函数仅被该触发器使用）
DROP TRIGGER IF EXISTS videos_search_vector_update ON videos;
DROP FUNCTION IF EXISTS update_video_search_vector();
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
CREATE TRIGGER videos_search_vector_update
    BEFORE INSERT OR UPDATE ON videos
    FOR EACH ROW EXECUTE FUNCTION update_video_search_vector();

-- 1b. 回填既有 search_vector（与查询端 'simple' 词典一致，幂等：重复执行结果相同）
UPDATE videos SET search_vector = 
    setweight(to_tsvector('simple', coalesce(title, '')), 'A') ||
    setweight(to_tsvector('simple', coalesce(description, '')), 'B') ||
    setweight(to_tsvector('simple', coalesce(category, '')), 'C');

-- 2. 回填 auth_tokens.tenant_id 为所属用户的 tenant_id
--    （条件限定为 NULL 或默认值 1，幂等：已对齐的行不受影响）
UPDATE auth_tokens SET tenant_id = u.tenant_id
FROM users u
WHERE auth_tokens.user_id = u.id
  AND (auth_tokens.tenant_id IS NULL OR auth_tokens.tenant_id = 1);

-- 3. 补充 videos(created_at) 索引，支撑 ORDER BY created_at DESC 的最新视频查询
CREATE INDEX IF NOT EXISTS idx_videos_created_at ON videos(created_at DESC);
