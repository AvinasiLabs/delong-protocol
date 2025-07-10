-- Add migration script here
CREATE TABLE IF NOT EXISTS test_reports (
    id SERIAL PRIMARY KEY,
    user_wallet VARCHAR(255) NOT NULL,
    file_hash VARCHAR(64) NOT NULL,
    raw_report_cid VARCHAR(255) NOT NULL,
    dataset VARCHAR(255) NOT NULL,
    test_time TIMESTAMPTZ NOT NULL,

    CONSTRAINT uq_test_reports_file_hash UNIQUE (file_hash),
    CONSTRAINT uq_test_reports_raw_report_cid UNIQUE (raw_report_cid)
);

CREATE INDEX IF NOT EXISTS idx_test_reports_user_wallet ON test_reports(user_wallet);
CREATE INDEX IF NOT EXISTS idx_test_reports_dataset ON test_reports(dataset);
CREATE INDEX IF NOT EXISTS idx_test_reports_test_time ON test_reports(test_time); 