-- DeLong Protocol Database Initialization Script
-- This script is run when the PostgreSQL container starts for the first time

-- Create the database if it doesn't exist
-- (This is automatically done by the POSTGRES_DB environment variable)

-- Create additional users if needed
-- CREATE USER delong_readonly WITH PASSWORD 'readonly_password';
-- GRANT CONNECT ON DATABASE delong TO delong_readonly;
-- GRANT USAGE ON SCHEMA public TO delong_readonly;
-- GRANT SELECT ON ALL TABLES IN SCHEMA public TO delong_readonly;
-- ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO delong_readonly;

-- Create extensions if needed
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Set timezone
SET timezone = 'UTC';

-- Create initial schema will be handled by SQLx migrations
-- This file is mainly for PostgreSQL-specific setup

COMMENT ON DATABASE delong IS 'DeLong Protocol - Privacy-preserving biomedical computation platform'; 