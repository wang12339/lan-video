-- Fix comments column types to match Rust CommentRow struct
-- id, video_id, user_id, parent_id need to be BIGINT for i64
-- created_at needs to be TIMESTAMP for NaiveDateTime

ALTER TABLE comments
  ALTER COLUMN id TYPE BIGINT,
  ALTER COLUMN video_id TYPE BIGINT,
  ALTER COLUMN user_id TYPE BIGINT,
  ALTER COLUMN parent_id TYPE BIGINT,
  ALTER COLUMN created_at TYPE TIMESTAMP;

-- Update the sequence to match BIGINT
ALTER SEQUENCE comments_id_seq AS BIGINT;

-- Rebuild indexes after type changes
REINDEX TABLE comments;
