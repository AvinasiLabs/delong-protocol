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
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
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
    vote_start_time TIMESTAMP NULL,
    vote_end_time TIMESTAMP NULL,
    status algo_exe_status NOT NULL DEFAULT 'queued',
    start_time TIMESTAMP NULL,
    end_time TIMESTAMP NULL,
    result TEXT NULL,
    error_msg TEXT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
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
    file_hash VARCHAR(255) NOT NULL,
    ipfs_cid VARCHAR(255) NOT NULL,
    author_wallet VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
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
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for blockchain_transactions
CREATE UNIQUE INDEX IF NOT EXISTS idx_tx_hash ON blockchain_transactions(tx_hash);
CREATE INDEX IF NOT EXISTS idx_entity ON blockchain_transactions(entity_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_tx_status ON blockchain_transactions(status);
CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_created_at ON blockchain_transactions(created_at);

-- Add comments for blockchain_transactions
COMMENT ON COLUMN blockchain_transactions.entity_type IS 'static_dataset, execution, data_usage, vote';

-- Create committees table (key-value store for committee configuration)
CREATE TABLE IF NOT EXISTS committees (
    id BIGSERIAL PRIMARY KEY,
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create unique index for committees
CREATE UNIQUE INDEX IF NOT EXISTS idx_key ON committees(key);

-- Create contracts table (smart contract metadata)
CREATE TABLE IF NOT EXISTS contracts (
    id BIGSERIAL PRIMARY KEY,
    contract_type VARCHAR(50) NOT NULL,
    address VARCHAR(255) NOT NULL,
    chain_id BIGINT NOT NULL,
    deployed_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for contracts
CREATE UNIQUE INDEX IF NOT EXISTS idx_address ON contracts(address);
CREATE INDEX IF NOT EXISTS idx_contract_type_chain ON contracts(contract_type, chain_id);
CREATE INDEX IF NOT EXISTS idx_deployed_at ON contracts(deployed_at);

-- Add comments for contracts
COMMENT ON COLUMN contracts.contract_type IS 'data_contribution, algorithm_review';

-- Create data_usages table
CREATE TABLE IF NOT EXISTS data_usages (
    id BIGSERIAL PRIMARY KEY,
    scientist_wallet VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    dataset VARCHAR(255) NOT NULL,
    used_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for data_usages
CREATE INDEX IF NOT EXISTS idx_data_usages_scientist_wallet ON data_usages(scientist_wallet);
CREATE INDEX IF NOT EXISTS idx_data_usages_cid ON data_usages(cid);
CREATE INDEX IF NOT EXISTS idx_data_usages_dataset ON data_usages(dataset);
CREATE INDEX IF NOT EXISTS idx_used_at ON data_usages(used_at);

-- Add comments for data_usages
COMMENT ON COLUMN data_usages.cid IS 'Algorithm CID';

-- Create votes table
CREATE TABLE IF NOT EXISTS votes (
    id BIGSERIAL PRIMARY KEY,
    cid VARCHAR(255) NOT NULL,
    member VARCHAR(255) NOT NULL,
    approved BOOLEAN NOT NULL,
    vote_time TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for votes
CREATE UNIQUE INDEX IF NOT EXISTS idx_cid_member ON votes(cid, member);
CREATE INDEX IF NOT EXISTS idx_vote_time ON votes(vote_time);

-- Add comments for votes
COMMENT ON COLUMN votes.cid IS 'Algorithm CID';
COMMENT ON COLUMN votes.member IS 'Committee member wallet address';

-- Create datasets table (missing from original but referenced in handlers)
CREATE TABLE IF NOT EXISTS datasets (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    owner_address VARCHAR(255) NOT NULL,
    ipfs_hash VARCHAR(255) NOT NULL,
    size_bytes BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
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

CREATE TRIGGER update_committees_updated_at BEFORE UPDATE ON committees
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_contracts_updated_at BEFORE UPDATE ON contracts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_data_usages_updated_at BEFORE UPDATE ON data_usages
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_votes_updated_at BEFORE UPDATE ON votes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_datasets_updated_at BEFORE UPDATE ON datasets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
