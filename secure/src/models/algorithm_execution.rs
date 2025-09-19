use avinapi::{query::PaginationQuery, transport::response::PaginatedData};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Type};
use std::fmt;

use super::{Create, FindById, Timestamped};
use crate::Result;

/// Algorithm review status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "algo_review_status", rename_all = "lowercase")]
#[serde(rename_all = "UPPERCASE")]
pub enum ReviewStatus {
    Reviewing,
    Approved,
    Rejected,
}

impl fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewStatus::Reviewing => write!(f, "reviewing"),
            ReviewStatus::Approved => write!(f, "approved"),
            ReviewStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Algorithm execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "algo_exe_status", rename_all = "lowercase")]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionStatus::Queued => write!(f, "queued"),
            ExecutionStatus::Running => write!(f, "running"),
            ExecutionStatus::Completed => write!(f, "completed"),
            ExecutionStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Unified algorithm execution tracking entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AlgorithmExecution {
    pub id: i64,
    // Algorithm information
    pub algo_id: i64,
    pub algo_name: Option<String>,
    pub algo_cid: String,
    pub algo_link: Option<String>,
    // Dataset information
    pub dataset_id: Option<i64>,
    pub dataset_name: String,
    pub dataset_cid: Option<String>,
    // User information
    pub wallet: String,
    pub user_id: Option<i64>,
    // Execution lifecycle
    pub review_status: String,
    pub execution_status: String,
    // Review timestamps
    pub vote_start_time: Option<DateTime<Utc>>,
    pub vote_end_time: Option<DateTime<Utc>>,
    // Execution timestamps
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    // Execution results
    pub success: Option<bool>,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub error_type: Option<String>,
    pub container_exit_code: Option<i32>,
    // Performance metrics
    pub runtime_seconds: Option<i32>,
    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Execution context for AlgoExecutor
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub id: i64,
    pub algo_id: i64,
    pub algo_name: Option<String>,
    pub algo_cid: String,
    pub dataset_name: String,
    pub dataset_id: Option<i64>,
    pub dataset_cid: Option<String>,
    pub wallet: String,
}

/// Container execution result
#[derive(Debug, Clone)]
pub enum ContainerResult {
    Success { output: String, exit_code: i32 },
    Failed { error: String, exit_code: i32 },
    Timeout,
}

/// Request to create a new algorithm execution
#[derive(Debug, Clone, Deserialize)]
pub struct CreateAlgorithmExecutionRequest {
    pub algo_id: i64,
    pub algo_name: Option<String>,
    pub algo_cid: String,
    pub algo_link: Option<String>,
    pub dataset_name: String,
    pub wallet: String,
    pub user_id: Option<i64>,
}

/// API response for algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmExecutionResponse {
    pub id: i64,
    pub algo_id: i64,
    pub algo_name: Option<String>,
    pub algo_cid: String,
    pub dataset_name: String,
    pub wallet: String,
    pub review_status: String,
    pub execution_status: String,
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: Option<bool>,
    pub output: Option<String>,
    pub error_message: Option<String>,
    pub runtime_seconds: Option<i32>,
}

/// Execution statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionStats {
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub average_runtime_seconds: Option<f64>,
    pub total_runtime_seconds: Option<i64>,
}

impl Timestamped for AlgorithmExecution {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

impl AlgorithmExecution {
    /// Load execution context for AlgoExecutor
    pub async fn load_context(pool: &PgPool, exe_id: i64) -> Result<ExecutionContext> {
        let ctx = sqlx::query_as!(
            ExecutionContext,
            r#"
            SELECT
                ae.id,
                ae.algo_id,
                ae.algo_name,
                ae.algo_cid,
                ae.dataset_name,
                ae.wallet,
                d.id as dataset_id,
                d.ipfs_cid as dataset_cid
            FROM algorithm_execution ae
            LEFT JOIN dataset d ON ae.dataset_name = d.name
            WHERE ae.id = $1
            "#,
            exe_id
        )
        .fetch_one(pool)
        .await?;

        Ok(ctx)
    }

    /// Update execution status
    pub async fn update_status(pool: &PgPool, exe_id: i64, status: &str) -> Result<()> {
        let now = Utc::now();
        let started_at = if status == "running" {
            Some(now)
        } else {
            None
        };

        sqlx::query!(
            r#"
            UPDATE algorithm_execution
            SET execution_status = $1,
                started_at = COALESCE($2, started_at),
                updated_at = NOW()
            WHERE id = $3
            "#,
            status,
            started_at,
            exe_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Record execution result
    pub async fn record_result(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        exe_id: i64,
        result: ContainerResult,
    ) -> Result<()> {
        let (success, output, error_msg, error_type, exit_code) = match result {
            ContainerResult::Success { output, exit_code } => {
                (true, Some(output), None, None, exit_code)
            }
            ContainerResult::Failed { error, exit_code } => {
                let error_type = Self::classify_error(&error);
                (false, None, Some(error), Some(error_type), exit_code)
            }
            ContainerResult::Timeout => {
                (false, None, Some("Execution timeout".to_string()), Some("timeout".to_string()), -1)
            }
        };

        sqlx::query!(
            r#"
            UPDATE algorithm_execution
            SET
                execution_status = $1,
                success = $2,
                output = $3,
                error_message = $4,
                error_type = $5,
                container_exit_code = $6,
                completed_at = NOW(),
                runtime_seconds = EXTRACT(EPOCH FROM (NOW() - started_at))::INTEGER,
                updated_at = NOW()
            WHERE id = $7
            "#,
            if success { "completed" } else { "failed" },
            success,
            output,
            error_msg,
            error_type,
            exit_code,
            exe_id
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Classify error type
    fn classify_error(error_msg: &str) -> String {
        if error_msg.contains("timeout") {
            "timeout".to_string()
        } else if error_msg.contains("memory") || error_msg.contains("resource") {
            "resource_limit".to_string()
        } else if error_msg.contains("compile") || error_msg.contains("syntax") {
            "compilation".to_string()
        } else {
            "runtime".to_string()
        }
    }

    /// Find executions by wallet
    pub async fn find_by_wallet(
        pool: &PgPool,
        wallet: &str,
        pagination: &PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM algorithm_execution WHERE wallet = $1",
            wallet
        )
        .fetch_one(pool)
        .await?;

        let executions = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                id, algo_id, algo_name, algo_cid, algo_link,
                dataset_id, dataset_name, dataset_cid,
                wallet, user_id,
                review_status, execution_status,
                vote_start_time, vote_end_time,
                submitted_at, started_at, completed_at,
                success, output, error_message, error_type, container_exit_code,
                runtime_seconds, created_at, updated_at
            FROM algorithm_execution
            WHERE wallet = $1
            ORDER BY submitted_at DESC
            LIMIT $2 OFFSET $3
            "#,
            wallet,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: executions,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Find executions by algorithm CID
    pub async fn find_by_algo_cid(
        pool: &PgPool,
        algo_cid: &str,
        pagination: &PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM algorithm_execution WHERE algo_cid = $1",
            algo_cid
        )
        .fetch_one(pool)
        .await?;

        let executions = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                id, algo_id, algo_name, algo_cid, algo_link,
                dataset_id, dataset_name, dataset_cid,
                wallet, user_id,
                review_status, execution_status,
                vote_start_time, vote_end_time,
                submitted_at, started_at, completed_at,
                success, output, error_message, error_type, container_exit_code,
                runtime_seconds, created_at, updated_at
            FROM algorithm_execution
            WHERE algo_cid = $1
            ORDER BY submitted_at DESC
            LIMIT $2 OFFSET $3
            "#,
            algo_cid,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: executions,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Find executions by dataset
    pub async fn find_by_dataset(
        pool: &PgPool,
        dataset_name: &str,
        pagination: &PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM algorithm_execution WHERE dataset_name = $1",
            dataset_name
        )
        .fetch_one(pool)
        .await?;

        let executions = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                id, algo_id, algo_name, algo_cid, algo_link,
                dataset_id, dataset_name, dataset_cid,
                wallet, user_id,
                review_status, execution_status,
                vote_start_time, vote_end_time,
                submitted_at, started_at, completed_at,
                success, output, error_message, error_type, container_exit_code,
                runtime_seconds, created_at, updated_at
            FROM algorithm_execution
            WHERE dataset_name = $1
            ORDER BY submitted_at DESC
            LIMIT $2 OFFSET $3
            "#,
            dataset_name,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: executions,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Get execution statistics
    pub async fn get_stats(pool: &PgPool, wallet: Option<&str>) -> Result<ExecutionStats> {
        let stats = if let Some(wallet) = wallet {
            sqlx::query_as!(
                ExecutionStats,
                r#"
                SELECT
                    COUNT(*) as "total_executions!",
                    COUNT(CASE WHEN success = true THEN 1 END) as "successful_executions!",
                    COUNT(CASE WHEN success = false THEN 1 END) as "failed_executions!",
                    AVG(runtime_seconds)::DOUBLE PRECISION as average_runtime_seconds,
                    SUM(runtime_seconds) as total_runtime_seconds
                FROM algorithm_execution
                WHERE wallet = $1
                "#,
                wallet
            )
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_as!(
                ExecutionStats,
                r#"
                SELECT
                    COUNT(*) as "total_executions!",
                    COUNT(CASE WHEN success = true THEN 1 END) as "successful_executions!",
                    COUNT(CASE WHEN success = false THEN 1 END) as "failed_executions!",
                    AVG(runtime_seconds)::DOUBLE PRECISION as average_runtime_seconds,
                    SUM(runtime_seconds) as total_runtime_seconds
                FROM algorithm_execution
                "#
            )
            .fetch_one(pool)
            .await?
        };

        Ok(stats)
    }

    /// Convert to API response
    pub fn to_response(&self) -> AlgorithmExecutionResponse {
        AlgorithmExecutionResponse {
            id: self.id,
            algo_id: self.algo_id,
            algo_name: self.algo_name.clone(),
            algo_cid: self.algo_cid.clone(),
            dataset_name: self.dataset_name.clone(),
            wallet: self.wallet.clone(),
            review_status: self.review_status.clone(),
            execution_status: self.execution_status.clone(),
            submitted_at: self.submitted_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            success: self.success,
            output: self.output.clone(),
            error_message: self.error_message.clone(),
            runtime_seconds: self.runtime_seconds,
        }
    }

    /// Find pending executions (RUNNING status) with confirmed transactions
    pub async fn find_pending_confirmed(pool: &PgPool) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                ae.id, ae.algo_id, ae.algo_name, ae.algo_cid, ae.algo_link,
                ae.dataset_id, ae.dataset_name, ae.dataset_cid,
                ae.wallet, ae.user_id,
                ae.review_status, ae.execution_status,
                ae.vote_start_time, ae.vote_end_time,
                ae.submitted_at, ae.started_at, ae.completed_at,
                ae.success, ae.output, ae.error_message, ae.error_type, ae.container_exit_code,
                ae.runtime_seconds, ae.created_at, ae.updated_at
            FROM algorithm_execution ae
            JOIN transaction bt
            ON bt.entity_id = ae.id
               AND bt.status = 'confirmed'
               AND bt.entity_type = 'execution'
            WHERE ae.execution_status = 'running'
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Find reviewing executions with confirmed transactions
    pub async fn find_reviewing_confirmed(pool: &PgPool) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                ae.id, ae.algo_id, ae.algo_name, ae.algo_cid, ae.algo_link,
                ae.dataset_id, ae.dataset_name, ae.dataset_cid,
                ae.wallet, ae.user_id,
                ae.review_status, ae.execution_status,
                ae.vote_start_time, ae.vote_end_time,
                ae.submitted_at, ae.started_at, ae.completed_at,
                ae.success, ae.output, ae.error_message, ae.error_type, ae.container_exit_code,
                ae.runtime_seconds, ae.created_at, ae.updated_at
            FROM algorithm_execution ae
            JOIN transaction bt
            ON bt.entity_id = ae.id
               AND bt.status = 'confirmed'
               AND bt.entity_type = 'execution'
            WHERE ae.review_status = 'reviewing'
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Find executions that are pending to run (APPROVED and QUEUED)
    pub async fn find_pending_to_run(pool: &PgPool) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                ae.id, ae.algo_id, ae.algo_name, ae.algo_cid, ae.algo_link,
                ae.dataset_id, ae.dataset_name, ae.dataset_cid,
                ae.wallet, ae.user_id,
                ae.review_status, ae.execution_status,
                ae.vote_start_time, ae.vote_end_time,
                ae.submitted_at, ae.started_at, ae.completed_at,
                ae.success, ae.output, ae.error_message, ae.error_type, ae.container_exit_code,
                ae.runtime_seconds, ae.created_at, ae.updated_at
            FROM algorithm_execution ae
            JOIN transaction bt
            ON bt.entity_id = ae.id
               AND bt.status = 'confirmed'
               AND bt.entity_type = 'execution'
            WHERE ae.review_status = 'approved'
              AND ae.execution_status = 'queued'
            ORDER BY ae.created_at ASC
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Find by review status
    pub async fn find_by_review_status(
        pool: &PgPool,
        review_status: &str,
    ) -> Result<Vec<Self>> {
        let exes = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                id, algo_id, algo_name, algo_cid, algo_link,
                dataset_id, dataset_name, dataset_cid,
                wallet, user_id,
                review_status, execution_status,
                vote_start_time, vote_end_time,
                submitted_at, started_at, completed_at,
                success, output, error_message, error_type, container_exit_code,
                runtime_seconds, created_at, updated_at
            FROM algorithm_execution
            WHERE review_status = $1
            "#,
            review_status
        )
        .fetch_all(pool)
        .await?;

        Ok(exes)
    }

    /// Update review status
    pub async fn update_review_status(
        pool: &PgPool,
        id: i64,
        review_status: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE algorithm_execution
            SET review_status = $1, updated_at = NOW()
            WHERE id = $2
            "#,
            review_status,
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
            UPDATE algorithm_execution
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

    /// Update execution completed
    pub async fn update_completed(
        pool: &PgPool,
        id: i64,
        result: String,
        error_msg: Option<String>,
    ) -> Result<Self> {
        let now = Utc::now();
        let status = if error_msg.is_some() {
            "failed"
        } else {
            "completed"
        };

        sqlx::query!(
            r#"
            UPDATE algorithm_execution
            SET execution_status = $1, output = $2, error_message = $3, completed_at = $4, updated_at = $5
            WHERE id = $6
            "#,
            status,
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

    /// Find by ID with required result
    pub async fn find_by_id_required(pool: &PgPool, id: i64) -> Result<Self> {
        Self::find_by_id(pool, id).await?.ok_or_else(|| {
            crate::AppError::NotFound(format!("Algorithm execution with ID {} not found", id))
        })
    }
}

#[async_trait::async_trait]
impl Create for AlgorithmExecution {
    type Request = CreateAlgorithmExecutionRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            INSERT INTO algorithm_execution (
                algo_id, algo_name, algo_cid, algo_link,
                dataset_name, wallet, user_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id, algo_id, algo_name, algo_cid, algo_link,
                dataset_id, dataset_name, dataset_cid,
                wallet, user_id,
                review_status, execution_status,
                vote_start_time, vote_end_time,
                submitted_at, started_at, completed_at,
                success, output, error_message, error_type, container_exit_code,
                runtime_seconds, created_at, updated_at
            "#,
            request.algo_id,
            request.algo_name,
            request.algo_cid,
            request.algo_link,
            request.dataset_name,
            request.wallet,
            request.user_id
        )
        .fetch_one(pool)
        .await?;

        Ok(execution)
    }
}

#[async_trait::async_trait]
impl FindById for AlgorithmExecution {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT
                id, algo_id, algo_name, algo_cid, algo_link,
                dataset_id, dataset_name, dataset_cid,
                wallet, user_id,
                review_status, execution_status,
                vote_start_time, vote_end_time,
                submitted_at, started_at, completed_at,
                success, output, error_message, error_type, container_exit_code,
                runtime_seconds, created_at, updated_at
            FROM algorithm_execution WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(execution)
    }
}