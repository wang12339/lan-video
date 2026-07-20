-- Add CHECK constraints for data integrity
-- Role values: 0=banned, 1=viewer, 2=editor, 3=admin
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_role_check') THEN
    ALTER TABLE users ADD CONSTRAINT users_role_check CHECK (role >= 0 AND role <= 3);
  END IF;
END $$;

-- Non-negative constraints
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'videos_views_check') THEN
    ALTER TABLE videos ADD CONSTRAINT videos_views_check CHECK (views >= 0);
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'videos_duration_check') THEN
    ALTER TABLE videos ADD CONSTRAINT videos_duration_check CHECK (duration >= 0);
  END IF;
END $$;

-- Add missing indexes for query performance
CREATE INDEX IF NOT EXISTS idx_share_links_expires ON share_links(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_user_likes_video_id ON user_likes(video_id);
CREATE INDEX IF NOT EXISTS idx_user_favorites_video_id ON user_favorites(video_id);
CREATE INDEX IF NOT EXISTS idx_playback_history_video_id ON playback_history(video_id);
CREATE INDEX IF NOT EXISTS idx_video_variants_video_id ON video_variants(video_id);
CREATE INDEX IF NOT EXISTS idx_transcoding_jobs_video_id ON transcoding_jobs(video_id);
