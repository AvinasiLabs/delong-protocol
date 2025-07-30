use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{
    Create, FindById, PaginatedResponse, PaginationParams, Timestamped,
    blockchain_transaction::EntityType,
    pg_types::{AlgoExeStatus, AlgoReviewStatus, TransactionStatus},
};
use crate::error::Result;

/// Algorithm execution entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgoExe {
    pub id: i64,
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: AlgoReviewStatus,
    pub vote_start_time: Option<DateTime<Utc>>,
    pub vote_end_time: Option<DateTime<Utc>>,
    pub status: AlgoExeStatus,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error_msg: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AlgoExe {
    /// SQL join clause for confirmed transactions
    const JOIN_CONFIRMED_TX: &str = r#"
JOIN blockchain_transactions bt
ON bt.entity_id = algo_exes.id
   AND bt.status = ?
   AND bt.entity_type = ?
    "#;

    /// Find pending executions (RUNNING status) with confirmed transactions
    pub async fn find_pending_confirmed(pool: &PgPool) -> Result<Vec<Self>> {
        let query = format!(
            r#"
            SELECT algo_exes.*
            FROM algo_exes
            {}
            WHERE algo_exes.status = ?
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let exes = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::Execution.as_str())
            .bind(AlgoExeStatus::Running)
            .fetch_all(pool)
            .await?;

        Ok(exes)
    }

    /// Find reviewing executions with confirmed transactions
    pub async fn find_reviewing_confirmed(pool: &PgPool) -> Result<Vec<Self>> {
        let query = format!(
            r#"
            SELECT algo_exes.*
            FROM algo_exes
            {}
            WHERE algo_exes.review_status = ?
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let exes = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::Execution.as_str())
            .bind(AlgoReviewStatus::Reviewing)
            .fetch_all(pool)
            .await?;

        Ok(exes)
    }

    /// Update review status
    pub async fn update_review_status(
        pool: &PgPool,
        id: i64,
        review_status: AlgoReviewStatus,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE algo_exes
            SET review_status = $1, updated_at = NOW()
            WHERE id = $2
            "#,
            review_status as _,
            id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update vote duration
    pub async fn update_vote_duration(
        pool: &PgPool,
        id: i64,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE algo_exes
            SET vote_start_time = $1, vote_end_time = $2, updated_at = NOW()
            WHERE id = $3
            "#,
            start_time,
            end_time,
            id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update execution status
    pub async fn update_status(pool: &PgPool, id: i64, status: AlgoExeStatus) -> Result<Self> {
        let now = Utc::now();

        if status == AlgoExeStatus::Running {
            sqlx::query!(
                r#"
                UPDATE algo_exes
                SET status = $1, updated_at = $2, start_time = $3
                WHERE id = $4
                "#,
                status as _,
                now,
                now,
                id
            )
            .execute(pool)
            .await?;
        } else {
            sqlx::query!(
                r#"
                UPDATE algo_exes
                SET status = $1, updated_at = $2
                WHERE id = $3
                "#,
                status as _,
                now,
                id
            )
            .execute(pool)
            .await?;
        }

        Self::find_by_id_required(pool, id).await
    }

    /// Update execution completed
    pub async fn update_completed(
        pool: &PgPool,
        id: i64,
        result: &str,
        error_msg: Option<&str>,
    ) -> Result<Self> {
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE algo_exes
            SET status = $1, result = $2, error_msg = $3, end_time = $4, updated_at = $5
            WHERE id = $6
            "#,
            AlgoExeStatus::Completed as _,
            result,
            error_msg,
            now,
            now,
            id
        )
        .execute(pool)
        .await?;

        Self::find_by_id_required(pool, id).await
    }

    /// Find by ID with confirmed transaction
    pub async fn find_by_id_confirmed(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let query = format!(
            r#"
            SELECT algo_exes.*
            FROM algo_exes
            {}
            WHERE algo_exes.id = ?
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let exe = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::Execution.as_str())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(exe)
    }

    /// List algorithm executions with algorithm info for pagination
    pub async fn list_with_algo_info(
        pool: &PgPool,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<AlgoExeWithAlgo>, u32)> {
        // Calculate offset
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM algo_exes
            JOIN blockchain_transactions bt
            ON bt.entity_id = algo_exes.id
               AND bt.status = $1
               AND bt.entity_type = $2
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Execution.as_str()
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let items = sqlx::query_as!(
            AlgoExeWithAlgo,
            r#"
            SELECT
                algo_exes.id,
                algo_exes.algo_id,
                algo_exes.used_dataset,
                algo_exes.scientist_wallet,
                algo_exes.review_status as "review_status: _",
                algo_exes.vote_start_time,
                algo_exes.vote_end_time,
                algo_exes.status as "status: _",
                algo_exes.start_time,
                algo_exes.end_time,
                algo_exes.result,
                algo_exes.error_msg,
                algo_exes.created_at,
                algo_exes.updated_at,
                algos.name as algo_name,
                algos.algo_link,
                algos.cid
            FROM algo_exes
            JOIN blockchain_transactions bt
            ON bt.entity_id = algo_exes.id
               AND bt.status = $1
               AND bt.entity_type = $2
            JOIN algos ON algos.id = algo_exes.algo_id
            ORDER BY algo_exes.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Execution.as_str(),
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;

        Ok((items, total as u32))
    }

    /// Create a new algorithm execution with transaction
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        request: CreateAlgoExe,
    ) -> Result<Self> {
        let exe = sqlx::query_as!(
            AlgoExe,
            r#"
            INSERT INTO algo_exes (algo_id, used_dataset, scientist_wallet, review_status, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, algo_id, used_dataset, scientist_wallet,
                      review_status as "review_status: _",
                      vote_start_time, vote_end_time,
                      status as "status: _",
                      start_time, end_time, result, error_msg,
                      created_at, updated_at
            "#,
            request.algo_id,
            &request.used_dataset,
            &request.scientist_wallet,
            request.review_status as _,
            request.status as _
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(exe)
    }
}

impl Timestamped for AlgoExe {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Algorithm execution with joined algorithm data
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgoExeWithAlgo {
    // AlgoExe fields
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
    // Algo fields
    pub algo_name: String,
    pub algo_link: String,
    pub cid: String,
}

impl AlgoExeWithAlgo {
    /// Get paginated executions with algorithm info
    pub async fn find_paginated(
        pool: &PgPool,
        pagination: PaginationParams,
    ) -> Result<PaginatedResponse<AlgoExeWithAlgo>> {
        // Get total count
        // Get total count first
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM algo_exes
            JOIN blockchain_transactions bt
            ON bt.entity_id = algo_exes.id
               AND bt.status = $1
               AND bt.entity_type = $2
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Execution.as_str()
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results with join
        let exes = sqlx::query_as!(
            AlgoExeWithAlgo,
            r#"
    SELECT
        algo_exes.id,
        algo_exes.algo_id,
        algo_exes.used_dataset,
        algo_exes.scientist_wallet,
        algo_exes.review_status as "review_status: _",
        algo_exes.vote_start_time,
        algo_exes.vote_end_time,
        algo_exes.status as "status: _",
        algo_exes.start_time,
        algo_exes.end_time,
        algo_exes.result,
        algo_exes.error_msg,
        algo_exes.created_at,
        algo_exes.updated_at,
        algos.name as algo_name,
        algos.algo_link,
        algos.cid
    FROM algo_exes
    JOIN blockchain_transactions bt
    ON bt.entity_id = algo_exes.id
       AND bt.status = $1
       AND bt.entity_type = $2
    JOIN algos ON algos.id = algo_exes.algo_id
    ORDER BY algo_exes.created_at DESC
    LIMIT $3 OFFSET $4
    "#,
            TransactionStatus::Confirmed as _,
            EntityType::Execution.as_str(),
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedResponse::new(exes, &pagination, total as u64))
    }
}

/// Request to create a new algorithm execution
#[derive(Debug, Clone, Deserialize)]
pub struct CreateAlgoExe {
    pub algo_id: i64,
    pub status: AlgoExeStatus,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: AlgoReviewStatus,
}

/// Request to create a new algorithm execution (for Create trait)
#[derive(Debug, Clone, Deserialize)]
pub struct CreateAlgoExeRequest {
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
}

#[async_trait::async_trait]
impl Create for AlgoExe {
    type Request = CreateAlgoExeRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let exe = sqlx::query_as!(
            AlgoExe,
            r#"
            INSERT INTO algo_exes (algo_id, used_dataset, scientist_wallet, review_status, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, algo_id, used_dataset, scientist_wallet,
                      review_status as "review_status: _",
                      vote_start_time, vote_end_time,
                      status as "status: _",
                      start_time, end_time, result, error_msg,
                      created_at, updated_at
            "#,
            request.algo_id,
            &request.used_dataset,
            &request.scientist_wallet,
            AlgoReviewStatus::Reviewing as _,
            AlgoExeStatus::Queued as _
        )
        .fetch_one(pool)
        .await?;

        Ok(exe)
    }
}

#[async_trait::async_trait]
impl FindById for AlgoExe {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let exe = sqlx::query_as!(
            AlgoExe,
            r#"
            SELECT id, algo_id, used_dataset, scientist_wallet,
                   review_status as "review_status: _",
                   vote_start_time, vote_end_time,
                   status as "status: _",
                   start_time, end_time, result, error_msg,
                   created_at, updated_at
            FROM algo_exes WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(exe)
    }
}
