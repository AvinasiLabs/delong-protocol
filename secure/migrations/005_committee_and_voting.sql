-- Committee and voting tables

-- Committee members table (keep original field names, only change member_wallet to wallet)
CREATE TABLE IF NOT EXISTS committee (
    id SERIAL PRIMARY KEY,
    wallet VARCHAR(255) NOT NULL,           -- was member_wallet, unified to wallet
    is_approved BOOLEAN NOT NULL DEFAULT false,  -- keep original name
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for committee
CREATE UNIQUE INDEX IF NOT EXISTS idx_committee_wallet ON committee(wallet);
CREATE INDEX IF NOT EXISTS idx_committee_is_approved ON committee(is_approved);
CREATE INDEX IF NOT EXISTS idx_committee_created_at ON committee(created_at);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_committee_updated_at ON committee;
CREATE TRIGGER update_committee_updated_at
    BEFORE UPDATE ON committee
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Voting records table
CREATE TABLE IF NOT EXISTS vote (
    id BIGSERIAL PRIMARY KEY,
    algo_cid VARCHAR(255) NOT NULL,     -- Algorithm CID being voted on
    wallet VARCHAR(255) NOT NULL,       -- Voter wallet address (was voter)
    approved BOOLEAN NOT NULL,          -- TRUE = approve, FALSE = reject (was approve)
    vote_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,  -- was voted_at
    reason TEXT,                         -- Optional vote reason
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(algo_cid, wallet)            -- One vote per member per algorithm
);

-- Indexes for vote
CREATE INDEX IF NOT EXISTS idx_vote_cid ON vote(algo_cid);
CREATE INDEX IF NOT EXISTS idx_vote_wallet ON vote(wallet);
CREATE INDEX IF NOT EXISTS idx_vote_approved ON vote(approved);
CREATE INDEX IF NOT EXISTS idx_vote_created_at ON vote(created_at);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_vote_updated_at ON vote;
CREATE TRIGGER update_vote_updated_at
    BEFORE UPDATE ON vote
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();