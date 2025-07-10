use common::{ApiError, ApiResult, models::BlockchainTransaction};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Request for creating a new blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlockchainTransactionRequest {
    pub tx_hash: String,
    pub entity_id: i64,
    pub entity_type: Option<String>,
    pub status: Option<String>,
    pub block_number: Option<i64>,
    pub block_timestamp: Option<DateTime<Utc>>,
}

/// Blockchain transaction service for database operations
pub struct BlockchainService;

impl BlockchainService {
    /// Create a new blockchain transaction record
    pub async fn create_transaction(
        pool: &PgPool,
        req: CreateBlockchainTransactionRequest,
    ) -> ApiResult<BlockchainTransaction> {
        let transaction = sqlx::query_as!(
            BlockchainTransaction,
            r#"
            INSERT INTO blockchain_transactions (
                tx_hash, entity_id, entity_type, status, block_number, block_timestamp
            ) VALUES ($1, $2, $3::text::entity_type, $4::text::transaction_status, $5, $6)
            RETURNING id, tx_hash, entity_id, entity_type::text as entity_type, 
                      status::text as status, block_number, block_timestamp, created_at, updated_at
            "#,
            req.tx_hash,
            req.entity_id,
            req.entity_type,
            req.status.unwrap_or_else(|| "PENDING".to_string()),
            req.block_number,
            req.block_timestamp
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create blockchain transaction");
            ApiError::DatabaseError("Failed to create blockchain transaction".to_string())
        })?;

        Ok(transaction)
    }

    /// Get blockchain transaction by hash
    pub async fn get_transaction_by_hash(
        pool: &PgPool,
        tx_hash: &str,
    ) -> ApiResult<BlockchainTransaction> {
        let transaction = sqlx::query_as!(
            BlockchainTransaction,
            "SELECT id, tx_hash, entity_id, entity_type::text as entity_type, status::text as status, block_number, block_timestamp, created_at, updated_at FROM blockchain_transactions WHERE tx_hash = $1",
            tx_hash
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tx_hash = %tx_hash, "Failed to get blockchain transaction");
            ApiError::NotFound
        })?;

        Ok(transaction)
    }

    /// Update blockchain transaction status
    pub async fn update_transaction_status(
        pool: &PgPool,
        tx_hash: &str,
        status: &str,
        block_number: Option<i64>,
        block_timestamp: Option<DateTime<Utc>>,
    ) -> ApiResult<BlockchainTransaction> {
        let transaction = sqlx::query_as!(
            BlockchainTransaction,
            r#"
            UPDATE blockchain_transactions
            SET status = $1::text::transaction_status,
                block_number = COALESCE($2, block_number),
                block_timestamp = COALESCE($3, block_timestamp),
                updated_at = CURRENT_TIMESTAMP
            WHERE tx_hash = $4
            RETURNING id, tx_hash, entity_id, entity_type::text as entity_type, 
                      status::text as status, block_number, block_timestamp, created_at, updated_at
            "#,
            status,
            block_number,
            block_timestamp,
            tx_hash
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, tx_hash = %tx_hash, "Failed to update blockchain transaction");
            ApiError::DatabaseError("Failed to update blockchain transaction".to_string())
        })?;

        Ok(transaction)
    }

    /// Get pending blockchain transactions
    pub async fn get_pending_transactions(
        pool: &PgPool,
    ) -> ApiResult<Vec<BlockchainTransaction>> {
        let transactions = sqlx::query_as!(
            BlockchainTransaction,
            "SELECT id, tx_hash, entity_id, entity_type::text as entity_type, status::text as status, block_number, block_timestamp, created_at, updated_at FROM blockchain_transactions WHERE status = 'PENDING'"
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get pending blockchain transactions");
            ApiError::DatabaseError("Failed to get pending transactions".to_string())
        })?;

        Ok(transactions)
    }

    /// Get transactions by entity
    pub async fn get_transactions_by_entity(
        pool: &PgPool,
        entity_id: i64,
        entity_type: &str,
    ) -> ApiResult<Vec<BlockchainTransaction>> {
        let transactions = sqlx::query_as!(
            BlockchainTransaction,
            "SELECT id, tx_hash, entity_id, entity_type::text as entity_type, status::text as status, block_number, block_timestamp, created_at, updated_at FROM blockchain_transactions WHERE entity_id = $1 AND entity_type = $2::text::entity_type",
            entity_id,
            entity_type
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, entity_id = %entity_id, entity_type = %entity_type, "Failed to get transactions by entity");
            ApiError::DatabaseError("Failed to get transactions by entity".to_string())
        })?;

        Ok(transactions)
    }

    /// Check if entity is confirmed (has at least one confirmed transaction)
    pub async fn is_entity_confirmed(
        pool: &PgPool,
        entity_id: i64,
        entity_type: &str,
    ) -> ApiResult<bool> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM blockchain_transactions
            WHERE entity_id = $1 AND entity_type = $2::text::entity_type AND status = 'CONFIRMED'
            "#,
            entity_id,
            entity_type
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, entity_id = %entity_id, entity_type = %entity_type, "Failed to check entity confirmation");
            ApiError::DatabaseError("Failed to check entity confirmation".to_string())
        })?
        .unwrap_or(0);

        Ok(count > 0)
    }
} 