-- Add performance indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_videos_source_type ON videos(source_type);
CREATE INDEX IF NOT EXISTS idx_videos_file_hash ON videos(file_hash);
CREATE INDEX IF NOT EXISTS idx_playback_history_username ON playback_history(username);
CREATE INDEX IF NOT EXISTS idx_playback_history_username_video_id ON playback_history(username, video_id);
CREATE INDEX IF NOT EXISTS idx_auth_tokens_token ON auth_tokens(token);
