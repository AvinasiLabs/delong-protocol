-- Migration: Rename data_usages table to data_usage
-- Purpose: Unify table naming convention (singular form)
-- Date: 2024

-- Rename the table
ALTER TABLE IF EXISTS data_usages RENAME TO data_usage;

-- Rename indexes
ALTER INDEX IF EXISTS idx_data_usages_scientist_wallet RENAME TO idx_data_usage_scientist_wallet;
ALTER INDEX IF EXISTS idx_data_usages_cid RENAME TO idx_data_usage_cid;
ALTER INDEX IF EXISTS idx_data_usages_dataset RENAME TO idx_data_usage_dataset;
ALTER INDEX IF EXISTS idx_used_at RENAME TO idx_data_usage_used_at;

-- Drop old trigger and recreate with new name
DROP TRIGGER IF EXISTS update_data_usages_updated_at ON data_usage;

CREATE TRIGGER update_data_usage_updated_at
    BEFORE UPDATE ON data_usage
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Update any foreign key constraints that might reference this table
-- (Currently none, but good practice to check)

-- Add a comment to track migration
COMMENT ON TABLE data_usage IS 'Records of data usage by scientists (renamed from data_usages)';
