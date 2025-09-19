-- Blockchain-related tables

-- Smart contract metadata table
CREATE TABLE IF NOT EXISTS contract (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    address VARCHAR(255) NOT NULL,      -- Contract address
    chain_id BIGINT NOT NULL,           -- Blockchain chain ID
    abi TEXT,                           -- Contract ABI JSON
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for contract
CREATE UNIQUE INDEX IF NOT EXISTS idx_contract_address ON contract(address, chain_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_contract_name_chain_id ON contract(name, chain_id);
CREATE INDEX IF NOT EXISTS idx_contract_name ON contract(name);
CREATE INDEX IF NOT EXISTS idx_contract_created_at ON contract(created_at);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_contract_updated_at ON contract;
CREATE TRIGGER update_contract_updated_at
    BEFORE UPDATE ON contract
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Blockchain transaction tracking table
CREATE TABLE IF NOT EXISTS transaction (
    id BIGSERIAL PRIMARY KEY,
    tx_hash VARCHAR(255) NOT NULL,      -- Transaction hash
    entity_id BIGINT NOT NULL,          -- Related entity ID
    entity_type entity_type NOT NULL,  -- Using enum type
    status transaction_status NOT NULL DEFAULT 'pending',
    gas_used BIGINT,
    gas_price BIGINT,
    block_number BIGINT,
    block_timestamp TIMESTAMPTZ,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,      -- Retry attempts for failed txs
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for transaction
CREATE UNIQUE INDEX IF NOT EXISTS idx_transaction_hash ON transaction(tx_hash);
CREATE INDEX IF NOT EXISTS idx_transaction_entity ON transaction(entity_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_transaction_status ON transaction(status);
CREATE INDEX IF NOT EXISTS idx_transaction_pending ON transaction(status, created_at)
    WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_transaction_created_at ON transaction(created_at);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_transaction_updated_at ON transaction;
CREATE TRIGGER update_transaction_updated_at
    BEFORE UPDATE ON transaction
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();