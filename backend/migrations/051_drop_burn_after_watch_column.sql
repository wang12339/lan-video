-- 051: 阅后即焚改为平台全局行为
-- 任何用户（含上传者）完整观看任意视频（含存量视频）都会触发永久删除，
-- per-video 的 burn_after_watch 开关不再有意义，删除该列。
-- 迁移 050 引入，从未在生产启用过 per-video 语义，直接清除。
ALTER TABLE videos DROP COLUMN IF EXISTS burn_after_watch;
