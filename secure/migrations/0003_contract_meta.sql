-- Create contract_meta table to store deployed contract addresses
CREATE TABLE IF NOT EXISTS contract_meta (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,           -- Contract name (e.g., 'DataContribution', 'AlgorithmReview')
    address VARCHAR(42) NOT NULL,          -- Ethereum address in hex format (0x...)
    chain_id VARCHAR(20) NOT NULL,         -- Chain ID (e.g., '1' for mainnet, '31337' for local)
    deployed_at TIMESTAMP NOT NULL DEFAULT NOW(),  -- When the contract was deployed
    deployed_by VARCHAR(42),               -- Optional: Address that deployed the contract
    tx_hash VARCHAR(66),                   -- Optional: Transaction hash of deployment
    block_number BIGINT,                   -- Optional: Block number of deployment
    metadata JSONB,                        -- Optional: Additional metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Ensure unique contract per chain
    CONSTRAINT unique_contract_per_chain UNIQUE (name, chain_id)
);

-- Create indexes for faster lookups
CREATE INDEX idx_contract_meta_name ON contract_meta(name);
CREATE INDEX idx_contract_meta_chain_id ON contract_meta(chain_id);
CREATE INDEX idx_contract_meta_address ON contract_meta(address);

-- Add trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_contract_meta_updated_at
    BEFORE UPDATE ON contract_meta
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add comments for documentation
COMMENT ON TABLE contract_meta IS 'Stores deployed smart contract addresses per blockchain network';
COMMENT ON COLUMN contract_meta.name IS 'Contract identifier name (e.g., DataContribution, AlgorithmReview)';
COMMENT ON COLUMN contract_meta.address IS 'Ethereum address of the deployed contract';
COMMENT ON COLUMN contract_meta.chain_id IS 'Blockchain network ID where contract is deployed';
COMMENT ON COLUMN contract_meta.deployed_at IS 'Timestamp when the contract was deployed on-chain';
COMMENT ON COLUMN contract_meta.deployed_by IS 'Address of the account that deployed the contract';
COMMENT ON COLUMN contract_meta.tx_hash IS 'Transaction hash of the contract deployment';
COMMENT ON COLUMN contract_meta.block_number IS 'Block number where the contract was deployed';
COMMENT ON COLUMN contract_meta.metadata IS 'Additional metadata about the contract deployment';
