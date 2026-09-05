-- 租户增强：添加缺失的配置和状态字段

-- 添加租户状态枚举（如果不存在）
DO $$ BEGIN
    CREATE TYPE tenant_status AS ENUM ('active', 'disabled', 'maintenance');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 添加缺失的列（使用 IF NOT EXISTS 避免重复）
ALTER TABLE tenants 
ADD COLUMN IF NOT EXISTS status tenant_status DEFAULT 'active',
ADD COLUMN IF NOT EXISTS settings JSONB DEFAULT '{}',
ADD COLUMN IF NOT EXISTS maintenance_eta TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS logo_url VARCHAR(512),
ADD COLUMN IF NOT EXISTS theme_color VARCHAR(7) DEFAULT '#1a1a2e';

-- 创建租户使用统计视图
CREATE OR REPLACE VIEW tenant_usage_stats AS
SELECT 
    t.id AS tenant_id,
    t.slug,
    t.name,
    t.status,
    t.max_users,
    t.max_storage_bytes,
    t.plan,
    COUNT(DISTINCT u.id) AS user_count,
    COUNT(DISTINCT v.id) AS video_count,
    COALESCE(SUM(v.file_size), 0) AS storage_used_bytes,
    CASE 
        WHEN t.max_storage_bytes > 0 
        THEN ROUND(COALESCE(SUM(v.file_size), 0)::NUMERIC / t.max_storage_bytes * 100, 2)
        ELSE 0 
    END AS storage_usage_percent
FROM tenants t
LEFT JOIN users u ON u.tenant_id = t.id
LEFT JOIN videos v ON v.tenant_id = t.id
GROUP BY t.id;

-- 租户活跃度索引
CREATE INDEX IF NOT EXISTS idx_videos_tenant_created 
ON videos(tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_users_tenant_created 
ON users(tenant_id, created_at DESC);

-- 更新触发器
CREATE OR REPLACE FUNCTION update_tenant_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$ BEGIN
    CREATE TRIGGER trigger_tenant_updated_at
    BEFORE UPDATE ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION update_tenant_updated_at();
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;
