-- Add migration script here
CREATE TYPE test_case_status_enum AS ENUM ('above_range', 'below_range', 'within_range', 'unknown');

CREATE TABLE IF NOT EXISTS test_cases (
    id SERIAL PRIMARY KEY,
    test_report_id INT NOT NULL,
    category VARCHAR(255) NOT NULL,
    name TEXT NOT NULL,
    definition TEXT NOT NULL,
    result TEXT NOT NULL,
    reference_range TEXT,
    explanation TEXT NOT NULL,
    status test_case_status_enum NOT NULL,
    suggestions TEXT,

    FOREIGN KEY (test_report_id) REFERENCES test_reports(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_test_cases_test_report_id ON test_cases(test_report_id);
CREATE INDEX IF NOT EXISTS idx_test_cases_category ON test_cases(category); 