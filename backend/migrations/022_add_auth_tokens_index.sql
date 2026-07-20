-- Add missing index on auth_tokens.user_id for faster user token lookups
-- Used in: has_active_tokens(), delete_tokens_by_user_id(), list_users() subquery
CREATE INDEX IF NOT EXISTS idx_auth_tokens_user_id ON auth_tokens(user_id, expires_at);
