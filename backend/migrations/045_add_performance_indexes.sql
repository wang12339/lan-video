-- Add performance indexes for frequently queried columns
-- Based on analysis of backend code and query patterns

-- ============================================
-- 1. Videos table - Category index
-- ============================================
-- The original idx_videos_category was dropped in migration 042,
-- but the code still filters by category in video_repo.rs
-- This index is needed for: WHERE category = $1
-- Note: idx_videos_category_views_id exists but is a composite index
-- optimized for sorting by views, not for pure category filtering
CREATE INDEX IF NOT EXISTS idx_videos_category ON videos(category);

-- ============================================
-- 2. Playback history - Enhanced composite index
-- ============================================
-- Current idx_playback_user_updated covers: WHERE username = $1 ORDER BY updated_at DESC
-- This enhanced index adds video_id to INCLUDE for covering index
-- This eliminates table lookups for the main history query
-- Used by: find_playback_history_by_username, count_watched_videos, sum_watch_time
CREATE INDEX IF NOT EXISTS idx_playback_user_updated_covering
ON playback_history (username, updated_at DESC)
INCLUDE (video_id, position_ms, duration_ms);

-- ============================================
-- 3. Comments - Parent_id index for reply queries
-- ============================================
-- The idx_comments_video_created covers: WHERE video_id = $1 ORDER BY created_at DESC
-- But get_replies() queries: WHERE parent_id = $1 ORDER BY created_at ASC
-- This index supports efficient reply retrieval
CREATE INDEX IF NOT EXISTS idx_comments_parent_created
ON comments (parent_id, created_at ASC)
INCLUDE (video_id, user_id, content);

-- ============================================
-- 4. Comments - User_id index for user comment queries
-- ============================================
-- Useful for queries like: WHERE user_id = $1 ORDER BY created_at DESC
-- Not currently in the codebase but may be needed for user profile pages
CREATE INDEX IF NOT EXISTS idx_comments_user_created
ON comments (user_id, created_at DESC)
INCLUDE (video_id, content);

-- ============================================
-- 5. Playback history - video_id index for joins
-- ============================================
-- The unique constraint on (username, video_id) helps, but
-- explicit video_id index is useful for: JOIN playback_history ON video_id = $1
-- Used in recommendation service and video detail queries
CREATE INDEX IF NOT EXISTS idx_playback_video_id ON playback_history(video_id);

-- ============================================
-- Update statistics for all affected tables
-- ============================================
ANALYZE videos;
ANALYZE playback_history;
ANALYZE comments;