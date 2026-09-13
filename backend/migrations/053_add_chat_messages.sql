-- 公共聊天室：消息持久化（多租户隔离）。
-- user_id 为 SET NULL：账号删除后公共聊天记录保留（仅丢失归属关联），
-- username 冗余存储用于展示（删号后仍能显示当时的发言者名字）。
CREATE TABLE IF NOT EXISTS chat_messages (
    id BIGSERIAL PRIMARY KEY,
    tenant_id BIGINT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    username VARCHAR(255) NOT NULL,
    content VARCHAR(500) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 历史分页按 (tenant_id, id DESC) 游标翻页
CREATE INDEX IF NOT EXISTS idx_chat_messages_tenant_id ON chat_messages(tenant_id, id DESC);
