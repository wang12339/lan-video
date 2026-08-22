-- 修复迁移 041 未生效的索引变更
-- 原因：041 包含 VACUUM ANALYZE 语句，PostgreSQL 不允许在事务块内执行 VACUUM，
-- 导致整个迁移回滚，但 _schema_migrations 中被手动标记为已应用。
-- 本迁移只包含 DDL（不含 VACUUM），确保在事务内可正常执行。
-- VACUUM 操作应在迁移外单独执行（psql 或 autovacuum）。

-- ============================================
-- 1. 删除冗余/未使用的索引
-- ============================================

-- auth_tokens 表：token_hash 唯一约束已覆盖查找，以下索引冗余
DROP INDEX IF EXISTS idx_auth_tokens_revoked;
DROP INDEX IF EXISTS idx_auth_tokens_tenant;
DROP INDEX IF EXISTS idx_auth_tokens_token_hash;  -- 与 auth_tokens_token_hash_key 重复
DROP INDEX IF EXISTS idx_auth_tokens_user_id;

-- comments 表：复合索引 idx_comments_video_created 将替代以下单列索引
DROP INDEX IF EXISTS idx_comments_created_at;
DROP INDEX IF EXISTS idx_comments_parent_id;
DROP INDEX IF EXISTS idx_comments_tenant;
DROP INDEX IF EXISTS idx_comments_user_id;
DROP INDEX IF EXISTS idx_comments_video_id;

-- email_verification_tokens / password_reset_tokens 表：token_hash 唯一约束已覆盖
DROP INDEX IF EXISTS idx_email_verification_tokens_hash;
DROP INDEX IF EXISTS idx_email_verification_tokens_user;
DROP INDEX IF EXISTS idx_password_reset_tokens_hash;
DROP INDEX IF EXISTS idx_password_reset_tokens_user;

-- playback_history 表：复合索引 idx_playback_user_updated 将替代以下索引
DROP INDEX IF EXISTS idx_playback_history_tenant;
DROP INDEX IF EXISTS idx_playback_history_username;
DROP INDEX IF EXISTS idx_playback_history_username_video_id;  -- 与 username_video_id_key 重复

-- playlist_items / playlists 表
DROP INDEX IF EXISTS idx_playlist_items_playlist;
DROP INDEX IF EXISTS idx_playlist_items_position;
DROP INDEX IF EXISTS idx_playlists_tenant;
DROP INDEX IF EXISTS idx_playlists_user_id;

-- share_links 表：复合索引 idx_share_links_video_created 将替代以下索引
DROP INDEX IF EXISTS idx_share_links_expires;
DROP INDEX IF EXISTS idx_share_links_tenant;
DROP INDEX IF EXISTS idx_share_links_token_hash;
DROP INDEX IF EXISTS idx_share_links_video_id;

-- tags / tenants 表
DROP INDEX IF EXISTS idx_tags_name;
DROP INDEX IF EXISTS idx_tags_tenant;
DROP INDEX IF EXISTS idx_tags_usage_count;
DROP INDEX IF EXISTS idx_tenants_slug;

-- transcoding_jobs 表
DROP INDEX IF EXISTS idx_transcoding_jobs_status;
DROP INDEX IF EXISTS idx_transcoding_jobs_video_id;

-- user_favorites / user_likes 表：复合索引将替代以下索引
DROP INDEX IF EXISTS idx_user_favorites_username_video;
DROP INDEX IF EXISTS idx_user_favorites_video_id;
DROP INDEX IF EXISTS idx_user_likes_username_video;
DROP INDEX IF EXISTS idx_user_likes_video_id;

-- users 表
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_users_email_unique;
DROP INDEX IF EXISTS idx_users_role;
DROP INDEX IF EXISTS idx_users_tenant;

-- video_tags / video_variants 表
DROP INDEX IF EXISTS idx_video_tags_tag;
DROP INDEX IF EXISTS idx_video_tags_tenant;
DROP INDEX IF EXISTS idx_video_variants_resolution;

-- videos 表：以下索引未被实际查询使用
DROP INDEX IF EXISTS idx_videos_category_trgm;
DROP INDEX IF EXISTS idx_videos_file_hash;
DROP INDEX IF EXISTS idx_videos_no_cover;
DROP INDEX IF EXISTS idx_videos_original_name_size;

-- ============================================
-- 2. 添加必要的复合索引（根据实际查询模式）
-- ============================================

-- comments 列表查询：WHERE video_id = $1 ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_comments_video_created
ON comments (video_id, created_at DESC);

-- playback_history 查询：WHERE username = $1 ORDER BY updated_at DESC
CREATE INDEX IF NOT EXISTS idx_playback_user_updated
ON playback_history (username, updated_at DESC);

-- share_links 查询：WHERE video_id = $1 ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_share_links_video_created
ON share_links (video_id, created_at DESC);

-- user_likes 状态查询：WHERE username = $1 AND video_id = $2
CREATE INDEX IF NOT EXISTS idx_user_likes_user_video
ON user_likes (username, video_id);

-- user_favorites 状态查询：WHERE username = $1 AND video_id = $2
CREATE INDEX IF NOT EXISTS idx_user_favorites_user_video
ON user_favorites (username, video_id);
