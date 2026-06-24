-- Add missing indexes for common query patterns identified in video_repo.rs

-- 1. Category filter used in count_all, find_all, find_all_paged
CREATE INDEX IF NOT EXISTS idx_videos_category ON videos(category);

-- 2. Composite index for find_existing_by_name_and_size_batch
-- Speeds up WHERE (original_name, file_size) IN (...) checks
CREATE INDEX IF NOT EXISTS idx_videos_original_name_size ON videos(original_name, file_size);

-- 3. Partial index for find_videos_without_cover (cursor-based pagination)
-- Only indexes rows matching the WHERE clause — small, fast
CREATE INDEX IF NOT EXISTS idx_videos_no_cover ON videos(id)
    WHERE cover_url IS NULL AND source_type LIKE 'local%';

-- 4. Covering index for playback history with updated_at (for sorted history queries)
-- Replaces idx_playback_history_username_video_id with added coverage
-- Used by find_playback_history_by_username, find_recent_history_with_details,
-- count_watched_videos, and sum_watch_time
CREATE INDEX IF NOT EXISTS idx_playback_history_user_updated
    ON playback_history(username, updated_at DESC)
    INCLUDE (video_id, position_ms, duration_ms);

-- Note: idx_playback_history_username and idx_playback_history_username_video_id
-- are kept for backward compatibility but idx_playback_history_user_updated
-- covers the same queries with better performance.
