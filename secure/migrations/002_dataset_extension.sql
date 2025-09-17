-- Migration: 002_dataset_extension
-- Description: Simplified extension for dataset module with minimal statistics and automatic Featured/Trending

-- ============================================================================
-- 1. Extend dataset table with essential fields
-- ============================================================================

-- Add author_id to link with user table (no foreign key)
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS author_id BIGINT;

-- Add mandatory encryption flag
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS is_encrypted BOOLEAN NOT NULL DEFAULT false;

-- Add slug for URL-friendly identifiers
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS slug VARCHAR(255) UNIQUE;

-- Add category and display fields
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS category VARCHAR(100);
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS license VARCHAR(100) DEFAULT 'Custom';
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS thumbnail_url TEXT;
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS version VARCHAR(50) DEFAULT 'v1.0';

-- Add extended description in Markdown format
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS detailed_desc TEXT;

-- Add views counter for Trending calculation
ALTER TABLE dataset ADD COLUMN IF NOT EXISTS views_count BIGINT DEFAULT 0;

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_dataset_slug ON dataset(slug);
CREATE INDEX IF NOT EXISTS idx_dataset_author_id ON dataset(author_id);
CREATE INDEX IF NOT EXISTS idx_dataset_is_encrypted ON dataset(is_encrypted);
CREATE INDEX IF NOT EXISTS idx_dataset_category ON dataset(category);
CREATE INDEX IF NOT EXISTS idx_dataset_views_count ON dataset(views_count DESC);
CREATE INDEX IF NOT EXISTS idx_dataset_created_at_views ON dataset(created_at DESC, views_count DESC);

-- Add comments for documentation
COMMENT ON COLUMN dataset.author_id IS 'User ID of the dataset creator (soft reference to user table)';
COMMENT ON COLUMN dataset.is_encrypted IS 'Whether the dataset is encrypted (mandatory field)';
COMMENT ON COLUMN dataset.slug IS 'URL-friendly unique identifier for the dataset';
COMMENT ON COLUMN dataset.category IS 'Dataset category (e.g., Healthcare, Finance, Computer Vision)';
COMMENT ON COLUMN dataset.license IS 'License type for the dataset';
COMMENT ON COLUMN dataset.thumbnail_url IS 'URL to thumbnail image for display';
COMMENT ON COLUMN dataset.version IS 'Version number of the dataset';
COMMENT ON COLUMN dataset.detailed_desc IS 'Detailed description in Markdown format';
COMMENT ON COLUMN dataset.views_count IS 'Number of views (used for Trending calculation)';

-- ============================================================================
-- 2. Create dataset_tags table for tagging system
-- ============================================================================

CREATE TABLE IF NOT EXISTS dataset_tags (
    id BIGSERIAL PRIMARY KEY,
    dataset_id BIGINT NOT NULL REFERENCES dataset(id) ON DELETE CASCADE,
    tag VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(dataset_id, tag)
);

-- Create indexes for dataset_tags
CREATE INDEX IF NOT EXISTS idx_dataset_tags_dataset_id ON dataset_tags(dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_tags_tag ON dataset_tags(tag);

-- Add comments
COMMENT ON TABLE dataset_tags IS 'Tags associated with datasets (must include encryption status tag)';
COMMENT ON COLUMN dataset_tags.tag IS 'Tag name (e.g., "encrypted", "public", "healthcare", "csv")';

-- ============================================================================
-- 3. Create simplified dataset_schema table
-- ============================================================================

CREATE TABLE IF NOT EXISTS dataset_schema (
    id BIGSERIAL PRIMARY KEY,
    dataset_id BIGINT NOT NULL REFERENCES dataset(id) ON DELETE CASCADE,
    field_name VARCHAR(255) NOT NULL,
    field_type VARCHAR(100) NOT NULL,
    description TEXT,
    is_nullable BOOLEAN DEFAULT true,
    field_order INTEGER,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(dataset_id, field_name)
);

-- Create indexes for dataset_schema
CREATE INDEX IF NOT EXISTS idx_dataset_schema_dataset_id ON dataset_schema(dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_schema_field_order ON dataset_schema(dataset_id, field_order);

-- Add comments
COMMENT ON TABLE dataset_schema IS 'Simplified schema information for dataset fields';
COMMENT ON COLUMN dataset_schema.field_type IS 'Simple data type (string, number, boolean, date)';
COMMENT ON COLUMN dataset_schema.field_order IS 'Display order for the field';

-- ============================================================================
-- 4. Extend data_usage table for better tracking
-- ============================================================================

ALTER TABLE data_usage ADD COLUMN IF NOT EXISTS user_id BIGINT;
ALTER TABLE data_usage ADD COLUMN IF NOT EXISTS algo_name VARCHAR(255);
ALTER TABLE data_usage ADD COLUMN IF NOT EXISTS execution_status VARCHAR(50);
ALTER TABLE data_usage ADD COLUMN IF NOT EXISTS runtime_seconds INTEGER;
ALTER TABLE data_usage ADD COLUMN IF NOT EXISTS records_processed BIGINT;

-- Create additional indexes for data_usage
CREATE INDEX IF NOT EXISTS idx_data_usage_dataset_status ON data_usage(dataset, execution_status);
CREATE INDEX IF NOT EXISTS idx_data_usage_user_id ON data_usage(user_id);
CREATE INDEX IF NOT EXISTS idx_data_usage_used_at_desc ON data_usage(used_at DESC);

-- Add comments for new columns
COMMENT ON COLUMN data_usage.user_id IS 'User ID who executed the algorithm';
COMMENT ON COLUMN data_usage.algo_name IS 'Name of the algorithm used';
COMMENT ON COLUMN data_usage.execution_status IS 'Status of the execution (completed, failed, etc.)';
COMMENT ON COLUMN data_usage.runtime_seconds IS 'Execution time in seconds';
COMMENT ON COLUMN data_usage.records_processed IS 'Number of records processed';

-- ============================================================================
-- 5. Create helper functions
-- ============================================================================

-- Function to generate slug from dataset name
CREATE OR REPLACE FUNCTION generate_dataset_slug(dataset_name TEXT)
RETURNS TEXT AS $$
BEGIN
    -- Convert to lowercase, replace spaces with hyphens, remove special characters
    RETURN LOWER(
        REGEXP_REPLACE(
            REGEXP_REPLACE(dataset_name, '[^a-zA-Z0-9\s-]', '', 'g'),
            '\s+',
            '-',
            'g'
        )
    );
END;
$$ LANGUAGE plpgsql;

-- Function to get featured datasets (based on usage)
CREATE OR REPLACE FUNCTION get_featured_datasets()
RETURNS TABLE(dataset_id BIGINT, usage_count BIGINT) AS $$
BEGIN
    RETURN QUERY
    WITH usage_stats AS (
        SELECT
            d.id as dataset_id,
            COUNT(DISTINCT du.id) as usage_count
        FROM dataset d
        LEFT JOIN data_usage du ON du.dataset = d.name
        WHERE du.execution_status = 'completed'
            AND du.used_at > NOW() - INTERVAL '30 days'
        GROUP BY d.id
    )
    SELECT us.dataset_id, us.usage_count
    FROM usage_stats us
    ORDER BY us.usage_count DESC
    LIMIT 6;
END;
$$ LANGUAGE plpgsql;

-- Function to get trending datasets (based on views)
CREATE OR REPLACE FUNCTION get_trending_datasets()
RETURNS TABLE(dataset_id BIGINT, views BIGINT) AS $$
BEGIN
    RETURN QUERY
    SELECT
        d.id as dataset_id,
        d.views_count as views
    FROM dataset d
    WHERE d.views_count > 0
        AND d.created_at > NOW() - INTERVAL '7 days'
    ORDER BY d.views_count DESC
    LIMIT 6;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 6. Create triggers for automatic updates
-- ============================================================================

-- Trigger to auto-generate slug for new datasets
CREATE OR REPLACE FUNCTION auto_generate_slug()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.slug IS NULL OR NEW.slug = '' THEN
        NEW.slug := generate_dataset_slug(NEW.name);

        -- Ensure uniqueness by appending number if needed
        DECLARE
            counter INTEGER := 1;
            temp_slug TEXT;
        BEGIN
            temp_slug := NEW.slug;
            WHILE EXISTS (SELECT 1 FROM dataset WHERE slug = NEW.slug AND id != NEW.id) LOOP
                NEW.slug := temp_slug || '-' || counter;
                counter := counter + 1;
            END LOOP;
        END;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER auto_generate_dataset_slug
    BEFORE INSERT OR UPDATE ON dataset
    FOR EACH ROW
    EXECUTE FUNCTION auto_generate_slug();

-- Trigger to automatically add encryption tag based on is_encrypted field
CREATE OR REPLACE FUNCTION auto_add_encryption_tag()
RETURNS TRIGGER AS $$
BEGIN
    -- Remove existing encryption tags
    DELETE FROM dataset_tags
    WHERE dataset_id = NEW.id
        AND tag IN ('encrypted', 'public');

    -- Add appropriate tag based on encryption status
    IF NEW.is_encrypted THEN
        INSERT INTO dataset_tags (dataset_id, tag)
        VALUES (NEW.id, 'encrypted')
        ON CONFLICT (dataset_id, tag) DO NOTHING;
    ELSE
        INSERT INTO dataset_tags (dataset_id, tag)
        VALUES (NEW.id, 'public')
        ON CONFLICT (dataset_id, tag) DO NOTHING;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER auto_add_encryption_tag_trigger
    AFTER INSERT OR UPDATE OF is_encrypted ON dataset
    FOR EACH ROW
    EXECUTE FUNCTION auto_add_encryption_tag();

-- ============================================================================
-- 7. Data migration for existing datasets
-- ============================================================================

-- Generate slugs for existing datasets
UPDATE dataset
SET slug = generate_dataset_slug(name)
WHERE slug IS NULL;

-- Handle duplicate slugs by appending numbers
DO $$
DECLARE
    rec RECORD;
    counter INTEGER;
    new_slug TEXT;
BEGIN
    FOR rec IN
        SELECT slug, COUNT(*) as cnt
        FROM dataset
        WHERE slug IS NOT NULL
        GROUP BY slug
        HAVING COUNT(*) > 1
    LOOP
        counter := 0;
        FOR rec IN
            SELECT id, slug
            FROM dataset
            WHERE slug = rec.slug
            ORDER BY created_at
        LOOP
            IF counter > 0 THEN
                new_slug := rec.slug || '-' || counter;
                UPDATE dataset SET slug = new_slug WHERE id = rec.id;
            END IF;
            counter := counter + 1;
        END LOOP;
    END LOOP;
END $$;

-- Set default categories for existing datasets based on file format
UPDATE dataset
SET category = CASE
    WHEN file_format IN ('CSV', 'TSV') THEN 'Tabular'
    WHEN file_format IN ('JSON', 'JSONL') THEN 'Structured'
    WHEN file_format IN ('Parquet', 'HDF5') THEN 'Big Data'
    ELSE 'General'
END
WHERE category IS NULL;

-- Initialize views_count to 0 for all existing datasets
UPDATE dataset
SET views_count = 0
WHERE views_count IS NULL;

-- Add encryption tags for existing datasets based on their current state
INSERT INTO dataset_tags (dataset_id, tag)
SELECT id, CASE WHEN is_encrypted THEN 'encrypted' ELSE 'public' END
FROM dataset
ON CONFLICT (dataset_id, tag) DO NOTHING;

-- ============================================================================
-- 8. Create views for easy querying
-- ============================================================================

-- View for featured datasets with full information
CREATE OR REPLACE VIEW featured_datasets AS
WITH usage_stats AS (
    SELECT
        d.id,
        COUNT(DISTINCT du.id) as usage_count
    FROM dataset d
    LEFT JOIN data_usage du ON du.dataset = d.name
    WHERE du.execution_status = 'completed'
        AND du.used_at > NOW() - INTERVAL '30 days'
    GROUP BY d.id
)
SELECT
    d.*,
    COALESCE(us.usage_count, 0) as usage_count
FROM dataset d
LEFT JOIN usage_stats us ON us.id = d.id
ORDER BY usage_count DESC
LIMIT 6;

-- View for trending datasets with full information
CREATE OR REPLACE VIEW trending_datasets AS
SELECT
    d.*
FROM dataset d
WHERE d.views_count > 0
    AND d.created_at > NOW() - INTERVAL '7 days'
ORDER BY d.views_count DESC
LIMIT 6;

-- ============================================================================
-- Migration completed successfully
-- ============================================================================

-- Grant appropriate permissions (adjust based on your user setup)
-- GRANT SELECT ON ALL TABLES IN SCHEMA public TO your_app_user;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO your_app_user;
