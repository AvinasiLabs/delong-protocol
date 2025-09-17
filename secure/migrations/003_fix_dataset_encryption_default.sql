-- Migration: Fix dataset encryption default value for security
-- Issue: Default should be true (encrypted) to prevent data exposure during creation

-- Change the default value of is_encrypted to true
ALTER TABLE dataset
ALTER COLUMN is_encrypted SET DEFAULT true;

-- Update any existing datasets that might have null values to be encrypted by default
UPDATE dataset
SET is_encrypted = true
WHERE is_encrypted IS NULL;

-- Add comment to document the security requirement
COMMENT ON COLUMN dataset.is_encrypted IS
'Encryption status of the dataset. Default is true for security. Must be explicitly set to false to make public.';