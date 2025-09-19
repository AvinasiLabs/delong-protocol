-- Algorithm-related tables

-- Algorithm metadata table
CREATE TABLE IF NOT EXISTS algorithm (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255) NOT NULL,  -- URL to algorithm source
    cid VARCHAR(255) NOT NULL,         -- IPFS CID
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for algorithm
CREATE UNIQUE INDEX IF NOT EXISTS idx_algorithm_link ON algorithm(algo_link);
CREATE UNIQUE INDEX IF NOT EXISTS idx_algorithm_cid ON algorithm(cid);
CREATE INDEX IF NOT EXISTS idx_algorithm_created_at ON algorithm(created_at);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_algorithm_updated_at ON algorithm;
CREATE TRIGGER update_algorithm_updated_at
    BEFORE UPDATE ON algorithm
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Legacy execution table (kept for compatibility)
CREATE TABLE IF NOT EXISTS execution (
    id BIGSERIAL PRIMARY KEY,
    algo_id BIGINT NOT NULL,
    used_dataset VARCHAR(255) NOT NULL,
    wallet VARCHAR(255) NOT NULL,      -- scientist wallet
    user_id BIGINT,
    review_status review_status NOT NULL DEFAULT 'reviewing',
    vote_start_time TIMESTAMPTZ,
    vote_end_time TIMESTAMPTZ,
    status execution_status NOT NULL DEFAULT 'queued',
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    result TEXT,
    error_msg TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for execution
CREATE INDEX IF NOT EXISTS idx_execution_algo_id ON execution(algo_id);
CREATE INDEX IF NOT EXISTS idx_execution_dataset ON execution(used_dataset);
CREATE INDEX IF NOT EXISTS idx_execution_wallet ON execution(wallet);
CREATE INDEX IF NOT EXISTS idx_execution_review_status ON execution(review_status);
CREATE INDEX IF NOT EXISTS idx_execution_status ON execution(status);
CREATE INDEX IF NOT EXISTS idx_execution_created_at ON execution(created_at);

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_execution_updated_at ON execution;
CREATE TRIGGER update_execution_updated_at
    BEFORE UPDATE ON execution
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Unified algorithm execution table (merged with data_usage)
CREATE TABLE IF NOT EXISTS algorithm_execution (
    id BIGSERIAL PRIMARY KEY,

    -- Algorithm info
    algo_id BIGINT NOT NULL,
    algo_name VARCHAR(255),
    algo_cid VARCHAR(255) NOT NULL,
    algo_link VARCHAR(255),

    -- Dataset info
    dataset_id BIGINT,
    dataset_name VARCHAR(255) NOT NULL,
    dataset_cid VARCHAR(255),

    -- User info
    wallet VARCHAR(255) NOT NULL,       -- scientist wallet
    user_id BIGINT,

    -- Execution lifecycle
    review_status VARCHAR(20) NOT NULL DEFAULT 'reviewing',
    execution_status VARCHAR(20) NOT NULL DEFAULT 'queued',

    -- Timestamps
    vote_start_time TIMESTAMPTZ,
    vote_end_time TIMESTAMPTZ,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- From data_usage

    -- Results
    success BOOLEAN,
    output TEXT,
    error_message TEXT,
    error_type VARCHAR(50),            -- compilation, runtime, timeout, resource_limit
    container_exit_code INTEGER,

    -- Metrics
    runtime_seconds INTEGER,
    records_processed BIGINT,          -- From data_usage

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for algorithm_execution
CREATE INDEX IF NOT EXISTS idx_algo_execution_wallet ON algorithm_execution(wallet);
CREATE INDEX IF NOT EXISTS idx_algo_execution_algo_cid ON algorithm_execution(algo_cid);
CREATE INDEX IF NOT EXISTS idx_algo_execution_dataset_name ON algorithm_execution(dataset_name);
CREATE INDEX IF NOT EXISTS idx_algo_execution_review_status ON algorithm_execution(review_status);
CREATE INDEX IF NOT EXISTS idx_algo_execution_execution_status ON algorithm_execution(execution_status);
CREATE INDEX IF NOT EXISTS idx_algo_execution_created_at ON algorithm_execution(created_at);
CREATE INDEX IF NOT EXISTS idx_algo_execution_success ON algorithm_execution(success) WHERE success IS NOT NULL;

-- Trigger for auto-updating updated_at
DROP TRIGGER IF EXISTS update_algorithm_execution_updated_at ON algorithm_execution;
CREATE TRIGGER update_algorithm_execution_updated_at
    BEFORE UPDATE ON algorithm_execution
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
