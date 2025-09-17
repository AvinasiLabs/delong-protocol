-- Migration: Remove category field from dataset table
-- Reason: Using tags system instead of fixed categories

-- First, check and drop dependent views if they exist
DROP VIEW IF EXISTS featured_datasets CASCADE;
DROP VIEW IF EXISTS trending_datasets CASCADE;

-- Drop any functions that might use category
DROP FUNCTION IF EXISTS get_datasets_by_category CASCADE;
DROP FUNCTION IF EXISTS search_datasets CASCADE;

-- Now we can safely drop the category column
ALTER TABLE dataset DROP COLUMN IF EXISTS category CASCADE;

-- Remove the index on category
DROP INDEX IF EXISTS idx_dataset_category;

-- Recreate views without category field
-- Featured datasets view (based on usage count)
-- Note: data_usage table uses 'dataset' column (string), not dataset_id
CREATE OR REPLACE VIEW featured_datasets AS
SELECT
    d.*,
    COUNT(DISTINCT du.id) as usage_count
FROM dataset d
LEFT JOIN data_usage du ON d.name = du.dataset  -- Fixed: use name, not id
GROUP BY d.id
ORDER BY usage_count DESC;

-- Trending datasets view (based on views in last 7 days)
CREATE OR REPLACE VIEW trending_datasets AS
SELECT
    d.*
FROM dataset d
WHERE d.created_at >= CURRENT_DATE - INTERVAL '7 days'
    OR d.updated_at >= CURRENT_DATE - INTERVAL '7 days'
ORDER BY d.views_count DESC;

-- Recreate search function without category but with tags
CREATE OR REPLACE FUNCTION search_datasets(
    search_keyword TEXT DEFAULT NULL,
    search_tags TEXT[] DEFAULT NULL,
    tag_mode TEXT DEFAULT 'any'
) RETURNS SETOF dataset AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT d.*
    FROM dataset d
    LEFT JOIN dataset_tags dt ON d.id = dt.dataset_id
    WHERE (
        search_keyword IS NULL
        OR d.name ILIKE '%' || search_keyword || '%'
        OR d.description ILIKE '%' || search_keyword || '%'
        OR d.detailed_desc ILIKE '%' || search_keyword || '%'
    )
    AND (
        search_tags IS NULL
        OR (
            tag_mode = 'any' AND dt.tag = ANY(search_tags)
        )
        OR (
            tag_mode = 'all' AND d.id IN (
                SELECT dataset_id
                FROM dataset_tags
                WHERE tag = ANY(search_tags)
                GROUP BY dataset_id
                HAVING COUNT(DISTINCT tag) = array_length(search_tags, 1)
            )
        )
    );
END;
$$ LANGUAGE plpgsql;

-- Create function to get datasets by tags (replaces category filter)
CREATE OR REPLACE FUNCTION get_datasets_by_tags(
    filter_tags TEXT[],
    tag_mode TEXT DEFAULT 'any'
) RETURNS SETOF dataset AS $$
BEGIN
    IF tag_mode = 'any' THEN
        -- Return datasets that have ANY of the specified tags
        RETURN QUERY
        SELECT DISTINCT d.*
        FROM dataset d
        JOIN dataset_tags dt ON d.id = dt.dataset_id
        WHERE dt.tag = ANY(filter_tags);
    ELSE
        -- Return datasets that have ALL of the specified tags
        RETURN QUERY
        SELECT d.*
        FROM dataset d
        WHERE d.id IN (
            SELECT dataset_id
            FROM dataset_tags
            WHERE tag = ANY(filter_tags)
            GROUP BY dataset_id
            HAVING COUNT(DISTINCT tag) = array_length(filter_tags, 1)
        );
    END IF;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_datasets_by_tags IS 'Get datasets filtered by tags, replacing the old category-based filtering';