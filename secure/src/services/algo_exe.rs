use common::{ApiError, ApiResult, PaginationParams, PaginatedResponse};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Algorithm execution database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlgorithmExecution {
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

/// Algorithm execution with algorithm info
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlgorithmExecutionWithAlgo {
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
    pub cid: String,
}

/// Algorithm database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Algorithm {
    pub id: i64,
    pub name: String,
    pub algo_link: String,
    pub cid: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request for creating a new algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgorithmExecutionRequest {
    pub algo_id: i64,
    pub used_dataset: String,
    pub scientist_wallet: String,
}

/// Request for creating a new algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgorithmRequest {
    pub name: String,
    pub algo_link: String,
    pub cid: String,
}

/// Algorithm execution service for database operations
pub struct AlgoExeService;

impl AlgoExeService {
    /// Create a new algorithm
    pub async fn create_algorithm(
        pool: &PgPool,
        req: CreateAlgorithmRequest,
    ) -> ApiResult<Algorithm> {
        let algorithm = sqlx::query_as!(
            Algorithm,
            r#"
            INSERT INTO algorithms (name, algo_link, cid)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
            req.name,
            req.algo_link,
            req.cid
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create algorithm");
            ApiError::InternalError("Failed to create algorithm".to_string())
        })?;

        Ok(algorithm)
    }

    /// Get algorithm by ID
    pub async fn get_algorithm_by_id(
        pool: &PgPool,
        id: i64,
    ) -> ApiResult<Algorithm> {
        let algorithm = sqlx::query_as!(
            Algorithm,
            "SELECT * FROM algorithms WHERE id = $1",
            id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, algorithm_id = %id, "Failed to get algorithm");
            ApiError::NotFound("Algorithm not found".to_string())
        })?;

        Ok(algorithm)
    }

    /// Get algorithm by link
    pub async fn get_algorithm_by_link(
        pool: &PgPool,
        algo_link: &str,
    ) -> ApiResult<Option<Algorithm>> {
        let algorithm = sqlx::query_as!(
            Algorithm,
            "SELECT * FROM algorithms WHERE algo_link = $1",
            algo_link
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, algo_link = %algo_link, "Failed to get algorithm by link");
            ApiError::InternalError("Failed to get algorithm".to_string())
        })?;

        Ok(algorithm)
    }

    /// Create a new algorithm execution
    pub async fn create_algorithm_execution(
        pool: &PgPool,
        req: CreateAlgorithmExecutionRequest,
    ) -> ApiResult<AlgorithmExecution> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            INSERT INTO algorithm_executions (algo_id, used_dataset, scientist_wallet)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
            req.algo_id,
            req.used_dataset,
            req.scientist_wallet
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create algorithm execution");
            ApiError::InternalError("Failed to create algorithm execution".to_string())
        })?;

        Ok(execution)
    }

    /// Get algorithm executions with pagination and algorithm info, only confirmed ones
    pub async fn get_algorithm_executions_with_algo(
        pool: &PgPool,
        params: PaginationParams,
    ) -> ApiResult<PaginatedResponse<AlgorithmExecutionWithAlgo>> {
        let offset = (params.page - 1) * params.limit;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM algorithm_executions ae
            INNER JOIN algorithms a ON a.id = ae.algo_id
            INNER JOIN blockchain_transactions bt ON bt.entity_id = ae.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'EXECUTION'
            "#
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to count algorithm executions");
            ApiError::InternalError("Failed to count executions".to_string())
        })?
        .unwrap_or(0);

        // Get paginated results
        let executions = sqlx::query_as!(
            AlgorithmExecutionWithAlgo,
            r#"
            SELECT 
                ae.id, ae.algo_id, ae.used_dataset, ae.scientist_wallet,
                ae.review_status, ae.vote_start_time, ae.vote_end_time,
                ae.status, ae.start_time, ae.end_time, ae.result, ae.error_msg,
                ae.created_at, ae.updated_at,
                a.name as algo_name, a.algo_link, a.cid
            FROM algorithm_executions ae
            INNER JOIN algorithms a ON a.id = ae.algo_id
            INNER JOIN blockchain_transactions bt ON bt.entity_id = ae.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'EXECUTION'
            ORDER BY ae.created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            params.limit as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get algorithm executions");
            ApiError::InternalError("Failed to get executions".to_string())
        })?;

        Ok(PaginatedResponse::new(
            executions,
            params.page,
            params.limit,
            total as u64,
        ))
    }

    /// Get an algorithm execution by ID, only confirmed ones
    pub async fn get_algorithm_execution_by_id(
        pool: &PgPool,
        id: i64,
    ) -> ApiResult<AlgorithmExecution> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT ae.*
            FROM algorithm_executions ae
            INNER JOIN blockchain_transactions bt ON bt.entity_id = ae.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'EXECUTION'
            WHERE ae.id = $1
            "#,
            id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %id, "Failed to get algorithm execution");
            ApiError::NotFound("Algorithm execution not found".to_string())
        })?;

        Ok(execution)
    }

    /// Get pending algorithm executions (status = RUNNING)
    pub async fn get_pending_algorithm_executions(
        pool: &PgPool,
    ) -> ApiResult<Vec<AlgorithmExecution>> {
        let executions = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            SELECT ae.*
            FROM algorithm_executions ae
            INNER JOIN blockchain_transactions bt ON bt.entity_id = ae.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'EXECUTION'
            WHERE ae.status = 'RUNNING'
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get pending algorithm executions");
            ApiError::InternalError("Failed to get pending executions".to_string())
        })?;

        Ok(executions)
    }

    /// Update algorithm execution status
    pub async fn update_execution_status(
        pool: &PgPool,
        execution_id: i64,
        status: &str,
    ) -> ApiResult<AlgorithmExecution> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            UPDATE algorithm_executions
            SET status = $1,
                end_time = CASE 
                    WHEN $1 IN ('COMPLETED', 'FAILED') THEN CURRENT_TIMESTAMP
                    ELSE end_time
                END,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            RETURNING *
            "#,
            status,
            execution_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to update execution status");
            ApiError::InternalError("Failed to update execution status".to_string())
        })?;

        Ok(execution)
    }

    /// Update algorithm execution with result
    pub async fn update_execution_completed(
        pool: &PgPool,
        execution_id: i64,
        result: &str,
        error_msg: Option<&str>,
    ) -> ApiResult<AlgorithmExecution> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            UPDATE algorithm_executions
            SET status = 'COMPLETED',
                result = $1,
                error_msg = $2,
                end_time = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $3
            RETURNING *
            "#,
            result,
            error_msg,
            execution_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to update execution completed");
            ApiError::InternalError("Failed to update execution".to_string())
        })?;

        Ok(execution)
    }

    /// Update review status of algorithm execution
    pub async fn update_review_status(
        pool: &PgPool,
        execution_id: i64,
        review_status: &str,
    ) -> ApiResult<AlgorithmExecution> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            UPDATE algorithm_executions
            SET review_status = $1,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            RETURNING *
            "#,
            review_status,
            execution_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to update review status");
            ApiError::InternalError("Failed to update review status".to_string())
        })?;

        Ok(execution)
    }

    /// Update vote duration for algorithm execution
    pub async fn update_vote_duration(
        pool: &PgPool,
        execution_id: i64,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> ApiResult<AlgorithmExecution> {
        let execution = sqlx::query_as!(
            AlgorithmExecution,
            r#"
            UPDATE algorithm_executions
            SET vote_start_time = COALESCE($1, vote_start_time),
                vote_end_time = COALESCE($2, vote_end_time),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $3
            RETURNING *
            "#,
            start_time,
            end_time,
            execution_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %execution_id, "Failed to update vote duration");
            ApiError::InternalError("Failed to update vote duration".to_string())
        })?;

        Ok(execution)
    }
} 