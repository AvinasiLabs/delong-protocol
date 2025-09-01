-- Complete database schema for delong-core
-- No foreign key constraints, proper NOT NULL constraints and defaults

-- Create enum type for rate limit tiers
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'rate_limit_tier') THEN
        CREATE TYPE rate_limit_tier AS ENUM ('basic', 'premium', 'enterprise');
    END IF;
END$$;

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255),
    role VARCHAR(50) NOT NULL DEFAULT 'scientist',
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    wallet_address VARCHAR(255),
    google_id VARCHAR(255) UNIQUE,
    avatar_url VARCHAR(500),
    provider VARCHAR(50) NOT NULL DEFAULT 'email',
    provider_data JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_login TIMESTAMP WITH TIME ZONE,
    last_provider_sync TIMESTAMP WITH TIME ZONE,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    two_factor_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    profile_data JSONB NOT NULL DEFAULT '{}'
);

-- Roles table
CREATE TABLE IF NOT EXISTS roles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    level INTEGER NOT NULL DEFAULT 1,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Permissions table
CREATE TABLE IF NOT EXISTS permissions (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    resource VARCHAR(100) NOT NULL,
    action VARCHAR(100) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Role permissions table (no foreign keys)
CREATE TABLE IF NOT EXISTS role_permissions (
    id SERIAL PRIMARY KEY,
    role_id INTEGER NOT NULL,
    permission_id INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(role_id, permission_id)
);

-- Verification codes table
CREATE TABLE IF NOT EXISTS verification_codes (
    id SERIAL PRIMARY KEY,
    user_email VARCHAR(255) NOT NULL,
    code VARCHAR(10) NOT NULL,
    verification_type VARCHAR(50) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    used_at TIMESTAMP WITH TIME ZONE,
    is_used BOOLEAN NOT NULL DEFAULT FALSE,
    attempts INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT verification_codes_email_type_unique UNIQUE (user_email, verification_type, code)
);

-- Audit logs table (no foreign keys)
CREATE TABLE IF NOT EXISTS audit_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER,
    action VARCHAR(255) NOT NULL,
    resource_type VARCHAR(100),
    resource_id VARCHAR(255),
    details JSONB NOT NULL DEFAULT '{}',
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Committee wallets table (no foreign keys)
CREATE TABLE IF NOT EXISTS committee_wallets (
    id SERIAL PRIMARY KEY,
    wallet_address VARCHAR(255) UNIQUE NOT NULL,
    user_id INTEGER NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    verification_tx VARCHAR(255),
    verified_at TIMESTAMP WITH TIME ZONE,
    verified_by INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- AI audit reports table
CREATE TABLE IF NOT EXISTS ai_audit_reports (
    id SERIAL PRIMARY KEY,
    algorithm_id INTEGER,
    execution_id INTEGER,
    github_url VARCHAR(500) NOT NULL,
    commit_hash VARCHAR(255) NOT NULL,
    repo_url VARCHAR(500) NOT NULL,
    audit_status VARCHAR(50) NOT NULL DEFAULT 'pending',
    audit_score INTEGER NOT NULL DEFAULT 0,
    audit_result JSONB NOT NULL DEFAULT '{}',
    raw_response JSONB NOT NULL DEFAULT '{}',
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP WITH TIME ZONE,
    UNIQUE(github_url, commit_hash)
);

-- OAuth providers table
CREATE TABLE IF NOT EXISTS oauth_providers (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    display_name VARCHAR(100) NOT NULL,
    client_id VARCHAR(255),
    client_secret VARCHAR(255),
    auth_url VARCHAR(500),
    token_url VARCHAR(500),
    user_info_url VARCHAR(500),
    scopes TEXT[],
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- User OAuth accounts table (no foreign keys)
CREATE TABLE IF NOT EXISTS user_oauth_accounts (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    provider_id INTEGER NOT NULL,
    provider_user_id VARCHAR(255) NOT NULL,
    provider_username VARCHAR(255),
    provider_email VARCHAR(255),
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at TIMESTAMP WITH TIME ZONE,
    provider_data JSONB NOT NULL DEFAULT '{}',
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(provider_id, provider_user_id),
    UNIQUE(user_id, provider_id)
);

-- API keys table (no foreign keys, proper defaults)
CREATE TABLE IF NOT EXISTS api_keys (
    id SERIAL PRIMARY KEY,
    api_key VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    user_id INTEGER NOT NULL,
    permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
    rate_limit_tier rate_limit_tier NOT NULL DEFAULT 'basic',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at TIMESTAMP WITH TIME ZONE,  -- Can be null for keys that never expire
    last_used_at TIMESTAMP WITH TIME ZONE,  -- Can be null if never used
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT unique_user_api_key_name UNIQUE (user_id, name)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role);
CREATE INDEX IF NOT EXISTS idx_users_status ON users(status);
CREATE INDEX IF NOT EXISTS idx_users_wallet_address ON users(wallet_address);
CREATE INDEX IF NOT EXISTS idx_users_google_id ON users(google_id);
CREATE INDEX IF NOT EXISTS idx_users_provider ON users(provider);

CREATE INDEX IF NOT EXISTS idx_verification_codes_email ON verification_codes(user_email);
CREATE INDEX IF NOT EXISTS idx_verification_codes_type ON verification_codes(verification_type);
CREATE INDEX IF NOT EXISTS idx_verification_codes_expires_at ON verification_codes(expires_at);
CREATE INDEX IF NOT EXISTS idx_verification_codes_is_used ON verification_codes(is_used);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at);

CREATE INDEX IF NOT EXISTS idx_ai_audit_reports_algorithm_id ON ai_audit_reports(algorithm_id);
CREATE INDEX IF NOT EXISTS idx_ai_audit_reports_execution_id ON ai_audit_reports(execution_id);
CREATE INDEX IF NOT EXISTS idx_ai_audit_reports_status ON ai_audit_reports(audit_status);
CREATE INDEX IF NOT EXISTS idx_ai_audit_reports_created_at ON ai_audit_reports(created_at);
CREATE INDEX IF NOT EXISTS idx_ai_audit_reports_github_commit ON ai_audit_reports(github_url, commit_hash);

CREATE INDEX IF NOT EXISTS idx_user_oauth_accounts_user_id ON user_oauth_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_user_oauth_accounts_provider_id ON user_oauth_accounts(provider_id);
CREATE INDEX IF NOT EXISTS idx_user_oauth_accounts_provider_user_id ON user_oauth_accounts(provider_user_id);
CREATE INDEX IF NOT EXISTS idx_user_oauth_accounts_provider_email ON user_oauth_accounts(provider_email);

CREATE INDEX IF NOT EXISTS idx_api_keys_api_key ON api_keys(api_key);
CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_is_active ON api_keys(is_active);
CREATE INDEX IF NOT EXISTS idx_api_keys_expires_at ON api_keys(expires_at);
CREATE INDEX IF NOT EXISTS idx_api_keys_last_used_at ON api_keys(last_used_at);
CREATE INDEX IF NOT EXISTS idx_api_keys_rate_limit_tier ON api_keys(rate_limit_tier);
CREATE INDEX IF NOT EXISTS idx_api_keys_created_at ON api_keys(created_at);

-- Insert default roles
INSERT INTO roles (name, display_name, description, level, is_system) VALUES
('scientist', '科学家', '可以上传算法、查看数据集、生成报告', 1, true),
('committee', '委员会成员', '可以提交算法、审核算法、管理数据集、查看所有报告', 2, true),
('admin', '系统管理员', '拥有所有权限，可以管理用户和系统', 3, true)
ON CONFLICT (name) DO NOTHING;

-- Insert Google OAuth provider
INSERT INTO oauth_providers (name, display_name, auth_url, token_url, user_info_url, scopes) VALUES
('google', 'Google',
 'https://accounts.google.com/o/oauth2/v2/auth',
 'https://oauth2.googleapis.com/token',
 'https://www.googleapis.com/oauth2/v2/userinfo',
 ARRAY['https://www.googleapis.com/auth/userinfo.email', 'https://www.googleapis.com/auth/userinfo.profile'])
ON CONFLICT (name) DO NOTHING;

-- Create function to update updated_at column
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create function to generate secure API keys
CREATE OR REPLACE FUNCTION generate_api_key() RETURNS VARCHAR AS $$
DECLARE
    chars TEXT := 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    result VARCHAR := 'dl_';  -- prefix for delong API keys
    i INTEGER;
BEGIN
    -- Generate a 32-character random string after the prefix
    FOR i IN 1..32 LOOP
        result := result || substr(chars, floor(random() * length(chars) + 1)::INTEGER, 1);
    END LOOP;
    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for updated_at columns
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_roles_updated_at ON roles;
CREATE TRIGGER update_roles_updated_at BEFORE UPDATE ON roles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_committee_wallets_updated_at ON committee_wallets;
CREATE TRIGGER update_committee_wallets_updated_at BEFORE UPDATE ON committee_wallets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_ai_audit_reports_updated_at ON ai_audit_reports;
CREATE TRIGGER update_ai_audit_reports_updated_at BEFORE UPDATE ON ai_audit_reports
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_oauth_providers_updated_at ON oauth_providers;
CREATE TRIGGER update_oauth_providers_updated_at BEFORE UPDATE ON oauth_providers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_user_oauth_accounts_updated_at ON user_oauth_accounts;
CREATE TRIGGER update_user_oauth_accounts_updated_at BEFORE UPDATE ON user_oauth_accounts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_api_keys_updated_at ON api_keys;
CREATE TRIGGER update_api_keys_updated_at BEFORE UPDATE ON api_keys
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create view for API key statistics (without joins)
CREATE OR REPLACE VIEW api_key_stats AS
SELECT
    ak.id,
    ak.name,
    ak.user_id,
    ak.rate_limit_tier,
    ak.is_active,
    ak.expires_at,
    ak.created_at,
    ak.last_used_at,
    CASE
        WHEN ak.expires_at IS NULL THEN 'never'
        WHEN ak.expires_at < NOW() THEN 'expired'
        WHEN ak.expires_at < NOW() + INTERVAL '7 days' THEN 'expiring_soon'
        ELSE 'valid'
    END as expiry_status,
    CASE
        WHEN ak.last_used_at IS NULL THEN -1
        ELSE EXTRACT(EPOCH FROM (NOW() - ak.last_used_at)) / 86400
    END::INTEGER as days_since_last_use,
    ak.permissions,
    jsonb_array_length(ak.permissions) as permission_count
FROM api_keys ak;

-- Migration completed
SELECT 'Database schema created successfully' as status;
