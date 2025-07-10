-- Add migration script here
CREATE TYPE transaction_status_enum AS ENUM ('PENDING', 'CONFIRMED', 'FAILED');

CREATE TABLE IF NOT EXISTS blockchain_transactions (
    id BIGSERIAL PRIMARY KEY,
    tx_hash VARCHAR(66) NOT NULL,
    entity_id BIGINT NOT NULL,
    entity_type VARCHAR(255) NOT NULL,
    status transaction_status_enum NOT NULL,
    block_number BIGINT,
    block_timestamp TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_blockchain_transactions_tx_hash UNIQUE (tx_hash)
);

CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_entity_id ON blockchain_transactions(entity_id);
CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_entity_type ON blockchain_transactions(entity_type);
CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_status ON blockchain_transactions(status);
CREATE INDEX IF NOT EXISTS idx_blockchain_transactions_created_at ON blockchain_transactions(created_at); 