-- Add migration script here
CREATE TABLE IF NOT EXISTS dynamic_datasets (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    file_path VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_dynamic_datasets_name UNIQUE (name)
);

CREATE INDEX IF NOT EXISTS idx_dynamic_datasets_created_at ON dynamic_datasets(created_at); 