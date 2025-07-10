-- Add migration script here
CREATE TABLE IF NOT EXISTS votes (
    id BIGSERIAL PRIMARY KEY,
    algo_cid VARCHAR(255) NOT NULL,
    voter VARCHAR(255) NOT NULL,
    approve BOOLEAN NOT NULL,
    voted_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_votes_algo_cid ON votes(algo_cid);
CREATE INDEX IF NOT EXISTS idx_votes_voter ON votes(voter);
CREATE INDEX IF NOT EXISTS idx_votes_voted_at ON votes(voted_at); 