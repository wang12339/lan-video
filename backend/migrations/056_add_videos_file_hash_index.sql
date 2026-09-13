-- 上传去重/check-hashes 的复合索引。
--
-- 005 创建的 idx_videos_file_hash 在 042/044 重建索引时被删除后一直未恢复，
-- 导致 find_video_by_file_hash（WHERE file_hash = $1 AND tenant_id = $2
-- AND uploader_id = $3）与 find_existing_hashes（file_hash = ANY(...)）
-- 退化为全表扫描。上传量增长后 finalize 延迟随之线性上升。
--
-- 部分索引只覆盖 file_hash 非空的行（外链视频/目录扫描可能为空），
-- 列顺序与去重查询的等值条件对齐以保证命中。
CREATE INDEX IF NOT EXISTS idx_videos_tenant_uploader_file_hash
    ON videos (tenant_id, uploader_id, file_hash)
    WHERE file_hash IS NOT NULL;
