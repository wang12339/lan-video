ALTER TABLE users ADD COLUMN approved BOOLEAN NOT NULL DEFAULT false;

-- First user (admin) is auto-approved
UPDATE users SET approved = true WHERE is_admin = true;
