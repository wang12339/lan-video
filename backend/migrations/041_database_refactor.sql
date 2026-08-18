-- 数据库重构脚本
-- 日期：2026-08-13
-- 目的：优化索引、清理冗余、提升性能

-- ============================================
-- 1. 删除未使用的索引（保留必要的主键和外键）
-- ============================================

-- auth_tokens 表：删除冗余索引
DROP INDEX IF EXISTS idx_auth_tokens_revoked;
DROP INDEX IF EXISTS idx_auth_tokens_tenant;
DROP INDEX IF EXISTS idx_auth_tokens_token_hash;  -- 与 auth_tokens_token_hash_key 重复
DROP INDEX IF EXISTS idx_auth_tokens_user_id;

-- comments 表：删除未使用的索引
DROP INDEX IF EXISTS idx_comments_created_at;
DROP INDEX IF EXISTS idx_comments_parent_id;
DROP INDEX IF EXISTS idx_comments_tenant;
DROP INDEX IF EXISTS idx_comments_user_id;
DROP INDEX IF EXISTS idx_comments_video_id;

-- email_verification_tokens 表：删除未使用的索引
DROP INDEX IF EXISTS idx_email_verification_tokens_hash;
DROP INDEX IF EXISTS idx_email_verification_tokens_user;

-- password_reset_tokens 表：删除未使用的索引
DROP INDEX IF EXISTS idx_password_reset_tokens_hash;
DROP INDEX IF EXISTS idx_password_reset_tokens_user;

-- playback_history 表：删除冗余索引
DROP INDEX IF EXISTS idx_playback_history_tenant;
DROP INDEX IF EXISTS idx_playback_history_username;
DROP INDEX IF EXISTS idx_playback_history_username_video_id;  -- 与 username_video_id_key 重复

-- playlist_items 表：删除未使用的索引
DROP INDEX IF EXISTS idx_playlist_items_playlist;
DROP INDEX IF EXISTS idx_playlist_items_position;

-- playlists 表：删除未使用的索引
DROP INDEX IF EXISTS idx_playlists_tenant;
DROP INDEX IF EXISTS idx_playlists_user_id;

-- share_links 表：删除未使用的索引
DROP INDEX IF EXISTS idx_share_links_expires;
DROP INDEX IF EXISTS idx_share_links_tenant;
DROP INDEX IF EXISTS idx_share_links_token_hash;
DROP INDEX IF EXISTS idx_share_links_video_id;

-- tags 表：删除未使用的索引
DROP INDEX IF EXISTS idx_tags_name;
DROP INDEX IF EXISTS idx_tags_tenant;
DROP INDEX IF EXISTS idx_tags_usage_count;

-- tenants 表：删除未使用的索引
DROP INDEX IF EXISTS idx_tenants_slug;

-- transcoding_jobs 表：删除未使用的索引
DROP INDEX IF EXISTS idx_transcoding_jobs_status;
DROP INDEX IF EXISTS idx_transcoding_jobs_video_id;

-- user_favorites 表：删除未使用的索引
DROP INDEX IF EXISTS idx_user_favorites_username_video;
DROP INDEX IF EXISTS idx_user_favorites_video_id;

-- user_likes 表：删除未使用的索引
DROP INDEX IF EXISTS idx_user_likes_username_video;
DROP INDEX IF EXISTS idx_user_likes_video_id;

-- users 表：删除未使用的索引
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_users_email_unique;
DROP INDEX IF EXISTS idx_users_role;
DROP INDEX IF EXISTS idx_users_tenant;

-- video_tags 表：删除未使用的索引
DROP INDEX IF EXISTS idx_video_tags_tag;
DROP INDEX IF EXISTS idx_video_tags_tenant;

-- video_variants 表：删除未使用的索引
DROP INDEX IF EXISTS idx_video_variants_resolution;

-- videos 表：删除未使用的索引
DROP INDEX IF EXISTS idx_videos_category_trgm;
DROP INDEX IF EXISTS idx_videos_file_hash;
DROP INDEX IF EXISTS idx_videos_no_cover;
DROP INDEX IF EXISTS idx_videos_original_name_size;

-- ============================================
-- 2. 添加必要的索引（根据实际查询模式）
-- ============================================

-- 为 comments 表添加常用查询索引
CREATE INDEX IF NOT EXISTS idx_comments_video_created 
ON comments (video_id, created_at DESC);

-- 为 playback_history 表添加常用查询索引
CREATE INDEX IF NOT EXISTS idx_playback_user_updated 
ON playback_history (username, updated_at DESC);

-- 为 share_links 表添加常用查询索引
CREATE INDEX IF NOT EXISTS idx_share_links_video_created 
ON share_links (video_id, created_at DESC);

-- 为 user_likes 表添加常用查询索引
CREATE INDEX IF NOT EXISTS idx_user_likes_user_video 
ON user_likes (username, video_id);

-- 为 user_favorites 表添加常用查询索引
CREATE INDEX IF NOT EXISTS idx_user_favorites_user_video 
ON user_favorites (username, video_id);

-- ============================================
-- 3. 清理死元组
-- ============================================

-- 清理 videos 表
VACUUM ANALYZE videos;

-- 清理 playback_history 表
VACUUM ANALYZE playback_history;

-- 清理 auth_tokens 表
VACUUM ANALYZE auth_tokens;

-- 清理 users 表
VACUUM ANALYZE users;

-- 清理其他表
VACUUM ANALYZE comments;
VACUUM ANALYZE share_links;
VACUUM ANALYZE user_likes;
VACUUM ANALYZE user_favorites;
VACUUM ANALYZE video_tags;
VACUUM ANALYZE video_variants;
VACUUM ANALYZE transcoding_jobs;
VACUUM ANALYZE playlists;
VACUUM ANALYZE playlist_items;
VACUUM ANALYZE tags;
VACUUM ANALYZE tenants;
VACUUM ANALYZE server_config;

-- ============================================
-- 4. 更新表统计信息
-- ============================================

ANALYZE videos;
ANALYZE users;
ANALYZE playback_history;
ANALYZE auth_tokens;
ANALYZE comments;
ANALYZE share_links;

-- ============================================
-- 5. 添加软删除支持（可选）
-- ============================================

-- 为 videos 表添加软删除字段
ALTER TABLE videos ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMP WITH TIME ZONE;
CREATE INDEX IF NOT EXISTS idx_videos_deleted_at ON videos (deleted_at) WHERE deleted_at IS NOT NULL;

-- 为 users 表添加软删除字段
ALTER TABLE users ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMP WITH TIME ZONE;
CREATE INDEX IF NOT EXISTS idx_users_deleted_at ON users (deleted_at) WHERE deleted_at IS NOT NULL;

-- 为 comments 表添加软删除字段
ALTER TABLE comments ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMP WITH TIME ZONE;
CREATE INDEX IF NOT EXISTS idx_comments_deleted_at ON comments (deleted_at) WHERE deleted_at IS NOT NULL;

-- ============================================
-- 6. 添加审计字段（可选）
-- ============================================

-- 为 videos 表添加更新者字段
ALTER TABLE videos ADD COLUMN IF NOT EXISTS updated_by BIGINT REFERENCES users(id);
ALTER TABLE videos ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP;

-- 为 comments 表添加更新者字段
ALTER TABLE comments ADD COLUMN IF NOT EXISTS updated_by BIGINT REFERENCES users(id);
ALTER TABLE comments ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP;

-- ============================================
-- 7. 优化表参数
-- ============================================

-- 设置 autovacuum 参数
ALTER TABLE videos SET (autovacuum_vacuum_scale_factor = 0.1);
ALTER TABLE videos SET (autovacuum_analyze_scale_factor = 0.05);

ALTER TABLE playback_history SET (autovacuum_vacuum_scale_factor = 0.2);
ALTER TABLE playback_history SET (autovacuum_analyze_scale_factor = 0.1);

ALTER TABLE auth_tokens SET (autovacuum_vacuum_scale_factor = 0.3);
ALTER TABLE auth_tokens SET (autovacuum_analyze_scale_factor = 0.15);

-- ============================================
-- 完成！
-- ============================================

-- 输出重构结果
DO $$
DECLARE
    index_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO index_count 
    FROM pg_indexes 
    WHERE schemaname = 'public';
    
    RAISE NOTICE '数据库重构完成！当前索引数量: %', index_count;
END $$;
