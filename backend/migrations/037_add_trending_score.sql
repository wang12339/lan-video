-- Add pre-computed trending score column for fast ORDER BY
ALTER TABLE videos ADD COLUMN IF NOT EXISTS trending_score FLOAT DEFAULT 0;

-- Index for fast trending queries
CREATE INDEX IF NOT EXISTS idx_videos_trending_score ON videos(trending_score DESC);

-- Function to calculate trending score
CREATE OR REPLACE FUNCTION calculate_trending_score(
    views BIGINT,
    created_at TIMESTAMP WITH TIME ZONE
) RETURNS FLOAT AS $$
DECLARE
    age_days FLOAT;
    score FLOAT;
BEGIN
    IF created_at IS NULL THEN
        RETURN 0;
    END IF;
    
    age_days := GREATEST(EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - created_at)) / 86400, 0.1);
    
    score := (LN(GREATEST(views, 0) + 1) * 100
              + GREATEST(views, 0) / age_days
              + CASE WHEN age_days < 7 THEN 30 ELSE 0 END
             ) / POWER(age_days + 2, 0.4);
    
    RETURN score;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Update existing videos
UPDATE videos SET trending_score = calculate_trending_score(views, created_at);

-- Trigger to auto-update on INSERT/UPDATE
CREATE OR REPLACE FUNCTION update_trending_score()
RETURNS TRIGGER AS $$
BEGIN
    NEW.trending_score := calculate_trending_score(NEW.views, NEW.created_at);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS videos_trending_score_update ON videos;
CREATE TRIGGER videos_trending_score_update
    BEFORE INSERT OR UPDATE OF views, created_at ON videos
    FOR EACH ROW EXECUTE FUNCTION update_trending_score();
