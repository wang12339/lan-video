-- SECURITY (A04 H2): per-user upload quota tracking.
-- A new column on the users table is the simplest way to track lifetime upload
-- bytes; a separate table would be more flexible (e.g. per-month quota) but
-- would require a sweep job. The byte counter is incremented on successful
-- upload finalisation and decremented on delete.
ALTER TABLE users ADD COLUMN IF NOT EXISTS storage_used_bytes BIGINT NOT NULL DEFAULT 0;
