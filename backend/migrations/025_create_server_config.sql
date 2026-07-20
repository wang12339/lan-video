CREATE TABLE IF NOT EXISTS server_config (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO server_config (key, value) VALUES ('registration_enabled', 'false')
ON CONFLICT (key) DO NOTHING;
