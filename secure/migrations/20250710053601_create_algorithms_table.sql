-- Add migration script here
CREATE TABLE IF NOT EXISTS algorithms (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255) NOT NULL,
    cid VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT uq_algorithms_algo_link UNIQUE (algo_link),
    CONSTRAINT uq_algorithms_cid UNIQUE (cid)
);

CREATE INDEX IF NOT EXISTS idx_algorithms_created_at ON algorithms(created_at); 