-- Add migration script here
CREATE TABLE IF NOT EXISTS data_usages (
    id BIGSERIAL PRIMARY KEY,
    scientist_wallet VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    dataset VARCHAR(255) NOT NULL,
    used_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_data_usages_wallet ON data_usages(scientist_wallet);
CREATE INDEX IF NOT EXISTS idx_data_usages_cid ON data_usages(cid);
CREATE INDEX IF NOT EXISTS idx_data_usages_dataset ON data_usages(dataset);
CREATE INDEX IF NOT EXISTS idx_data_usages_used_at ON data_usages(used_at); 