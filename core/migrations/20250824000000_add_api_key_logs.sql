-- Create api_key_logs table for auditing API key usage
CREATE TABLE IF NOT EXISTS api_key_logs (
    id SERIAL PRIMARY KEY,
    api_key_id INTEGER NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    path VARCHAR(255) NOT NULL,
    method VARCHAR(10) NOT NULL,
    status_code INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_api_key_logs_api_key_id ON api_key_logs(api_key_id);
CREATE INDEX IF NOT EXISTS idx_api_key_logs_created_at ON api_key_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_api_key_logs_api_key_created ON api_key_logs(api_key_id, created_at DESC);

-- Add comment to table
COMMENT ON TABLE api_key_logs IS 'Audit log for API key usage';
COMMENT ON COLUMN api_key_logs.api_key_id IS 'Foreign key to api_keys table';
COMMENT ON COLUMN api_key_logs.path IS 'API endpoint path accessed';
COMMENT ON COLUMN api_key_logs.method IS 'HTTP method used (GET, POST, etc.)';
COMMENT ON COLUMN api_key_logs.status_code IS 'HTTP response status code';
COMMENT ON COLUMN api_key_logs.created_at IS 'Timestamp of the API call';
