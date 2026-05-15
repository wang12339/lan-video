CREATE TABLE IF NOT EXISTS videos (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    source_type VARCHAR(50) NOT NULL,
    cover_url TEXT,
    stream_url TEXT NOT NULL,
    category VARCHAR(100) NOT NULL DEFAULT 'general',
    file_hash VARCHAR(64),
    file_size BIGINT,
    original_name TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
