-- SECURITY (A07-07): Hash existing share_tokens the same way auth_tokens were
-- migrated in 023. After this migration, the raw token is never persisted.

ALTER TABLE share_links ADD COLUMN IF NOT EXISTS token_hash VARCHAR(64);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'share_links' AND column_name = 'token') THEN
    UPDATE share_links SET token_hash = encode(sha256(token::bytea), 'hex') WHERE token_hash IS NULL;
  END IF;
END $$;

-- Allow NULLs during the column-swap dance; the NOT NULL is set after backfill.
ALTER TABLE share_links ALTER COLUMN token_hash SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_share_links_token_hash ON share_links(token_hash);
DROP INDEX IF EXISTS idx_share_links_token;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'share_links' AND column_name = 'token') THEN
    ALTER TABLE share_links DROP COLUMN token;
  END IF;
END $$;
