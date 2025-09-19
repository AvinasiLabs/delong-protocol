-- Dataset-related tables

-- Dataset metadata table
CREATE TABLE IF NOT EXISTS dataset (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    ui_name VARCHAR(255) NOT NULL,
    "desc" TEXT,
    file_hash VARCHAR(255) NOT NULL,
    ipfs_cid VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    file_format VARCHAR(255) NOT NULL,
    author TEXT,
    wallet VARCHAR(255) NOT NULL,       -- author_wallet unified to wallet
    sample_url TEXT,
    file_path TEXT,
    author_id BIGINT,                   -- Added in 002
    is_encrypted BOOLEAN NOT NULL DEFAULT true, -- Added in 002, default changed in 003
    slug VARCHAR(255) UNIQUE,           -- Added in 002
    license VARCHAR(100) DEFAULT 'Custom', -- Added in 002
    thumbnail_url TEXT,                  -- Added in 002
    version VARCHAR(50) DEFAULT 'v1.0', -- Added in 002
    detailed_desc TEXT,                  -- Added in 002
    views_count BIGINT DEFAULT 0,        -- Added in 002
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for dataset (from original schema)
CREATE UNIQUE INDEX IF NOT EXISTS idx_file_hash ON dataset(file_hash);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ipfs_cid ON dataset(ipfs_cid);
CREATE INDEX IF NOT EXISTS idx_dataset_created_at ON dataset(created_at);
-- Indexes added in 002 migration
CREATE UNIQUE INDEX IF NOT EXISTS idx_dataset_slug ON dataset(slug);
CREATE INDEX IF NOT EXISTS idx_dataset_author_id ON dataset(author_id);
CREATE INDEX IF NOT EXISTS idx_dataset_encrypted ON dataset(is_encrypted);
CREATE INDEX IF NOT EXISTS idx_dataset_views ON dataset(views_count);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_dataset_updated_at ON dataset;
CREATE TRIGGER update_dataset_updated_at
    BEFORE UPDATE ON dataset
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Dataset schema definition table
CREATE TABLE IF NOT EXISTS dataset_schema (
    id BIGSERIAL PRIMARY KEY,
    dataset_id BIGINT NOT NULL,         -- Related dataset ID (no FK)
    field_name VARCHAR(255) NOT NULL,
    field_type VARCHAR(100) NOT NULL,
    description TEXT,
    is_nullable BOOLEAN DEFAULT true,
    default_value TEXT,
    constraints JSONB,                  -- Additional constraints as JSON
    field_order INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(dataset_id, field_name)
);

-- Indexes for dataset_schema
CREATE INDEX IF NOT EXISTS idx_dataset_schema_dataset_id ON dataset_schema(dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_schema_field_order ON dataset_schema(dataset_id, field_order);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_dataset_schema_updated_at ON dataset_schema;
CREATE TRIGGER update_dataset_schema_updated_at
    BEFORE UPDATE ON dataset_schema
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Dataset tag table
CREATE TABLE IF NOT EXISTS dataset_tag (
    id BIGSERIAL PRIMARY KEY,
    dataset_id BIGINT NOT NULL,         -- Related dataset ID (no FK)
    tag VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(dataset_id, tag)
);

-- Indexes for dataset_tag
CREATE INDEX IF NOT EXISTS idx_dataset_tag_dataset_id ON dataset_tag(dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_tag_tag ON dataset_tag(tag);