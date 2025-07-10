-- Add migration script here
CREATE TABLE IF NOT EXISTS static_datasets (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    ui_name VARCHAR(255) NOT NULL,
    "desc" TEXT,
    file_hash VARCHAR(64) NOT NULL,
    ipfs_cid VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    file_format VARCHAR(50) NOT NULL,
    author VARCHAR(255),
    author_wallet VARCHAR(255) NOT NULL,
    sample_url VARCHAR(255),
    file_path VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_static_datasets_name UNIQUE (name),
    CONSTRAINT uq_static_datasets_ui_name UNIQUE (ui_name),
    CONSTRAINT uq_static_datasets_file_hash UNIQUE (file_hash)
);

CREATE INDEX IF NOT EXISTS idx_static_datasets_author_wallet ON static_datasets(author_wallet);
CREATE INDEX IF NOT EXISTS idx_static_datasets_created_at ON static_datasets(created_at);
