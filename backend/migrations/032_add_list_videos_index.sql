-- Composite index to accelerate the most common list_videos query:
--   SELECT ... FROM videos WHERE source_type = ? ORDER BY views DESC, id DESC LIMIT ? OFFSET ?
-- Without this, PG does an index scan on idx_videos_source_type then a separate sort.
CREATE INDEX IF NOT EXISTS idx_videos_source_type_views_id
    ON videos (source_type, views DESC, id DESC);

-- Also cover the category-only filter (less frequent but still common)
CREATE INDEX IF NOT EXISTS idx_videos_category_views_id
    ON videos (category, views DESC, id DESC);
