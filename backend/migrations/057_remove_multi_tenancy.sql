-- 彻底移除多租户功能
--
-- 背景：项目改为单站点部署。本迁移删除租户/套餐相关的表、视图、函数与枚举，
-- 并删除所有业务表的 tenant_id 列（034/048 添加，默认值均为 1）。
-- tenant_id 上的索引与外键约束会随列一起删除；下面仍显式 DROP INDEX IF EXISTS
-- 以清理历史迁移中可能残存的索引。
--
-- 注意：这是不可逆迁移。执行后原多租户数据将归并到同一站点。

-- ============================================
-- 1. 租户统计视图
-- ============================================
DROP VIEW IF EXISTS tenant_usage_stats;

-- ============================================
-- 2. 业务表 tenant_id 列
-- ============================================
ALTER TABLE users DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE auth_tokens DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE videos DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE comments DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE share_links DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE tags DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE video_tags DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE playlists DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE playlist_items DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE playback_history DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE user_likes DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE user_favorites DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE video_variants DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE transcoding_jobs DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE danmaku DROP COLUMN IF EXISTS tenant_id CASCADE;
ALTER TABLE chat_messages DROP COLUMN IF EXISTS tenant_id CASCADE;

-- ============================================
-- 3. 清理 tenant_id 相关索引（多数已随列删除，IF EXISTS 兜底）
-- ============================================
DROP INDEX IF EXISTS idx_users_tenant;
DROP INDEX IF EXISTS idx_auth_tokens_tenant;
DROP INDEX IF EXISTS idx_videos_tenant;
DROP INDEX IF EXISTS idx_comments_tenant;
DROP INDEX IF EXISTS idx_share_links_tenant;
DROP INDEX IF EXISTS idx_tags_tenant;
DROP INDEX IF EXISTS idx_video_tags_tenant;
DROP INDEX IF EXISTS idx_playlists_tenant;
DROP INDEX IF EXISTS idx_playlist_items_tenant;
DROP INDEX IF EXISTS idx_playback_history_tenant;
DROP INDEX IF EXISTS idx_user_likes_tenant;
DROP INDEX IF EXISTS idx_user_favorites_tenant;
DROP INDEX IF EXISTS idx_video_variants_tenant;
DROP INDEX IF EXISTS idx_transcoding_jobs_tenant;
DROP INDEX IF EXISTS idx_danmaku_tenant;
DROP INDEX IF EXISTS idx_users_tenant_guest;
DROP INDEX IF EXISTS idx_users_tenant_created;
DROP INDEX IF EXISTS idx_videos_tenant_created;
DROP INDEX IF EXISTS idx_videos_tenant_uploader_file_hash;
DROP INDEX IF EXISTS idx_chat_messages_tenant_id;

-- ============================================
-- 4. 套餐与租户表
-- ============================================
DROP TABLE IF EXISTS plans CASCADE;
DROP TABLE IF EXISTS tenants CASCADE;

-- ============================================
-- 5. 租户触发器函数与枚举类型
-- ============================================
DROP FUNCTION IF EXISTS update_tenant_updated_at() CASCADE;
DROP TYPE IF EXISTS tenant_status;
