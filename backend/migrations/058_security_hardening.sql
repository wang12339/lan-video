-- 058_security_hardening.sql
--
-- 1) 恢复 044 误删的热路径索引
-- 2) 上传去重：同一上传者 + 文件哈希唯一（防并发重复入库/重复计费）
-- 3) 网关 SSO 绑定不可变 sub（防按用户名关联导致的账号接管）

-- ============================================
-- 1. 恢复索引
-- ============================================
-- find_videos_without_cover 使用（videos 表热路径）
CREATE INDEX IF NOT EXISTS idx_videos_no_cover
    ON videos(id) WHERE cover_url IS NULL AND source_type LIKE 'local%';

-- find_existing_by_name_and_size_batch 使用（/admin/videos/check-files）
CREATE INDEX IF NOT EXISTS idx_videos_original_name_size
    ON videos(original_name, file_size);

-- ============================================
-- 2. 上传去重唯一索引
-- ============================================
-- 历史重复数据：保留 id 最小的一条，其余置空 file_hash（不删数据）
WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (PARTITION BY uploader_id, file_hash ORDER BY id) AS rn
    FROM videos
    WHERE file_hash IS NOT NULL AND uploader_id IS NOT NULL
)
UPDATE videos
SET file_hash = NULL
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

CREATE UNIQUE INDEX IF NOT EXISTS uq_videos_uploader_file_hash
    ON videos(uploader_id, file_hash)
    WHERE file_hash IS NOT NULL AND uploader_id IS NOT NULL;

-- ============================================
-- 3. 网关 SSO sub 绑定
-- ============================================
ALTER TABLE users ADD COLUMN IF NOT EXISTS gateway_sub TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS uq_users_gateway_sub
    ON users(gateway_sub) WHERE gateway_sub IS NOT NULL;
