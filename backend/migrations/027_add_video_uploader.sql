-- SECURITY (A04 H1): Track which user uploaded each video so we can enforce
-- ownership on share creation. Existing rows default to NULL; for those videos
-- only admins are allowed to create share links (until a re-upload is done).
ALTER TABLE videos ADD COLUMN IF NOT EXISTS uploader_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_videos_uploader_id ON videos(uploader_id);
