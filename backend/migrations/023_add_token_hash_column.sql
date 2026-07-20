ALTER TABLE auth_tokens ADD COLUMN IF NOT EXISTS token_hash VARCHAR(64);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'auth_tokens' AND column_name = 'token') THEN
    UPDATE auth_tokens SET token_hash = encode(sha256(token::bytea), 'hex') WHERE token_hash IS NULL;
  END IF;
END $$;

ALTER TABLE auth_tokens ALTER COLUMN token_hash SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_tokens_token_hash ON auth_tokens(token_hash);

DROP INDEX IF EXISTS idx_auth_tokens_token;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'auth_tokens' AND column_name = 'token') THEN
    ALTER TABLE auth_tokens DROP COLUMN token;
  END IF;
END $$;
