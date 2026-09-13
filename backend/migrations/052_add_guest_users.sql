-- 访客模式：匿名访客在 users 表中表现为 is_guest = true 的影子账号
-- （无密码、自动审批、role=1），凭 token cookie 维持会话。
-- 访客注册/登录真实账号后，其内容通过 merge 合并到真实账号，
-- 影子账号行被删除，因此常规路径下不会积累垃圾行。
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_guest BOOLEAN NOT NULL DEFAULT FALSE;

-- 部分索引：访客账号数量可能远大于真实账号（每浏览器一个），
-- 仅在 is_guest = true 的行上建索引，供清理/统计使用。
CREATE INDEX IF NOT EXISTS idx_users_tenant_guest ON users(tenant_id, is_guest) WHERE is_guest;
