-- 优化租户表查询性能的索引
-- 确保 slug 和 custom_domain 查询的高效性

-- 1. 确保 slug 索引存在（部分索引，只索引活跃租户）
CREATE INDEX IF NOT EXISTS idx_tenants_slug_active ON tenants(slug) WHERE is_active = TRUE;

-- 2. 确保 custom_domain 索引存在（部分索引，只索引活跃租户且 domain 不为空）
CREATE INDEX IF NOT EXISTS idx_tenants_custom_domain_active ON tenants(custom_domain) 
WHERE is_active = TRUE AND custom_domain IS NOT NULL;

-- 3. 复合索引：优化 resolve_from_host 中的批量查询
CREATE INDEX IF NOT EXISTS idx_tenants_active_slug_domain ON tenants(is_active, slug, custom_domain);

-- 4. 统计信息更新（可选，但有助于查询规划器做出更好的决策）
ANALYZE tenants;