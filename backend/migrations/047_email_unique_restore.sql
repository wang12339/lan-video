-- 恢复邮箱唯一约束
-- 041/042 的索引清理误删了 idx_users_email_unique（email 唯一性属于功能约束，
-- 不是冗余索引）。handler/服务层仍按“邮箱唯一”实现 409 冲突语义，此处还原。
--
-- 清理步骤：若约束缺失期间产生了重复邮箱，保留最早绑定的用户，其余置空。

-- 1) 重复邮箱：保留 id 最小（最早注册）的记录，其余置 NULL
WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (PARTITION BY email ORDER BY id) AS rn
    FROM users
    WHERE email IS NOT NULL AND email <> ''
)
UPDATE users
SET email = NULL
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

-- 2) 重建唯一索引
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_unique ON users(email) WHERE email IS NOT NULL;