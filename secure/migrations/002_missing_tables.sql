-- Missing tables and fields for DeLong Protocol Secure Service
-- Based on blockchain_sync.rs requirements

-- Blockchain events table for audit trail and event tracking
CREATE TABLE blockchain_events (
    id BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    transaction_hash VARCHAR(66) NOT NULL,
    block_number BIGINT NOT NULL,
    contract_address VARCHAR(42) NOT NULL,
    event_data JSONB NOT NULL,
    processed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for blockchain_events
CREATE INDEX idx_blockchain_events_event_type ON blockchain_events(event_type);
CREATE INDEX idx_blockchain_events_transaction_hash ON blockchain_events(transaction_hash);
CREATE INDEX idx_blockchain_events_block_number ON blockchain_events(block_number);
CREATE INDEX idx_blockchain_events_processed ON blockchain_events(processed);
CREATE INDEX idx_blockchain_events_created_at ON blockchain_events(created_at);

-- Blockchain sync state table for tracking sync progress
CREATE TABLE blockchain_sync_state (
    sync_key VARCHAR(50) PRIMARY KEY,
    block_number BIGINT NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Insert initial sync state
INSERT INTO blockchain_sync_state (sync_key, block_number) 
VALUES ('last_processed_block', 0) 
ON CONFLICT (sync_key) DO NOTHING;

-- Add missing fields to votes table
ALTER TABLE votes 
ADD COLUMN IF NOT EXISTS created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Create trigger for blockchain_events updated_at
CREATE TRIGGER update_blockchain_events_updated_at
    BEFORE UPDATE ON blockchain_events
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create trigger for blockchain_sync_state updated_at  
CREATE TRIGGER update_blockchain_sync_state_updated_at
    BEFORE UPDATE ON blockchain_sync_state
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create trigger for votes updated_at
CREATE TRIGGER update_votes_updated_at
    BEFORE UPDATE ON votes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add some additional indexes for performance
CREATE INDEX idx_algorithm_executions_review_status_created_at ON algorithm_executions(review_status, created_at);
CREATE INDEX idx_votes_execution_id_voter_wallet ON votes(execution_id, voter_wallet);
CREATE INDEX idx_blockchain_transactions_status_entity_type ON blockchain_transactions(status, entity_type);

-- Create view for confirmed committee members (matching Go logic)
CREATE OR REPLACE VIEW confirmed_committee_members AS
SELECT cm.*
FROM committee_members cm
JOIN blockchain_transactions bt ON bt.entity_id = cm.id
WHERE bt.status = 'CONFIRMED' AND bt.entity_type = 'COMMITTEE';

-- Create view for confirmed algorithm executions
CREATE OR REPLACE VIEW confirmed_algorithm_executions AS  
SELECT ae.*
FROM algorithm_executions ae
JOIN blockchain_transactions bt ON bt.entity_id = ae.id
WHERE bt.status = 'CONFIRMED' AND bt.entity_type = 'EXECUTION';

-- Create view for confirmed votes
CREATE OR REPLACE VIEW confirmed_votes AS
SELECT v.*
FROM votes v
JOIN blockchain_transactions bt ON bt.entity_id = v.id  
WHERE bt.status = 'CONFIRMED' AND bt.entity_type = 'VOTE';

COMMENT ON TABLE blockchain_events IS 'Event audit trail for blockchain synchronization';
COMMENT ON TABLE blockchain_sync_state IS 'Tracks blockchain synchronization progress';
COMMENT ON VIEW confirmed_committee_members IS 'Committee members with confirmed blockchain transactions';
COMMENT ON VIEW confirmed_algorithm_executions IS 'Algorithm executions with confirmed blockchain transactions';
COMMENT ON VIEW confirmed_votes IS 'Votes with confirmed blockchain transactions'; 