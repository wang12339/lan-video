ALTER TABLE auth_tokens ADD COLUMN IF NOT EXISTS revoked BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX IF NOT EXISTS idx_auth_tokens_revoked ON auth_tokens(revoked) WHERE revoked = true;
