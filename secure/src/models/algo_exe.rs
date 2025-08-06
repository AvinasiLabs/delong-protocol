use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{
    blockchain_transaction::EntityType,
    pg_types::{ExecutionStatus, ReviewStatus, TransactionStatus},
    Create, FindById, Timestamped,
};
use crate::error::Result;

/// Algorithm execution entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgoExe {
    pub id: i64,
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: ReviewStatus,
    pub vote_start_time: Option<DateTime<Utc>>,
    pub vote_end_time: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
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
            .bind(ExecutionStatus::Running)
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
            .bind(ReviewStatus::Reviewing)
            .fetch_all(pool)
            .await?;

        Ok(exes)
    }

    /// Find by review status
    pub async fn find_by_review_status(
        pool: &PgPool,
        review_status: ReviewStatus,
    ) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            AlgoExe,
            r#"
            SELECT id, algo_id, used_dataset, scientist_wallet,
                   review_status as "review_status: ReviewStatus",
                   vote_start_time, vote_end_time,
                   status as "status: ExecutionStatus",
                   start_time, end_time, result, error_msg,
                   created_at, updated_at
            FROM algo_exes
            WHERE review_status = $1
            "#,
            review_status as _
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Update review status
    pub async fn update_review_status(
        pool: &PgPool,
        id: i64,
        review_status: ReviewStatus,
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
    pub async fn update_status(pool: &PgPool, id: i64, status: ExecutionStatus) -> Result<Self> {
        let now = Utc::now();

        if status == ExecutionStatus::Running {
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
        result: String,
        error_msg: Option<String>,
    ) -> Result<Self> {
        let now = Utc::now();
        let status = if error_msg.is_some() {
            ExecutionStatus::Failed
        } else {
            ExecutionStatus::Completed
        };

        sqlx::query!(
            r#"
            UPDATE algo_exes
            SET status = $1, result = $2, error_msg = $3, end_time = $4, updated_at = $5
            WHERE id = $6
            "#,
            status as _,
            Some(result),
            error_msg,
            now,
            now,
            id
        )
        .execute(pool)
        .await?;

        Self::find_by_id_required(pool, id).await
    }

    /// Find executions that are pending to run (APPROVED and QUEUED)
    pub async fn find_pending_to_run(pool: &PgPool) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            Self,
            r#"
            SELECT
                algo_exes.id, algo_exes.algo_id, algo_exes.used_dataset, algo_exes.scientist_wallet,
                algo_exes.review_status as "review_status: ReviewStatus",
                algo_exes.vote_start_time, algo_exes.vote_end_time,
                algo_exes.status as "status: ExecutionStatus",
                algo_exes.start_time, algo_exes.end_time, algo_exes.result, algo_exes.error_msg,
                algo_exes.created_at, algo_exes.updated_at
            FROM algo_exes
            JOIN blockchain_transactions bt
            ON bt.entity_id = algo_exes.id
               AND bt.status = $1
               AND bt.entity_type = $2
            WHERE algo_exes.review_status = $3
              AND algo_exes.status = $4
            ORDER BY algo_exes.created_at ASC
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Execution as _,
            ReviewStatus::Approved as _,
            ExecutionStatus::Queued as _
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Find algo execution by algorithm CID
    pub async fn find_by_algo_cid(pool: &PgPool, algo_cid: &str) -> Result<Option<Self>> {
        let exe = sqlx::query_as!(
            Self,
            r#"
            SELECT
                ae.id, ae.algo_id, ae.used_dataset, ae.scientist_wallet,
                ae.review_status as "review_status: ReviewStatus",
                ae.vote_start_time, ae.vote_end_time,
                ae.status as "status: ExecutionStatus",
                ae.start_time, ae.end_time, ae.result, ae.error_msg,
                ae.created_at, ae.updated_at
            FROM algo_exes ae
            JOIN algos a ON ae.algo_id = a.id
            WHERE a.cid = $1
            ORDER BY ae.created_at DESC
            LIMIT 1
            "#,
            algo_cid
        )
        .fetch_optional(pool)
        .await?;

        Ok(exe)
    }

    /// Update review status by algorithm CID
    pub async fn update_review_status_by_cid(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        algo_cid: &str,
        review_status: ReviewStatus,
    ) -> Result<()> {
        let result = sqlx::query!(
            r#"
            UPDATE algo_exes
            SET review_status = $1, updated_at = NOW()
            WHERE algo_id = (
                SELECT id FROM algos WHERE cid = $2
            )
            "#,
            review_status as _,
            algo_cid
        )
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AppError::NotFound(format!(
                "AlgoExe with algo_cid {} not found",
                algo_cid
            )));
        }

        Ok(())
    }

    /// Find executions by status
    pub async fn find_by_status(pool: &PgPool, status: ExecutionStatus) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            Self,
            r#"
            SELECT
                id, algo_id, used_dataset, scientist_wallet,
                review_status as "review_status: ReviewStatus",
                vote_start_time, vote_end_time,
                status as "status: ExecutionStatus",
                start_time, end_time, result, error_msg,
                created_at, updated_at
            FROM algo_exes
            WHERE status = $1
            "#,
            status as _
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Find by ID with required result
    pub async fn find_by_id_required(pool: &PgPool, id: i64) -> Result<Self> {
        Self::find_by_id(pool, id).await?.ok_or_else(|| {
            crate::error::AppError::NotFound(format!(
                "Algorithm execution with ID {} not found",
                id
            ))
        })
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

/// Response for algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgoExeResponse {
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

impl From<AlgoExe> for AlgoExeResponse {
    fn from(algo_exe: AlgoExe) -> Self {
        Self {
            id: algo_exe.id,
            algo_id: algo_exe.algo_id,
            used_dataset: algo_exe.used_dataset,
            scientist_wallet: algo_exe.scientist_wallet,
            review_status: algo_exe.review_status.to_string(),
            vote_start_time: algo_exe.vote_start_time,
            vote_end_time: algo_exe.vote_end_time,
            status: algo_exe.status.to_string(),
            start_time: algo_exe.start_time,
            end_time: algo_exe.end_time,
            result: algo_exe.result,
            error_msg: algo_exe.error_msg,
            created_at: algo_exe.created_at,
            updated_at: algo_exe.updated_at,
        }
    }
}

/// Algorithm execution with algorithm details
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgoExeWithAlgo {
    // AlgoExe fields
    pub id: i64,
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: ReviewStatus,
    pub vote_start_time: Option<DateTime<Utc>>,
    pub vote_end_time: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
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
    /// List algorithm executions with algorithm details
    pub async fn list(
        pool: &PgPool,
        scientist_wallet: Option<&str>,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<Self>, u64)> {
        let mut count_query =
            "SELECT COUNT(*) as \"count!\" FROM algo_exes ae JOIN algos a ON ae.algo_id = a.id"
                .to_string();
        let mut data_query = r#"
            SELECT ae.id, ae.algo_id, ae.used_dataset, ae.scientist_wallet,
                   ae.review_status,
                   ae.vote_start_time, ae.vote_end_time,
                   ae.status,
                   ae.start_time, ae.end_time, ae.result, ae.error_msg,
                   ae.created_at, ae.updated_at,
                   a.name as algo_name, a.algo_link, a.cid
            FROM algo_exes ae
            JOIN algos a ON ae.algo_id = a.id
        "#
        .to_string();

        if let Some(_wallet) = scientist_wallet {
            let where_clause = " WHERE ae.scientist_wallet = $1";
            count_query.push_str(where_clause);
            data_query.push_str(where_clause);
        }

        let (limit_param, offset_param) = if scientist_wallet.is_some() {
            ("$2", "$3")
        } else {
            ("$1", "$2")
        };
        data_query.push_str(&format!(
            " ORDER BY ae.created_at DESC LIMIT {} OFFSET {}",
            limit_param, offset_param
        ));

        let total = if let Some(wallet) = scientist_wallet {
            sqlx::query_scalar::<_, i64>(&count_query)
                .bind(wallet)
                .fetch_one(pool)
                .await?
        } else {
            sqlx::query_scalar::<_, i64>(&count_query)
                .fetch_one(pool)
                .await?
        };

        let exes = if let Some(wallet) = scientist_wallet {
            sqlx::query_as::<_, Self>(&data_query)
                .bind(wallet)
                .bind(per_page as i64)
                .bind(((page - 1) * per_page) as i64)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, Self>(&data_query)
                .bind(per_page as i64)
                .bind(((page - 1) * per_page) as i64)
                .fetch_all(pool)
                .await?
        };

        Ok((exes, total as u64))
    }
}

/// Request to create a new algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgoExe {
    pub algo_id: i64,
    pub status: ExecutionStatus,
    pub used_dataset: String,
    pub scientist_wallet: String,
    pub review_status: ReviewStatus,
}

/// Create algorithm execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgoExeRequest {
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
}

#[async_trait::async_trait]
impl Create for AlgoExe {
    type Request = CreateAlgoExe;

    async fn create(pool: &PgPool, req: Self::Request) -> Result<Self> {
        let exe = sqlx::query_as!(
            AlgoExe,
            r#"
            INSERT INTO algo_exes (algo_id, used_dataset, scientist_wallet, review_status, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, algo_id, used_dataset, scientist_wallet,
                     review_status as "review_status: ReviewStatus",
                     vote_start_time, vote_end_time,
                     status as "status: ExecutionStatus",
                     start_time, end_time, result, error_msg,
                     created_at, updated_at
            "#,
            req.algo_id,
            req.used_dataset,
            req.scientist_wallet,
            req.review_status as _,
            req.status as _
        )
        .fetch_one(pool)
        .await?;

        Ok(exe)
    }
}

impl AlgoExe {
    pub async fn create_with_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: CreateAlgoExe,
    ) -> Result<Self> {
        let exe = sqlx::query_as!(
            AlgoExe,
            r#"
            INSERT INTO algo_exes (algo_id, used_dataset, scientist_wallet, review_status, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, algo_id, used_dataset, scientist_wallet,
                     review_status as "review_status: ReviewStatus",
                     vote_start_time, vote_end_time,
                     status as "status: ExecutionStatus",
                     start_time, end_time, result, error_msg,
                     created_at, updated_at
            "#,
            req.algo_id,
            req.used_dataset,
            req.scientist_wallet,
            req.review_status as _,
            req.status as _
        )
        .fetch_one(&mut **tx)
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
                   review_status as "review_status: ReviewStatus",
                   vote_start_time, vote_end_time,
                   status as "status: ExecutionStatus",
                   start_time, end_time, result, error_msg,
                   created_at, updated_at
            FROM algo_exes
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(exe)
    }
}
