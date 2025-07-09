-- Create API keys table for third-party developer authentication
CREATE TABLE api_keys (
    id SERIAL PRIMARY KEY,
    api_key VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    permissions JSONB DEFAULT '[]'::jsonb,
    rate_limit_tier VARCHAR(50) DEFAULT 'basic',
    is_active BOOLEAN DEFAULT TRUE,
    expires_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for performance
CREATE INDEX idx_api_keys_api_key ON api_keys(api_key);
CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX idx_api_keys_is_active ON api_keys(is_active);
CREATE INDEX idx_api_keys_expires_at ON api_keys(expires_at);
CREATE INDEX idx_api_keys_last_used_at ON api_keys(last_used_at);

-- Create trigger to update updated_at column
CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON api_keys
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create enum type for rate limit tiers
CREATE TYPE rate_limit_tier AS ENUM ('basic', 'premium', 'enterprise');

-- Update api_keys table to use the enum
ALTER TABLE api_keys ALTER COLUMN rate_limit_tier TYPE rate_limit_tier USING rate_limit_tier::rate_limit_tier;

-- Insert some default permissions for reference
INSERT INTO permissions (name, display_name, description, resource, action) VALUES
('api_key.read', 'Read API Keys', 'Can read own API keys', 'api_key', 'read'),
('api_key.create', 'Create API Keys', 'Can create new API keys', 'api_key', 'create'),
('api_key.update', 'Update API Keys', 'Can update own API keys', 'api_key', 'update'),
('api_key.delete', 'Delete API Keys', 'Can delete own API keys', 'api_key', 'delete'),
('api_key.admin', 'Admin API Keys', 'Can manage all API keys', 'api_key', 'admin'),
('user.read', 'Read Users', 'Can read user information', 'user', 'read'),
('user.create', 'Create Users', 'Can create new users', 'user', 'create'),
('user.update', 'Update Users', 'Can update user information', 'user', 'update'),
('user.delete', 'Delete Users', 'Can delete users', 'user', 'delete'),
('dataset.read', 'Read Datasets', 'Can read dataset information', 'dataset', 'read'),
('dataset.create', 'Create Datasets', 'Can create new datasets', 'dataset', 'create'),
('dataset.update', 'Update Datasets', 'Can update dataset information', 'dataset', 'update'),
('dataset.delete', 'Delete Datasets', 'Can delete datasets', 'dataset', 'delete'),
('audit.read', 'Read Audit Reports', 'Can read audit reports', 'audit', 'read'),
('audit.create', 'Create Audit Reports', 'Can create audit reports', 'audit', 'create'),
('vote.read', 'Read Votes', 'Can read vote information', 'vote', 'read'),
('vote.create', 'Create Votes', 'Can create votes', 'vote', 'create');

-- Add API key permissions to roles
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'admin' AND p.name IN ('api_key.admin', 'user.read', 'user.create', 'user.update', 'user.delete');

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'scientist' AND p.name IN ('api_key.read', 'api_key.create', 'api_key.update', 'api_key.delete', 'dataset.read', 'audit.read');

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'committee' AND p.name IN ('api_key.read', 'api_key.create', 'api_key.update', 'api_key.delete', 'dataset.read', 'dataset.create', 'audit.read', 'audit.create', 'vote.read', 'vote.create');
