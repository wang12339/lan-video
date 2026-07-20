-- Add role column to users table for permission levels
-- 0 = readonly (can login, view own profile, but no video access)
-- 1 = viewer (can login, view videos, no upload/delete)
-- 2 = editor (can upload, edit videos, no user management)
-- 3 = admin (full access)

ALTER TABLE users ADD COLUMN role SMALLINT NOT NULL DEFAULT 1;

-- Update existing admin user to role 3
UPDATE users SET role = 3 WHERE is_admin = true;

-- Update existing non-admin users to role 1 (viewer)
UPDATE users SET role = 1 WHERE is_admin = false;

-- Add index for role-based queries
CREATE INDEX idx_users_role ON users(role);