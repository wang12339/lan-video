-- 套餐管理表
CREATE TABLE IF NOT EXISTS plans (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    slug VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    max_users INTEGER NOT NULL DEFAULT 10,
    max_storage_bytes BIGINT NOT NULL DEFAULT 53687091200, -- 50 GB
    max_upload_size_mb INTEGER NOT NULL DEFAULT 500,
    max_videos_per_user INTEGER NOT NULL DEFAULT 1000,
    storage_quota_gb INTEGER NOT NULL DEFAULT 50,
    registration_enabled BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 添加更新时间触发器
CREATE TRIGGER trigger_plans_updated_at
    BEFORE UPDATE ON plans
    FOR EACH ROW
    EXECUTE FUNCTION update_tenant_updated_at();

-- 插入默认套餐
INSERT INTO plans (name, slug, description, max_users, max_storage_bytes, max_upload_size_mb, max_videos_per_user, storage_quota_gb, registration_enabled, sort_order)
VALUES 
    ('免费版', 'free', '适合个人用户', 10, 53687091200, 500, 1000, 50, false, 1),
    ('基础版', 'basic', '适合小型团队', 50, 107374182400, 1000, 5000, 100, true, 2),
    ('专业版', 'pro', '适合中型企业', 200, 536870912000, 2000, 20000, 500, true, 3),
    ('企业版', 'enterprise', '适合大型企业', 1000, 1099511627776, 5000, 100000, 1000, true, 4)
ON CONFLICT (slug) DO NOTHING;

-- 为 tenants 表添加 plan_id 字段
ALTER TABLE tenants ADD COLUMN IF NOT EXISTS plan_id BIGINT REFERENCES plans(id);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_tenants_plan_id ON tenants(plan_id) WHERE plan_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_plans_slug ON plans(slug) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_plans_sort_order ON plans(sort_order);
