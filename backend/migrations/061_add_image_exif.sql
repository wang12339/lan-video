-- 061_add_image_exif.sql
--
-- 为 videos 表补充图片 EXIF 元数据列（拍摄时间/地点/器材/曝光参数/方向），
-- 供时间线排序、地图聚类与详情展示。图片上传后由后台任务解析 EXIF 回填，
-- 解析失败或图片本身无 EXIF 时列保持 NULL。
--
-- exif_extracted 标记"已尝试解析"：没有 EXIF 的图片（截图、导出图、视频等）
-- 解析结果全为 NULL，若只按字段是否为 NULL 判断待回填，会被反复扫描重试；
-- 用该布尔列区分"未解析"与"解析过但无数据"。
--
-- 幂等：所有列与索引均使用 IF NOT EXISTS，可安全重复执行。

-- ============================================
-- 1. videos 新增 EXIF 列
-- ============================================
ALTER TABLE videos
    -- 拍摄时间（EXIF DateTimeOriginal 优先；EXIF 无时区，统一按 UTC 归一化存储）
    ADD COLUMN IF NOT EXISTS exif_taken_at TIMESTAMPTZ,
    -- GPS 十进制度坐标（南纬/西经为负）
    ADD COLUMN IF NOT EXISTS exif_lat DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS exif_lon DOUBLE PRECISION,
    -- 器材：Make + Model 去重拼接；镜头 LensModel
    ADD COLUMN IF NOT EXISTS exif_camera TEXT,
    ADD COLUMN IF NOT EXISTS exif_lens TEXT,
    -- 曝光参数：光圈 FNumber、快门 ExposureTime 格式化字符串、ISO、焦距(mm)
    ADD COLUMN IF NOT EXISTS exif_aperture DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS exif_shutter TEXT,
    ADD COLUMN IF NOT EXISTS exif_iso INTEGER,
    ADD COLUMN IF NOT EXISTS exif_focal_length DOUBLE PRECISION,
    -- 方向 1..8（EXIF Orientation 原值）
    ADD COLUMN IF NOT EXISTS exif_orientation INTEGER,
    -- 已尝试解析（成功与否均置 TRUE），避免无 EXIF 的图片被反复回填
    ADD COLUMN IF NOT EXISTS exif_extracted BOOLEAN NOT NULL DEFAULT FALSE;

-- ============================================
-- 2. 索引：按拍摄时间倒序的时间线排序
-- ============================================
-- 部分索引只覆盖已解析出拍摄时间的行，避免把大量 NULL 行计入索引。
CREATE INDEX IF NOT EXISTS idx_videos_exif_taken_at
    ON videos(exif_taken_at DESC) WHERE exif_taken_at IS NOT NULL;
