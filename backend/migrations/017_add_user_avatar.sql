-- Add avatar support to users
ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_url TEXT;

-- Store avatars in media/avatars/ directory
-- Avatar file naming: {user_id}.{ext}
