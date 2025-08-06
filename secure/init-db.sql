-- Secure Service Database Initialization Script
-- This script creates the initial schema for the secure service to match Go models

-- Enable necessary extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Drop existing tables if they exist (for development)
DROP TABLE IF EXISTS test_results CASCADE;
DROP TABLE IF EXISTS test_reports CASCADE;
DROP TABLE IF EXISTS votes CASCADE;
DROP TABLE IF EXISTS data_usage CASCADE;
DROP TABLE IF EXISTS algo_exes CASCADE;
DROP TABLE IF EXISTS algos CASCADE;
DROP TABLE IF EXISTS static_datasets CASCADE;
DROP TABLE IF EXISTS dynamic_datasets CASCADE;
DROP TABLE IF EXISTS committee_members CASCADE;
DROP TABLE IF EXISTS blockchain_transactions CASCADE;
DROP TABLE IF EXISTS contract_metas CASCADE;

-- Drop types if they exist
DROP TYPE IF EXISTS review_status CASCADE;
DROP TYPE IF EXISTS execution_status CASCADE;
DROP TYPE IF EXISTS transaction_status CASCADE;
DROP TYPE IF EXISTS test_status CASCADE;

-- Create custom types
CREATE TYPE review_status AS ENUM ('REVIEWING', 'APPROVED', 'REJECTED');
CREATE TYPE execution_status AS ENUM ('QUEUED', 'RUNNING', 'COMPLETED', 'FAILED');
CREATE TYPE transaction_status AS ENUM ('PENDING', 'CONFIRMED', 'FAILED');
CREATE TYPE test_status AS ENUM ('above_range', 'below_range', 'within_range', 'unknown');

-- Algorithms table (matches algo.go)
CREATE TABLE IF NOT EXISTS algos (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_algo_link UNIQUE (algo_link),
    CONSTRAINT idx_cid UNIQUE (cid)
);

CREATE INDEX idx_created_at ON algos(created_at);

-- Add comments
COMMENT ON COLUMN algos.cid IS 'source code CID';

-- Algorithm executions table (matches algoexe.go)
CREATE TABLE IF NOT EXISTS algo_exes (
    id BIGSERIAL PRIMARY KEY,
    algo_id BIGINT NOT NULL,
    used_dataset VARCHAR(255) NOT NULL,
    scientist_wallet VARCHAR(255) NOT NULL,
    review_status review_status NOT NULL DEFAULT 'REVIEWING',
    vote_start_time TIMESTAMP WITH TIME ZONE,
    vote_end_time TIMESTAMP WITH TIME ZONE,
    status execution_status NOT NULL DEFAULT 'QUEUED',
    start_time TIMESTAMP WITH TIME ZONE,
    end_time TIMESTAMP WITH TIME ZONE,
    result TEXT,
    error_msg TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_algo_id ON algo_exes(algo_id);
CREATE INDEX idx_dataset ON algo_exes(used_dataset);
CREATE INDEX idx_wallet ON algo_exes(scientist_wallet);
CREATE INDEX idx_review_status ON algo_exes(review_status);
CREATE INDEX idx_status ON algo_exes(status);
CREATE INDEX idx_algo_exes_created_at ON algo_exes(created_at);

-- Add comments
COMMENT ON COLUMN algo_exes.scientist_wallet IS 'Ethereum wallet address in hexadecimal format (0x...)';

-- Blockchain transactions table (matches blockchain.go)
CREATE TABLE IF NOT EXISTS blockchain_transactions (
    id BIGSERIAL PRIMARY KEY,
    tx_hash VARCHAR(66) NOT NULL,
    entity_id BIGINT NOT NULL,
    entity_type VARCHAR(255) NOT NULL,
    status transaction_status NOT NULL,
    block_number BIGINT,
    block_timestamp TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_tx_hash UNIQUE (tx_hash)
);

CREATE INDEX idx_entity_id ON blockchain_transactions(entity_id);
CREATE INDEX idx_entity_type ON blockchain_transactions(entity_type);
CREATE INDEX idx_bt_status ON blockchain_transactions(status);
CREATE INDEX idx_bt_created_at ON blockchain_transactions(created_at);

-- Add comments
COMMENT ON COLUMN blockchain_transactions.tx_hash IS 'Ethereum transaction hash';
COMMENT ON COLUMN blockchain_transactions.entity_id IS 'ID of the associated entity';
COMMENT ON COLUMN blockchain_transactions.entity_type IS 'Type of the associated entity: EXECUTION, VOTE, etc.';
COMMENT ON COLUMN blockchain_transactions.status IS 'Transaction status: PENDING, CONFIRMED, FAILED';
COMMENT ON COLUMN blockchain_transactions.block_number IS 'Block number where transaction was confirmed';
COMMENT ON COLUMN blockchain_transactions.block_timestamp IS 'Timestamp of the block';

-- Committee members table (matches committee.go)
CREATE TABLE IF NOT EXISTS committee_members (
    id SERIAL PRIMARY KEY,
    member_wallet VARCHAR(255) NOT NULL,
    is_approved BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_member_wallet UNIQUE (member_wallet)
);

CREATE INDEX idx_is_approved ON committee_members(is_approved);
CREATE INDEX idx_cm_created_at ON committee_members(created_at);

-- Contract metadata table (matches contract.go)
CREATE TABLE IF NOT EXISTS contract_metas (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    address VARCHAR(42) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_contract_name UNIQUE (name)
);

-- Add comments
COMMENT ON COLUMN contract_metas.name IS 'contract identifier, e.g. ''data_contribution''';
COMMENT ON COLUMN contract_metas.address IS 'contract address';

-- Data usage table (matches datausage.go)
CREATE TABLE IF NOT EXISTS data_usage (
    id BIGSERIAL PRIMARY KEY,
    scientist_wallet VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    dataset VARCHAR(255) NOT NULL,
    used_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_du_wallet ON data_usage(scientist_wallet);
CREATE INDEX idx_du_cid ON data_usage(cid);
CREATE INDEX idx_du_dataset ON data_usage(dataset);
CREATE INDEX idx_used_at ON data_usage(used_at);

-- Add comments
COMMENT ON COLUMN data_usage.dataset IS 'Dataset name';

-- Dynamic datasets table (matches dyn_dataset.go)
CREATE TABLE IF NOT EXISTS dynamic_datasets (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    file_path VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_dd_name UNIQUE (name)
);

CREATE INDEX idx_dd_created_at ON dynamic_datasets(created_at);

-- Static datasets table (matches stc_dataset.go)
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
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_sd_name UNIQUE (name),
    CONSTRAINT idx_ui_name UNIQUE (ui_name),
    CONSTRAINT idx_file_hash UNIQUE (file_hash)
);

CREATE INDEX idx_author_wallet ON static_datasets(author_wallet);
CREATE INDEX idx_sd_created_at ON static_datasets(created_at);

-- Add comments
COMMENT ON COLUMN static_datasets.file_hash IS 'SHA-256 hash of original file for deduplication';
COMMENT ON COLUMN static_datasets.ipfs_cid IS 'IPFS content identifier';
COMMENT ON COLUMN static_datasets.file_format IS 'csv, json, parquet...';

-- Test reports table (matches report.go)
CREATE TABLE IF NOT EXISTS test_reports (
    id BIGSERIAL PRIMARY KEY,
    user_wallet VARCHAR(255) NOT NULL,
    file_hash VARCHAR(64) NOT NULL,
    raw_report_cid VARCHAR(255) NOT NULL,
    dataset VARCHAR(255) NOT NULL,
    test_time TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT idx_tr_file_hash UNIQUE (file_hash),
    CONSTRAINT idx_raw_report_cid UNIQUE (raw_report_cid)
);

CREATE INDEX idx_user_wallet ON test_reports(user_wallet);
CREATE INDEX idx_tr_dataset ON test_reports(dataset);
CREATE INDEX idx_test_time ON test_reports(test_time);

-- Add comments
COMMENT ON COLUMN test_reports.file_hash IS 'SHA-256 hash of original file for deduplication';
COMMENT ON COLUMN test_reports.raw_report_cid IS 'CID of encrypted original file';
COMMENT ON COLUMN test_reports.dataset IS 'dataset_registry.name';

-- Test results table (matches report.go)
CREATE TABLE IF NOT EXISTS test_results (
    id BIGSERIAL PRIMARY KEY,
    test_report_id BIGINT NOT NULL,
    category VARCHAR(255) NOT NULL,
    name TEXT NOT NULL,
    definition TEXT NOT NULL,
    result TEXT NOT NULL,
    reference_range TEXT,
    explanation TEXT NOT NULL,
    status test_status NOT NULL,
    suggestions TEXT
);

CREATE INDEX idx_test_report_id ON test_results(test_report_id);
CREATE INDEX idx_category ON test_results(category);

-- Votes table (matches vote.go)
CREATE TABLE IF NOT EXISTS votes (
    id BIGSERIAL PRIMARY KEY,
    algo_cid VARCHAR(255) NOT NULL,
    voter VARCHAR(255) NOT NULL,
    approve BOOLEAN NOT NULL,
    voted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_algo_cid ON votes(algo_cid);
CREATE INDEX idx_voter ON votes(voter);
CREATE INDEX idx_voted_at ON votes(voted_at);

-- Add comments
COMMENT ON COLUMN votes.voter IS '0x...';

-- Create update timestamp trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Add update triggers to relevant tables
CREATE TRIGGER update_algos_updated_at BEFORE UPDATE ON algos
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_algo_exes_updated_at BEFORE UPDATE ON algo_exes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_blockchain_transactions_updated_at BEFORE UPDATE ON blockchain_transactions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_committee_members_updated_at BEFORE UPDATE ON committee_members
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_data_usage_updated_at BEFORE UPDATE ON data_usage
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_dynamic_datasets_updated_at BEFORE UPDATE ON dynamic_datasets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_static_datasets_updated_at BEFORE UPDATE ON static_datasets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_votes_updated_at BEFORE UPDATE ON votes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert initial data (optional)
-- Insert a test committee member
INSERT INTO committee_members (member_wallet, is_approved)
VALUES ('0x0000000000000000000000000000000000000000', true)
ON CONFLICT (member_wallet) DO NOTHING;

-- Grant permissions (adjust as needed for production)
-- GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO secure_user;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO secure_user;
