CREATE TABLE IF NOT EXISTS tenants (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) NOT NULL UNIQUE,
    custom_domain VARCHAR(255),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    max_users INTEGER NOT NULL DEFAULT 10,
    max_storage_bytes BIGINT NOT NULL DEFAULT 53687091200,
    plan VARCHAR(50) NOT NULL DEFAULT 'free',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tenants_slug ON tenants(slug);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tenants_custom_domain ON tenants(custom_domain) WHERE custom_domain IS NOT NULL;

INSERT INTO tenants (name, slug, plan, max_users, max_storage_bytes) VALUES ('Default', 'default', 'enterprise', 999999, 1099511627776);
