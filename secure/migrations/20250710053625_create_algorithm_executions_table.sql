-- Add migration script here
CREATE TYPE review_status_enum AS ENUM ('REVIEWING', 'APPROVED', 'REJECTED');
CREATE TYPE execution_status_enum AS ENUM ('QUEUED', 'RUNNING', 'COMPLETED', 'FAILED');

CREATE TABLE IF NOT EXISTS algorithm_executions (
    id BIGSERIAL PRIMARY KEY,
    algo_id BIGINT NOT NULL,
    used_dataset VARCHAR(255) NOT NULL,
    scientist_wallet VARCHAR(255) NOT NULL,
    review_status review_status_enum NOT NULL DEFAULT 'REVIEWING',
    vote_start_time TIMESTAMPTZ,
    vote_end_time TIMESTAMPTZ,
    status execution_status_enum NOT NULL DEFAULT 'QUEUED',
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    result TEXT,
    error_msg TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_algo_executions_algo_id ON algorithm_executions(algo_id);
CREATE INDEX IF NOT EXISTS idx_algo_executions_dataset ON algorithm_executions(used_dataset);
CREATE INDEX IF NOT EXISTS idx_algo_executions_wallet ON algorithm_executions(scientist_wallet);
CREATE INDEX IF NOT EXISTS idx_algo_executions_review_status ON algorithm_executions(review_status);
CREATE INDEX IF NOT EXISTS idx_algo_executions_status ON algorithm_executions(status);
CREATE INDEX IF NOT EXISTS idx_algo_executions_created_at ON algorithm_executions(created_at); 