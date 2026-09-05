-- 租户隔离加固：补齐弹幕表租户字段 + 缺失的租户索引
--
-- 背景：迁移 034 为 video / comment / tag / playlist / playback 等表添加了
-- tenant_id，但 041_danmaku 建表时漏掉了 tenant_id，导致弹幕成为唯一
-- 未隔离的资源。本迁移补齐该列并回填现有数据，同时为按租户过滤的
-- 查询补齐缺失的索引。

-- 1. danmaku 补齐 tenant_id，并以所属视频的租户回填存量数据
ALTER TABLE danmaku ADD COLUMN tenant_id BIGINT NOT NULL DEFAULT 1 REFERENCES tenants(id);

UPDATE danmaku d
SET tenant_id = v.tenant_id
FROM videos v
WHERE d.video_id = v.id;

CREATE INDEX IF NOT EXISTS idx_danmaku_tenant ON danmaku(tenant_id);

-- 2. 补齐按租户过滤时缺失的索引（复合索引已存在的不重复创建）
CREATE INDEX IF NOT EXISTS idx_videos_tenant ON videos(tenant_id);

-- 播放历史 / 点赞 / 收藏：按 (username, video_id) 命中后仍需按租户过滤
CREATE INDEX IF NOT EXISTS idx_playback_history_tenant ON playback_history(tenant_id);
CREATE INDEX IF NOT EXISTS idx_user_likes_tenant ON user_likes(tenant_id);
CREATE INDEX IF NOT EXISTS idx_user_favorites_tenant ON user_favorites(tenant_id);
CREATE INDEX IF NOT EXISTS idx_playlist_items_tenant ON playlist_items(tenant_id);
CREATE INDEX IF NOT EXISTS idx_video_variants_tenant ON video_variants(tenant_id);
CREATE INDEX IF NOT EXISTS idx_transcoding_jobs_tenant ON transcoding_jobs(tenant_id);