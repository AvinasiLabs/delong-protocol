-- Initial schema migration for delong-protocol secure module

-- Create enum types
CREATE TYPE algo_review_status AS ENUM ('reviewing', 'approved', 'rejected');
CREATE TYPE algo_exe_status AS ENUM ('queued', 'running', 'completed', 'failed');
CREATE TYPE transaction_status AS ENUM ('pending', 'confirmed', 'failed');

-- Create algos table
CREATE TABLE IF NOT EXISTS algos (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique indexes for algos
CREATE UNIQUE INDEX IF NOT EXISTS idx_algo_link ON algos(algo_link);
CREATE UNIQUE INDEX IF NOT EXISTS idx_cid ON algos(cid);
CREATE INDEX IF NOT EXISTS idx_algos_created_at ON algos(created_at);

-- Add comments for algos
COMMENT ON COLUMN algos.cid IS 'IPFS CID for algorithm source code';

-- Create algo_exes table (algorithm executions)
CREATE TABLE IF NOT EXISTS algo_exes (
    id BIGSERIAL PRIMARY KEY,
    algo_id BIGINT NOT NULL REFERENCES algos(id),
    used_dataset VARCHAR(255) NOT NULL,
    scientist_wallet VARCHAR(255) NOT NULL,
    review_status algo_review_status NOT NULL DEFAULT 'reviewing',
    vote_start_time TIMESTAMPTZ NULL,
    vote_end_time TIMESTAMPTZ NULL,
    status algo_exe_status NOT NULL DEFAULT 'queued',
    start_time TIMESTAMPTZ NULL,
    end_time TIMESTAMPTZ NULL,
    result TEXT NULL,
    error_msg TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for algo_exes
CREATE INDEX IF NOT EXISTS idx_algo_id ON algo_exes(algo_id);
CREATE INDEX IF NOT EXISTS idx_dataset ON algo_exes(used_dataset);
CREATE INDEX IF NOT EXISTS idx_wallet ON algo_exes(scientist_wallet);
CREATE INDEX IF NOT EXISTS idx_review_status ON algo_exes(review_status);
CREATE INDEX IF NOT EXISTS idx_status ON algo_exes(status);
CREATE INDEX IF NOT EXISTS idx_algo_exes_created_at ON algo_exes(created_at);

-- Add comments for algo_exes
COMMENT ON COLUMN algo_exes.scientist_wallet IS 'Ethereum wallet address (0x...)';

-- Create static_datasets table
CREATE TABLE IF NOT EXISTS static_datasets (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    ui_name VARCHAR(255) NOT NULL,
    "desc" TEXT,
    file_hash VARCHAR(255) NOT NULL,
    ipfs_cid VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    file_format VARCHAR(255) NOT NULL,
    author TEXT,
    author_wallet VARCHAR(255) NOT NULL,
    sample_url TEXT,
    file_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique indexes for static_datasets
CREATE UNIQUE INDEX IF NOT EXISTS idx_file_hash ON static_datasets(file_hash);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ipfs_cid ON static_datasets(ipfs_cid);
CREATE INDEX IF NOT EXISTS idx_static_datasets_created_at ON static_datasets(created_at);

-- Create blockchain_transactions table
CREATE TABLE IF NOT EXISTS blockchain_transactions (
    id BIGSERIAL PRIMARY KEY,
    tx_hash VARCHAR(255) NOT NULL,
    entity_id BIGINT NOT NULL,
    entity_type VARCHAR(50) NOT NULL,
    status transaction_status NOT NULL DEFAULT 'pending',
    block_number BIGINT NULL,
    block_timestamp TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for blockchain_transactions
CREATE UNIQUE INDEX IF NOT EXISTS idx_tx_hash ON blockchain_transactions(tx_hash);
CREATE INDEX IF NOT EXISTS idx_entity ON blockchain_transactions(entity_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_tx_status ON blockchain_transactions(status);
CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_created_at ON blockchain_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_block_number ON blockchain_transactions(block_number);

-- Add comments for blockchain_transactions
COMMENT ON COLUMN blockchain_transactions.entity_type IS 'static_dataset, execution, data_usage, vote, committee';

-- Create committee_members table
CREATE TABLE IF NOT EXISTS committee_members (
    id SERIAL PRIMARY KEY,
    member_wallet VARCHAR(255) NOT NULL,
    is_approved BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique index for committee_members
CREATE UNIQUE INDEX IF NOT EXISTS idx_committee_member_wallet ON committee_members(member_wallet);
CREATE INDEX IF NOT EXISTS idx_committee_members_created_at ON committee_members(created_at);

-- Create contract_metas table (smart contract metadata)
CREATE TABLE IF NOT EXISTS contract_metas (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    address VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for contract_metas
CREATE UNIQUE INDEX IF NOT EXISTS idx_contract_name ON contract_metas(name);
CREATE INDEX IF NOT EXISTS idx_contract_address ON contract_metas(address);
CREATE INDEX IF NOT EXISTS idx_contract_metas_created_at ON contract_metas(created_at);

-- Add comments for contract_metas
COMMENT ON COLUMN contract_metas.name IS 'Contract identifier (data_contribution, algorithm_review, etc.)';

-- Create data_usage table
CREATE TABLE IF NOT EXISTS data_usage (
    id BIGSERIAL PRIMARY KEY,
    scientist_wallet VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    dataset VARCHAR(255) NOT NULL,
    used_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for data_usage
CREATE INDEX IF NOT EXISTS idx_data_usage_scientist_wallet ON data_usage(scientist_wallet);
CREATE INDEX IF NOT EXISTS idx_data_usage_cid ON data_usage(cid);
CREATE INDEX IF NOT EXISTS idx_data_usage_dataset ON data_usage(dataset);
CREATE INDEX IF NOT EXISTS idx_data_usage_used_at ON data_usage(used_at);

-- Add comments for data_usage
COMMENT ON COLUMN data_usage.cid IS 'Algorithm CID';

-- Create votes table
CREATE TABLE IF NOT EXISTS votes (
    id BIGSERIAL PRIMARY KEY,
    algo_cid VARCHAR(255) NOT NULL,
    voter VARCHAR(255) NOT NULL,
    approve BOOLEAN NOT NULL,
    voted_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for votes
CREATE UNIQUE INDEX IF NOT EXISTS idx_algo_cid_voter ON votes(algo_cid, voter);
CREATE INDEX IF NOT EXISTS idx_voted_at ON votes(voted_at);

-- Add comments for votes
COMMENT ON COLUMN votes.algo_cid IS 'Algorithm CID';
COMMENT ON COLUMN votes.voter IS 'Committee member wallet address';

-- Create datasets table (missing from original but referenced in handlers)
CREATE TABLE IF NOT EXISTS datasets (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    owner_address VARCHAR(255) NOT NULL,
    ipfs_hash VARCHAR(255) NOT NULL,
    size_bytes BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for datasets
CREATE UNIQUE INDEX IF NOT EXISTS idx_datasets_name ON datasets(name);
CREATE UNIQUE INDEX IF NOT EXISTS idx_datasets_ipfs_hash ON datasets(ipfs_hash);
CREATE INDEX IF NOT EXISTS idx_datasets_owner ON datasets(owner_address);
CREATE INDEX IF NOT EXISTS idx_datasets_created_at ON datasets(created_at);

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for all tables to update updated_at
CREATE TRIGGER update_algos_updated_at BEFORE UPDATE ON algos
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_algo_exes_updated_at BEFORE UPDATE ON algo_exes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_static_datasets_updated_at BEFORE UPDATE ON static_datasets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_blockchain_transactions_updated_at BEFORE UPDATE ON blockchain_transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_committee_members_updated_at BEFORE UPDATE ON committee_members
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_data_usage_updated_at BEFORE UPDATE ON data_usage
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_votes_updated_at BEFORE UPDATE ON votes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_datasets_updated_at BEFORE UPDATE ON datasets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
