-- Fix playlists and playlist_items column types to match Rust structs
-- created_at/updated_at need to be TIMESTAMP for NaiveDateTime

ALTER TABLE playlists
  ALTER COLUMN id TYPE BIGINT,
  ALTER COLUMN user_id TYPE BIGINT,
  ALTER COLUMN created_at TYPE TIMESTAMP,
  ALTER COLUMN updated_at TYPE TIMESTAMP;

ALTER SEQUENCE playlists_id_seq AS BIGINT;

ALTER TABLE playlist_items
  ALTER COLUMN id TYPE BIGINT,
  ALTER COLUMN playlist_id TYPE BIGINT,
  ALTER COLUMN video_id TYPE BIGINT,
  ALTER COLUMN added_at TYPE TIMESTAMP;

ALTER SEQUENCE playlist_items_id_seq AS BIGINT;
