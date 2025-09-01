-- Migration: Add path restrictions to API keys
-- Description: Add fields to restrict API key access to specific paths and methods
-- Date: 2025-08-22

-- Add allowed_paths column to store path patterns that the API key can access
ALTER TABLE api_keys
ADD COLUMN IF NOT EXISTS allowed_paths TEXT[] DEFAULT ARRAY[]::TEXT[];

-- Add allowed_methods column to store HTTP methods that the API key can use
ALTER TABLE api_keys
ADD COLUMN IF NOT EXISTS allowed_methods TEXT[] DEFAULT ARRAY[]::TEXT[];

-- Add metadata column for additional configuration
ALTER TABLE api_keys
ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}'::JSONB;

-- Add comment for documentation
COMMENT ON COLUMN api_keys.allowed_paths IS 'Array of path patterns that this API key is allowed to access. Supports wildcards like /api/datasets/*';
COMMENT ON COLUMN api_keys.allowed_methods IS 'Array of HTTP methods that this API key is allowed to use (GET, POST, PUT, DELETE, etc.)';
COMMENT ON COLUMN api_keys.metadata IS 'Additional metadata and configuration for the API key';

-- Create index for better query performance
CREATE INDEX IF NOT EXISTS idx_api_keys_allowed_paths ON api_keys USING GIN (allowed_paths);
CREATE INDEX IF NOT EXISTS idx_api_keys_metadata ON api_keys USING GIN (metadata);

-- Update existing API keys to have default permissions (safe defaults)
-- Existing keys will need to be manually updated with specific permissions
UPDATE api_keys
SET
    allowed_paths = ARRAY['/api/datasets/*', '/api/algoexes/*', '/api/committee/*', '/api/ws']::TEXT[],
    allowed_methods = ARRAY['GET', 'POST', 'PUT', 'DELETE']::TEXT[],
    metadata = jsonb_build_object(
        'migration_version', '20250822',
        'default_permissions', true,
        'needs_review', true
    )
WHERE allowed_paths IS NULL OR array_length(allowed_paths, 1) IS NULL;

-- Create audit table for API key usage
CREATE TABLE IF NOT EXISTS api_key_usage_logs (
    id SERIAL PRIMARY KEY,
    api_key_id INTEGER NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    method TEXT NOT NULL,
    status_code INTEGER,
    response_time_ms INTEGER,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create index for audit queries
CREATE INDEX IF NOT EXISTS idx_api_key_usage_logs_api_key_id ON api_key_usage_logs(api_key_id);
CREATE INDEX IF NOT EXISTS idx_api_key_usage_logs_created_at ON api_key_usage_logs(created_at);

-- Add comment for audit table
COMMENT ON TABLE api_key_usage_logs IS 'Audit log for API key usage to track access patterns and detect anomalies';
