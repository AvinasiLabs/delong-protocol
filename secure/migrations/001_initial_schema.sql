-- Initial schema for DeLong Protocol Secure Service
-- Migrated from MySQL to PostgreSQL

-- Create custom types for enums
CREATE TYPE transaction_status AS ENUM ('PENDING', 'CONFIRMED', 'FAILED');
CREATE TYPE entity_type AS ENUM ('EXECUTION', 'VOTE', 'COMMITTEE', 'TEST_REPORT', 'STATIC_DATASET', 'DATAUSAGE');
CREATE TYPE algo_status AS ENUM ('REVIEWING', 'APPROVED', 'REJECTED');
CREATE TYPE exe_status AS ENUM ('QUEUED', 'RUNNING', 'COMPLETED', 'FAILED');

-- Blockchain transactions table
CREATE TABLE blockchain_transactions (
    id BIGSERIAL PRIMARY KEY,
    tx_hash VARCHAR(66) NOT NULL UNIQUE,
    entity_id BIGINT NOT NULL,
    entity_type entity_type NOT NULL,
    status transaction_status NOT NULL DEFAULT 'PENDING',
    block_number BIGINT,
    block_timestamp TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for blockchain_transactions
CREATE INDEX idx_blockchain_transactions_tx_hash ON blockchain_transactions(tx_hash);
CREATE INDEX idx_blockchain_transactions_entity_id ON blockchain_transactions(entity_id);
CREATE INDEX idx_blockchain_transactions_entity_type ON blockchain_transactions(entity_type);
CREATE INDEX idx_blockchain_transactions_status ON blockchain_transactions(status);
CREATE INDEX idx_blockchain_transactions_created_at ON blockchain_transactions(created_at);

-- Static datasets table
CREATE TABLE static_datasets (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    ui_name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    file_hash VARCHAR(64) NOT NULL UNIQUE,
    ipfs_cid VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    file_format VARCHAR(50) NOT NULL,
    author VARCHAR(255),
    author_wallet VARCHAR(255) NOT NULL,
    sample_url VARCHAR(255),
    file_path VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for static_datasets
CREATE INDEX idx_static_datasets_name ON static_datasets(name);
CREATE INDEX idx_static_datasets_ui_name ON static_datasets(ui_name);
CREATE INDEX idx_static_datasets_file_hash ON static_datasets(file_hash);
CREATE INDEX idx_static_datasets_author_wallet ON static_datasets(author_wallet);
CREATE INDEX idx_static_datasets_created_at ON static_datasets(created_at);

-- Algorithms table
CREATE TABLE algorithms (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255) NOT NULL UNIQUE,
    cid VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for algorithms
CREATE INDEX idx_algorithms_algo_link ON algorithms(algo_link);
CREATE INDEX idx_algorithms_cid ON algorithms(cid);
CREATE INDEX idx_algorithms_created_at ON algorithms(created_at);

-- Algorithm executions table
CREATE TABLE algorithm_executions (
    id BIGSERIAL PRIMARY KEY,
    algo_id BIGINT NOT NULL,
    used_dataset VARCHAR(255) NOT NULL,
    scientist_wallet VARCHAR(255) NOT NULL,
    review_status algo_status NOT NULL DEFAULT 'REVIEWING',
    vote_start_time TIMESTAMP WITH TIME ZONE,
    vote_end_time TIMESTAMP WITH TIME ZONE,
    status exe_status NOT NULL DEFAULT 'QUEUED',
    start_time TIMESTAMP WITH TIME ZONE,
    end_time TIMESTAMP WITH TIME ZONE,
    result TEXT,
    error_msg TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (algo_id) REFERENCES algorithms(id) ON DELETE CASCADE
);

-- Create indexes for algorithm_executions
CREATE INDEX idx_algorithm_executions_algo_id ON algorithm_executions(algo_id);
CREATE INDEX idx_algorithm_executions_dataset ON algorithm_executions(used_dataset);
CREATE INDEX idx_algorithm_executions_scientist_wallet ON algorithm_executions(scientist_wallet);
CREATE INDEX idx_algorithm_executions_review_status ON algorithm_executions(review_status);
CREATE INDEX idx_algorithm_executions_status ON algorithm_executions(status);
CREATE INDEX idx_algorithm_executions_created_at ON algorithm_executions(created_at);

-- Committee members table
CREATE TABLE committee_members (
    id BIGSERIAL PRIMARY KEY,
    wallet_address VARCHAR(255) NOT NULL UNIQUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for committee_members
CREATE INDEX idx_committee_members_wallet_address ON committee_members(wallet_address);
CREATE INDEX idx_committee_members_is_active ON committee_members(is_active);

-- Votes table
CREATE TABLE votes (
    id BIGSERIAL PRIMARY KEY,
    execution_id BIGINT NOT NULL,
    voter_wallet VARCHAR(255) NOT NULL,
    decision VARCHAR(10) NOT NULL CHECK (decision IN ('APPROVE', 'REJECT')),
    voted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (execution_id) REFERENCES algorithm_executions(id) ON DELETE CASCADE,
    UNIQUE(execution_id, voter_wallet)
);

-- Create indexes for votes
CREATE INDEX idx_votes_execution_id ON votes(execution_id);
CREATE INDEX idx_votes_voter_wallet ON votes(voter_wallet);
CREATE INDEX idx_votes_voted_at ON votes(voted_at);

-- Data usage tracking table
CREATE TABLE data_usage (
    id BIGSERIAL PRIMARY KEY,
    dataset_id BIGINT NOT NULL,
    execution_id BIGINT NOT NULL,
    usage_timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (dataset_id) REFERENCES static_datasets(id) ON DELETE CASCADE,
    FOREIGN KEY (execution_id) REFERENCES algorithm_executions(id) ON DELETE CASCADE
);

-- Create indexes for data_usage
CREATE INDEX idx_data_usage_dataset_id ON data_usage(dataset_id);
CREATE INDEX idx_data_usage_execution_id ON data_usage(execution_id);
CREATE INDEX idx_data_usage_timestamp ON data_usage(usage_timestamp);

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Add triggers to automatically update updated_at
CREATE TRIGGER update_blockchain_transactions_updated_at
    BEFORE UPDATE ON blockchain_transactions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_static_datasets_updated_at
    BEFORE UPDATE ON static_datasets
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_algorithms_updated_at
    BEFORE UPDATE ON algorithms
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_algorithm_executions_updated_at
    BEFORE UPDATE ON algorithm_executions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_committee_members_updated_at
    BEFORE UPDATE ON committee_members
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column(); 