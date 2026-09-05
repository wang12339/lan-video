-- 050: 阅后即焚（burn after watch）
-- 视频级开关：启用后，任何非上传者用户完整观看（播放进度 ≥ 90%）后，
-- 后端将永久删除该视频的物理文件（主文件/转码变体/封面/缩略图）与数据库记录。
-- 上传者本人观看不触发（便于预览检查），上传时通过表单字段 / 续传请求头设置。
ALTER TABLE videos ADD COLUMN IF NOT EXISTS burn_after_watch BOOLEAN NOT NULL DEFAULT FALSE;

-- 焚毁候选查询（管理端/统计用）基数很小，不建索引。
