-- Migration: 005_remove_auto_encryption_tags
-- Description: Remove automatic encryption tag generation and clean up existing tags

-- ============================================================================
-- 1. Drop the automatic encryption tag trigger
-- ============================================================================

DROP TRIGGER IF EXISTS auto_add_encryption_tag_trigger ON dataset;
DROP FUNCTION IF EXISTS auto_add_encryption_tag();

-- ============================================================================
-- 2. Remove all 'public' and 'encrypted' tags from dataset_tags
-- ============================================================================

DELETE FROM dataset_tags
WHERE tag IN ('public', 'encrypted');

-- ============================================================================
-- 3. Update comments to reflect the change
-- ============================================================================

COMMENT ON TABLE dataset_tags IS 'Tags associated with datasets';
COMMENT ON COLUMN dataset_tags.tag IS 'Tag name (e.g., "healthcare", "genomics", "csv")';

-- ============================================================================
-- Migration completed successfully
-- ============================================================================