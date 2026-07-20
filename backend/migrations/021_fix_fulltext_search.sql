-- Fix full-text search: change dictionary from 'simple' to 'chinese'
-- The previous migration (016) used 'simple' for the trigger but 'chinese' in queries,
-- causing the search to not work correctly for Chinese text.

CREATE OR REPLACE FUNCTION update_video_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector := 
        setweight(to_tsvector('chinese', coalesce(NEW.title, '')), 'A') ||
        setweight(to_tsvector('chinese', coalesce(NEW.description, '')), 'B') ||
        setweight(to_tsvector('chinese', coalesce(NEW.category, '')), 'C');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Rebuild search vectors for existing videos using the correct dictionary
UPDATE videos SET search_vector = 
    setweight(to_tsvector('chinese', coalesce(title, '')), 'A') ||
    setweight(to_tsvector('chinese', coalesce(description, '')), 'B') ||
    setweight(to_tsvector('chinese', coalesce(category, '')), 'C');
