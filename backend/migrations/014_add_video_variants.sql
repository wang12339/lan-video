-- Video variants table for multi-resolution transcoding
CREATE TABLE video_variants (
    id SERIAL PRIMARY KEY,
    video_id INTEGER REFERENCES videos(id) ON DELETE CASCADE,
    resolution VARCHAR(20) NOT NULL, -- '1080p', '720p', '480p', '360p'
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    bitrate INTEGER, -- kbps
    codec VARCHAR(20) DEFAULT 'h264',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(video_id, resolution)
);

CREATE INDEX idx_video_variants_video_id ON video_variants(video_id);
CREATE INDEX idx_video_variants_resolution ON video_variants(resolution);

-- Transcoding jobs table for tracking background tasks
CREATE TABLE transcoding_jobs (
    id SERIAL PRIMARY KEY,
    video_id INTEGER REFERENCES videos(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- 'pending', 'processing', 'completed', 'failed'
    resolution VARCHAR(20) NOT NULL,
    progress INTEGER DEFAULT 0, -- 0-100
    error_message TEXT,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(video_id, resolution)
);

CREATE INDEX idx_transcoding_jobs_video_id ON transcoding_jobs(video_id);
CREATE INDEX idx_transcoding_jobs_status ON transcoding_jobs(status);

-- Add original_video_id to videos table for tracking transcoded versions
ALTER TABLE videos ADD COLUMN has_variants BOOLEAN DEFAULT FALSE;