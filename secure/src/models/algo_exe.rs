use crate::models::Algo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// Algorithm status constants
pub const ALGO_STATUS_REVIEWING: &str = "REVIEWING";
pub const ALGO_STATUS_APPROVED: &str = "APPROVED";
pub const ALGO_STATUS_REJECTED: &str = "REJECTED";

// Execution status constants
pub const EXE_STATUS_QUEUED: &str = "QUEUED";
pub const EXE_STATUS_RUNNING: &str = "RUNNING";
pub const EXE_STATUS_COMPLETED: &str = "COMPLETED";
pub const EXE_STATUS_FAILED: &str = "FAILED";

// Entity types and transaction status
pub const ENTITY_TYPE_EXECUTION: &str = "EXECUTION";
pub const TX_STATUS_CONFIRMED: &str = "CONFIRMED";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgoExe {
    pub id: i64,
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: String,
    pub vote_start_time: Option<DateTime<Utc>>,
    pub vote_end_time: Option<DateTime<Utc>>,
    pub status: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error_msg: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgoExeRequest {
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgoExeWithAlgo {
    pub id: i64,
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: String,
    pub vote_start_time: Option<DateTime<Utc>>,
    pub vote_end_time: Option<DateTime<Utc>>,
    pub status: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error_msg: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub algo_name: String,
    pub algo_link: String,
    pub algo_cid: String,
}

/// Request model for algorithm execution submission (from GitHub)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitAlgoExeRequest {
    pub github_repo: String,
    pub commit_hash: String,
    pub scientist_wallet: String,
    pub dataset: String,
}

/// Response model for algorithm execution submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitAlgoExeResponse {
    pub id: i64,
    pub tx_hash: String,
    pub status: String,
}

impl AlgoExe {
    /// Create a new algorithm execution record
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: CreateAlgoExeRequest,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, AlgoExe>(
            r#"
            INSERT INTO algorithm_executions (algo_id, used_dataset, scientist_wallet, review_status, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(req.algo_id)
        .bind(req.used_dataset)
        .bind(req.scientist_wallet)
        .bind(ALGO_STATUS_REVIEWING)
        .bind(EXE_STATUS_QUEUED)
        .fetch_one(&mut **tx)
        .await
    }

    /// Get a single algorithm execution by ID, ensuring it's confirmed on the blockchain
    pub async fn get_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        let execution = sqlx::query_as(
            r#"
            SELECT ae.*
            FROM algo_exes AS ae
            JOIN blockchain_transactions AS bt ON ae.id = bt.entity_id
            WHERE ae.id = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
        )
        .bind(id)
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_EXECUTION)
        .fetch_optional(pool)
        .await?;
        Ok(execution)
    }

    /// Get all executions currently in the 'REVIEWING' state
    pub async fn get_reviewing(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, AlgoExe>("SELECT * FROM algorithm_executions WHERE review_status = 'REVIEWING'")
            .fetch_all(pool)
            .await
    }

    /// Get pending executions in RUNNING state with confirmed transactions
    pub async fn get_pending_confirmed(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, AlgoExe>(
            r#"
            SELECT ae.*
            FROM algorithm_executions ae
            JOIN blockchain_transactions bt ON bt.entity_id = ae.id
            WHERE bt.status = $1 AND bt.entity_type = $2 AND ae.status = $3
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_EXECUTION)
        .bind(EXE_STATUS_RUNNING)
        .fetch_all(pool)
        .await
    }

    /// Get paginated execution records
    pub async fn get_paginated(
        pool: &PgPool,
        page: i64,
        limit: i64,
    ) -> Result<(Vec<AlgoExeWithAlgo>, i64), sqlx::Error> {
        let offset = (page - 1) * limit;

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM algo_exes
            JOIN blockchain_transactions bt ON bt.entity_id = algo_exes.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_EXECUTION)
        .fetch_one(pool)
        .await?;

        let executions = sqlx::query_as(
            r#"
            SELECT
                algo_exes.*,
                algos.name as algo_name,
                algos.algo_link,
                algos.cid
            FROM algo_exes
            JOIN algorithms AS algos ON algos.id = algo_exes.algo_id
            JOIN blockchain_transactions bt ON bt.entity_id = algo_exes.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            ORDER BY algo_exes.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_EXECUTION)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok((executions, total.0))
    }

    /// Get paginated execution records with additional algorithm information
    pub async fn get_paginated_with_algo_info(
        pool: &PgPool,
        page: i64,
        limit: i64,
    ) -> Result<(Vec<AlgoExeWithAlgo>, i64), sqlx::Error> {
        let offset = (page - 1) * limit;

        let total_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM algorithm_executions ae
            JOIN blockchain_transactions bt ON bt.entity_id = ae.id
            JOIN algorithms a ON a.id = ae.algo_id
            WHERE bt.status = $1 AND bt.entity_type = $2
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_EXECUTION)
        .fetch_one(pool)
        .await?;

        let executions = sqlx::query_as::<_, AlgoExeWithAlgo>(
            r#"
            SELECT ae.*, a.name as algo_name, a.algo_link, a.cid as algo_cid
            FROM algorithm_executions ae
            JOIN blockchain_transactions bt ON bt.entity_id = ae.id
            JOIN algorithms a ON a.id = ae.algo_id
            WHERE bt.status = $1 AND bt.entity_type = $2
            ORDER BY ae.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(ENTITY_TYPE_EXECUTION)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((executions, total_count))
    }

    /// Update algorithm execution status
    pub async fn update_execution_status(
        pool: &PgPool,
        execution_id: i64,
        status: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, AlgoExe>(
            r#"
            UPDATE algorithm_executions
            SET status = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING *
            "#,
        )
        .bind(status)
        .bind(execution_id)
        .fetch_one(pool)
        .await
    }

    /// Update algorithm execution when completed
    pub async fn update_execution_completed(
        pool: &PgPool,
        execution_id: i64,
        result: &str,
        error_msg: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, AlgoExe>(
            r#"
            UPDATE algorithm_executions
            SET status = 'COMPLETED',
                result = $1,
                error_msg = $2,
                end_time = NOW(),
                updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(result)
        .bind(error_msg)
        .bind(execution_id)
        .fetch_one(pool)
        .await
    }

    /// Update algorithm review status and voting times
    pub async fn update_review_status(
        pool: &PgPool,
        id: i64,
        review_status: &str,
        vote_start: Option<DateTime<Utc>>,
        vote_end: Option<DateTime<Utc>>,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, AlgoExe>(
            r#"
            UPDATE algorithm_executions
            SET review_status = $1, vote_start_time = $2, vote_end_time = $3, updated_at = NOW()
            WHERE id = $4
            RETURNING *
            "#,
        )
        .bind(review_status)
        .bind(vote_start)
        .bind(vote_end)
        .bind(id)
        .fetch_optional(pool)
        .await
    }
} 