-- Initial schema migration for delong-protocol secure module

-- Create enum types
CREATE TYPE algo_review_status AS ENUM ('reviewing', 'approved', 'rejected');
CREATE TYPE algo_exe_status AS ENUM ('queued', 'running', 'completed', 'failed');
CREATE TYPE transaction_status AS ENUM ('pending', 'confirmed', 'failed');

-- Create algo table
CREATE TABLE IF NOT EXISTS algo (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique indexes for algo
CREATE UNIQUE INDEX IF NOT EXISTS idx_algo_link ON algo(algo_link);
CREATE UNIQUE INDEX IF NOT EXISTS idx_cid ON algo(cid);
CREATE INDEX IF NOT EXISTS idx_algo_created_at ON algo(created_at);

-- Add comments for algo
COMMENT ON COLUMN algo.cid IS 'IPFS CID for algorithm source code';

-- Create algo_exe table (algorithm executions)
CREATE TABLE IF NOT EXISTS algo_exe (
    id BIGSERIAL PRIMARY KEY,
    algo_id BIGINT NOT NULL REFERENCES algo(id),
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

-- Create indexes for algo_exe
CREATE INDEX IF NOT EXISTS idx_algo_id ON algo_exe(algo_id);
CREATE INDEX IF NOT EXISTS idx_dataset ON algo_exe(used_dataset);
CREATE INDEX IF NOT EXISTS idx_wallet ON algo_exe(scientist_wallet);
CREATE INDEX IF NOT EXISTS idx_review_status ON algo_exe(review_status);
CREATE INDEX IF NOT EXISTS idx_status ON algo_exe(status);
CREATE INDEX IF NOT EXISTS idx_algo_exe_created_at ON algo_exe(created_at);

-- Add comments for algo_exe
COMMENT ON COLUMN algo_exe.scientist_wallet IS 'Ethereum wallet address (0x...)';

-- Create dataset table
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
    author_wallet VARCHAR(255) NOT NULL,
    sample_url TEXT,
    file_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique indexes for dataset
CREATE UNIQUE INDEX IF NOT EXISTS idx_file_hash ON dataset(file_hash);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ipfs_cid ON dataset(ipfs_cid);
CREATE INDEX IF NOT EXISTS idx_dataset_created_at ON dataset(created_at);

-- Create blockchain_transaction table
CREATE TABLE IF NOT EXISTS blockchain_transaction (
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

-- Create indexes for blockchain_transaction
CREATE UNIQUE INDEX IF NOT EXISTS idx_tx_hash ON blockchain_transaction(tx_hash);
CREATE INDEX IF NOT EXISTS idx_entity ON blockchain_transaction(entity_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_tx_status ON blockchain_transaction(status);
CREATE INDEX IF NOT EXISTS idx_blockchain_transaction_created_at ON blockchain_transaction(created_at);
CREATE INDEX IF NOT EXISTS idx_blockchain_transaction_block_number ON blockchain_transaction(block_number);

-- Add comments for blockchain_transaction
COMMENT ON COLUMN blockchain_transaction.entity_type IS 'dataset, execution, data_usage, vote, committee';

-- Create committee_member table
CREATE TABLE IF NOT EXISTS committee_member (
    id SERIAL PRIMARY KEY,
    member_wallet VARCHAR(255) NOT NULL,
    is_approved BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique index for committee_member
CREATE UNIQUE INDEX IF NOT EXISTS idx_committee_member_wallet ON committee_member(member_wallet);
CREATE INDEX IF NOT EXISTS idx_committee_member_created_at ON committee_member(created_at);

-- Create contract_meta table (smart contract metadata)
CREATE TABLE IF NOT EXISTS contract_meta (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    address VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for contract_meta
CREATE UNIQUE INDEX IF NOT EXISTS idx_contract_name ON contract_meta(name);
CREATE INDEX IF NOT EXISTS idx_contract_address ON contract_meta(address);
CREATE INDEX IF NOT EXISTS idx_contract_meta_created_at ON contract_meta(created_at);

-- Add comments for contract_meta
COMMENT ON COLUMN contract_meta.name IS 'Contract identifier (data_contribution, algorithm_review, etc.)';

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

-- Create vote table
CREATE TABLE IF NOT EXISTS vote (
    id BIGSERIAL PRIMARY KEY,
    algo_cid VARCHAR(255) NOT NULL,
    voter VARCHAR(255) NOT NULL,
    approve BOOLEAN NOT NULL,
    voted_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for vote
CREATE UNIQUE INDEX IF NOT EXISTS idx_algo_cid_voter ON vote(algo_cid, voter);
CREATE INDEX IF NOT EXISTS idx_voted_at ON vote(voted_at);

-- Add comments for vote
COMMENT ON COLUMN vote.algo_cid IS 'Algorithm CID';
COMMENT ON COLUMN vote.voter IS 'Committee member wallet address';



-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for all tables to update updated_at
CREATE TRIGGER update_algo_updated_at BEFORE UPDATE ON algo
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_algo_exe_updated_at BEFORE UPDATE ON algo_exe
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_dataset_updated_at BEFORE UPDATE ON dataset
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_blockchain_transaction_updated_at BEFORE UPDATE ON blockchain_transaction
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_committee_member_updated_at BEFORE UPDATE ON committee_member
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_data_usage_updated_at BEFORE UPDATE ON data_usage
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_vote_updated_at BEFORE UPDATE ON vote
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
