use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::types::Json as SqlxJson;
use sqlx::{FromRow, PgPool, Row};
use common::{ApiError, Result as ApiResult};

// Entity types and transaction status
pub const ENTITY_TYPE_TEST_REPORT: &str = "TEST_REPORT";
pub const TX_STATUS_CONFIRMED: &str = "CONFIRMED";


/// Request model for creating a new test report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTestReportRequest {
    pub user_wallet: String,
    pub file_hash: String,
    pub raw_report_cid: String,
    pub dataset: String,
    pub results: Vec<TestResultRequest>,
}

/// Individual test result item within a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResultRequest {
    pub name: String,
    pub status: String,
    pub explanation: String,
}

/// Test report model for uploaded reports from users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub id: i64,
    pub user_wallet: String,
    pub file_hash: String,
    pub raw_report_cid: String,
    pub dataset: String,
    pub test_results: Vec<TestResult>,
    pub test_time: DateTime<Utc>,
}

/// Public-facing information about a test report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReportInfo {
    pub id: i64,
    pub user_wallet: String,
    pub file_hash: String,
    pub raw_report_cid: String,
    pub dataset: String,
    pub test_results: Vec<TestResult>,
    pub test_time: DateTime<Utc>,
}

impl From<TestReport> for TestReportInfo {
    fn from(report: TestReport) -> Self {
        Self {
            id: report.id,
            user_wallet: report.user_wallet,
            file_hash: report.file_hash,
            raw_report_cid: report.raw_report_cid,
            dataset: report.dataset,
            test_results: report.test_results,
            test_time: report.test_time,
        }
    }
}


/// Manual implementation of FromRow to handle JSONB mapping.
impl<'r> FromRow<'r, PgRow> for TestReport {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let test_results_json: SqlxJson<Vec<TestResult>> = row.try_get("test_results")?;
        Ok(TestReport {
            id: row.try_get("id")?,
            user_wallet: row.try_get("user_wallet")?,
            file_hash: row.try_get("file_hash")?,
            raw_report_cid: row.try_get("raw_report_cid")?,
            dataset: row.try_get("dataset")?,
            test_results: test_results_json.0,
            test_time: row.try_get("test_time")?,
        })
    }
}

/// Individual test result item under a report.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "jsonb")]
pub struct TestResult {
    pub test_name: String,
    pub status: String,
    pub description: String,
}

impl TestReport {
    /// Create a new test report within a database transaction
    pub async fn create_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: CreateTestReportRequest,
    ) -> Result<Self, sqlx::Error> {
        let test_results: Vec<TestResult> = req
            .results
            .into_iter()
            .map(|r| TestResult {
                test_name: r.name,
                status: r.status,
                description: r.explanation,
            })
            .collect();

        let test_results_json = serde_json::to_value(&test_results)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        sqlx::query_as(
            r#"
            INSERT INTO test_reports (user_wallet, file_hash, raw_report_cid, dataset, test_results)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(req.user_wallet)
        .bind(req.file_hash)
        .bind(req.raw_report_cid)
        .bind(req.dataset)
        .bind(test_results_json)
        .fetch_one(&mut **tx)
        .await
    }

    /// Find a confirmed test report by its ID
    pub async fn find_by_id(pool: &PgPool, id: i64) -> ApiResult<Option<Self>> {
        let report = sqlx::query_as(
            r#"
            SELECT tr.* FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE tr.id = $1 AND bt.entity_type = $2 AND bt.status = $3
            "#,
        )
        .bind(id)
        .bind(ENTITY_TYPE_TEST_REPORT)
        .bind(TX_STATUS_CONFIRMED)
        .fetch_optional(pool)
        .await?;
        Ok(report)
    }

    /// Find a confirmed test report by its file hash
    pub async fn find_by_hash(pool: &PgPool, hash: &str) -> ApiResult<Option<Self>> {
        let report = sqlx::query_as(
            r#"
            SELECT tr.* FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE tr.file_hash = $1 AND bt.entity_type = $2 AND bt.status = $3
            "#,
        )
        .bind(hash)
        .bind(ENTITY_TYPE_TEST_REPORT)
        .bind(TX_STATUS_CONFIRMED)
        .fetch_optional(pool)
        .await?;
        Ok(report)
    }

    /// Find confirmed reports by user with pagination
    pub async fn find_by_user(
        pool: &PgPool,
        user_wallet: &str,
        page: i64,
        page_size: i64,
    ) -> ApiResult<(Vec<TestReport>, i64)> {
        let offset = (page - 1) * page_size;
        let query = r#"
            SELECT tr.* FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE tr.user_wallet = $1 AND bt.entity_type = $2 AND bt.status = $3
            ORDER BY tr.test_time DESC LIMIT $4 OFFSET $5
        "#;
        let reports = sqlx::query_as(query)
                .bind(user_wallet)
                .bind(ENTITY_TYPE_TEST_REPORT)
                .bind(TX_STATUS_CONFIRMED)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;

        let count_query = r#"
            SELECT COUNT(tr.id) FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE tr.user_wallet = $1 AND bt.entity_type = $2 AND bt.status = $3
        "#;
        let (total,): (i64,) = sqlx::query_as(count_query)
            .bind(user_wallet)
            .bind(ENTITY_TYPE_TEST_REPORT)
            .bind(TX_STATUS_CONFIRMED)
            .fetch_one(pool)
            .await?;
        Ok((reports, total))
    }

    /// Find confirmed reports by dataset with pagination
    pub async fn find_by_dataset(
        pool: &PgPool,
        dataset: &str,
        page: i64,
        page_size: i64,
    ) -> ApiResult<(Vec<TestReport>, i64)> {
        let offset = (page - 1) * page_size;
        let query = r#"
            SELECT tr.* FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE tr.dataset = $1 AND bt.entity_type = $2 AND bt.status = $3
            ORDER BY tr.test_time DESC LIMIT $4 OFFSET $5
        "#;
        let reports = sqlx::query_as(query)
                .bind(dataset)
                .bind(ENTITY_TYPE_TEST_REPORT)
                .bind(TX_STATUS_CONFIRMED)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;

        let count_query = r#"
            SELECT COUNT(tr.id) FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE tr.dataset = $1 AND bt.entity_type = $2 AND bt.status = $3
        "#;
        let (total,): (i64,) = sqlx::query_as(count_query)
            .bind(dataset)
            .bind(ENTITY_TYPE_TEST_REPORT)
            .bind(TX_STATUS_CONFIRMED)
            .fetch_one(pool)
            .await?;
        Ok((reports, total))
    }

    /// Find all confirmed reports with pagination
    pub async fn find_all(
        pool: &PgPool,
        page: i64,
        page_size: i64,
    ) -> ApiResult<(Vec<Self>, i64)> {
        let offset = (page - 1) * page_size;

        let query = r#"
            SELECT tr.* FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE bt.entity_type = $1 AND bt.status = $2
            ORDER BY tr.test_time DESC LIMIT $3 OFFSET $4
        "#;
        let reports: Vec<TestReport> = sqlx::query_as(query)
                .bind(ENTITY_TYPE_TEST_REPORT)
                .bind(TX_STATUS_CONFIRMED)
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;
        
        let count_query = r#"
            SELECT COUNT(tr.id) FROM test_reports tr
            JOIN blockchain_transactions bt ON tr.id = bt.entity_id
            WHERE bt.entity_type = $1 AND bt.status = $2
        "#;
        let (total,): (i64,) = sqlx::query_as(count_query)
            .bind(ENTITY_TYPE_TEST_REPORT)
            .bind(TX_STATUS_CONFIRMED)
            .fetch_one(pool)
            .await?;

        Ok((reports, total))
    }
} 