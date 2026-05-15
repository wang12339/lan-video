CREATE TABLE IF NOT EXISTS playback_history (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL DEFAULT 'demo',
    video_id BIGINT NOT NULL,
    position_ms BIGINT NOT NULL,
    duration_ms BIGINT NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(username, video_id)
);
