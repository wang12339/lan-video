-- Clean up orphaned playback_history rows before adding FK constraint
DELETE FROM playback_history WHERE video_id NOT IN (SELECT id FROM videos);

-- Add foreign key constraint with cascade delete for playback_history
ALTER TABLE playback_history
    DROP CONSTRAINT IF EXISTS playback_history_video_id_fkey,
    ADD CONSTRAINT playback_history_video_id_fkey
        FOREIGN KEY (video_id) REFERENCES videos(id)
        ON DELETE CASCADE;
