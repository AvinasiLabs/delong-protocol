use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

// Blockchain transaction status constants - matches Go constants
pub const TX_STATUS_PENDING: &str = "PENDING";     // Transaction submitted but not confirmed
pub const TX_STATUS_CONFIRMED: &str = "CONFIRMED"; // Transaction confirmed on chain
pub const TX_STATUS_FAILED: &str = "FAILED";       // Transaction failed

// Entity type constants - matches Go constants
pub const ENTITY_TYPE_EXECUTION: &str = "EXECUTION";
pub const ENTITY_TYPE_VOTE: &str = "VOTE";
pub const ENTITY_TYPE_COMMITTEE: &str = "COMMITTEE";
pub const ENTITY_TYPE_TEST_REPORT: &str = "TEST_REPORT";
pub const ENTITY_TYPE_STATIC_DATASET: &str = "STATIC_DATASET";
pub const ENTITY_TYPE_DATAUSAGE: &str = "DATAUSAGE";

/// BlockchainTransaction records blockchain transactions and their status
/// Matches the Go struct from /root/delong/internal/models/blockchain.go
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BlockchainTransaction {
    pub id: i64,                                    // bigint unsigned
    pub tx_hash: String,                            // Ethereum transaction hash
    pub entity_id: i64,                             // ID of the associated entity
    pub entity_type: String,                        // Type of the associated entity: USER, ALGO, etc.
    pub status: String,                             // Transaction status: PENDING, CONFIRMED, FAILED
    pub args: serde_json::Value,
    pub error: Option<String>,
    pub block_number: Option<i64>,                  // Block number where transaction was confirmed
    pub block_timestamp: Option<DateTime<Utc>>,     // Block timestamp
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating a blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransactionRequest {
    pub tx_hash: String,
    pub entity_id: u64,
    pub entity_type: String,
    pub status: Option<String>,
    pub block_number: Option<u64>,
    pub block_timestamp: Option<DateTime<Utc>>,
}

/// Response model for blockchain transaction info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainTransactionInfo {
    pub id: u64,
    pub tx_hash: String,
    pub entity_id: u64,
    pub entity_type: String,
    pub status: String,
    pub block_number: Option<u64>,
    pub block_timestamp: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub args: serde_json::Value,
    pub error: Option<String>,
}

/// Transaction status update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTransactionStatusRequest {
    pub status: String,
    pub block_number: Option<u64>,
    pub block_timestamp: Option<DateTime<Utc>>,
}

impl From<BlockchainTransaction> for BlockchainTransactionInfo {
    fn from(tx: BlockchainTransaction) -> Self {
        BlockchainTransactionInfo {
            id: tx.id as u64,
            tx_hash: tx.tx_hash,
            entity_id: tx.entity_id as u64,
            entity_type: tx.entity_type,
            status: tx.status,
            block_number: tx.block_number.map(|n| n as u64),
            block_timestamp: Some(tx.created_at), // Placeholder, as original had this field missing
            created_at: tx.created_at,
            updated_at: tx.updated_at,
            args: tx.args,
            error: tx.error,
        }
    }
}

impl BlockchainTransaction {
    /// Create a new blockchain transaction record within a database transaction
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tx_hash: &str,
        entity_id: i64,
        entity_type: &str,
        args: &serde_json::Value,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            INSERT INTO blockchain_transactions (tx_hash, entity_id, entity_type, status, args)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(tx_hash)
        .bind(entity_id)
        .bind(entity_type)
        .bind(TX_STATUS_PENDING)
        .bind(args)
        .fetch_one(&mut **tx)
        .await
    }

    /// Create a new blockchain transaction with default pending status
    pub async fn create_with_status(
        pool: &PgPool,
        tx_hash: &str,
        entity_id: i64,
        entity_type: &str,
        status: &str,
        block_number: Option<i64>,
        block_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Self, sqlx::Error> {
        let transaction = sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            INSERT INTO blockchain_transactions (tx_hash, entity_id, entity_type, status, block_number, block_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(tx_hash)
        .bind(entity_id)
        .bind(entity_type)
        .bind(status)
        .bind(block_number)
        .bind(block_timestamp)
        .fetch_one(pool)
        .await?;

        Ok(transaction)
    }

    /// Update transaction status
    pub async fn update_status(
        pool: &PgPool,
        tx_hash: &str,
        status: &str,
        block_number: Option<i64>,
        block_time: Option<DateTime<Utc>>,
    ) -> Result<Option<Self>, sqlx::Error> {
        let transaction = sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            UPDATE blockchain_transactions
            SET status = $1, block_number = $2, block_timestamp = $3, updated_at = NOW()
            WHERE tx_hash = $4
            RETURNING *
            "#,
        )
        .bind(status)
        .bind(block_number)
        .bind(block_time)
        .bind(tx_hash)
        .fetch_optional(pool)
        .await?;

        Ok(transaction)
    }

    /// Get transactions by entity
    pub async fn get_by_entity(
        pool: &PgPool,
        entity_id: i64,
        entity_type: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let transactions = sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            SELECT * FROM blockchain_transactions
            WHERE entity_id = $1 AND entity_type = $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(entity_id)
        .bind(entity_type)
        .fetch_all(pool)
        .await?;

        Ok(transactions)
    }

    /// Get transactions by status
    pub async fn get_by_status(pool: &PgPool, status: &str) -> Result<Vec<Self>, sqlx::Error> {
        let transactions = sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            SELECT * FROM blockchain_transactions
            WHERE status = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(status)
        .fetch_all(pool)
        .await?;

        Ok(transactions)
    }

    /// Get pending transactions
    pub async fn get_pending(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, BlockchainTransaction>("SELECT * FROM blockchain_transactions WHERE status = 'PENDING'")
            .fetch_all(pool)
            .await
    }

    /// Get confirmed transactions
    pub async fn get_confirmed(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        Self::get_by_status(pool, TX_STATUS_CONFIRMED).await
    }

    /// Get failed transactions
    pub async fn get_failed(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        Self::get_by_status(pool, TX_STATUS_FAILED).await
    }

    /// Get transactions with pagination
    pub async fn get_paginated(
        pool: &PgPool,
        page: i64,
        page_size: i64,
        status_filter: Option<&str>,
        entity_type_filter: Option<&str>,
    ) -> Result<(Vec<Self>, i64), sqlx::Error> {
        let offset = (page - 1) * page_size;

        let (transactions, total_count) = if let (Some(status), Some(entity_type)) = (status_filter, entity_type_filter) {
            let total_count = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*) as count
                FROM blockchain_transactions
                WHERE status = $1 AND entity_type = $2
                "#,
            )
            .bind(status)
            .bind(entity_type)
            .fetch_one(pool)
            .await?;

            let transactions = sqlx::query_as::<_, BlockchainTransaction>(
                r#"
                SELECT *
                FROM blockchain_transactions
                WHERE status = $1 AND entity_type = $2
                ORDER BY created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(status)
            .bind(entity_type)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            (transactions, total_count)
        } else if let Some(status) = status_filter {
            let total_count = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*) as count
                FROM blockchain_transactions
                WHERE status = $1
                "#,
            )
            .bind(status)
            .fetch_one(pool)
            .await?;

            let transactions = sqlx::query_as::<_, BlockchainTransaction>(
                r#"
                SELECT *
                FROM blockchain_transactions
                WHERE status = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(status)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            (transactions, total_count)
        } else if let Some(entity_type) = entity_type_filter {
            let total_count = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*) as count
                FROM blockchain_transactions
                WHERE entity_type = $1
                "#,
            )
            .bind(entity_type)
            .fetch_one(pool)
            .await?;

            let transactions = sqlx::query_as::<_, BlockchainTransaction>(
                r#"
                SELECT *
                FROM blockchain_transactions
                WHERE entity_type = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(entity_type)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            (transactions, total_count)
        } else {
            let total_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) as count FROM blockchain_transactions"
            )
            .fetch_one(pool)
            .await?;

            let transactions = sqlx::query_as::<_, BlockchainTransaction>(
                "SELECT * FROM blockchain_transactions ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            )
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            (transactions, total_count)
        };

        Ok((transactions, total_count))
    }

    /// Check if entity has confirmed transaction
    pub async fn has_confirmed_transaction(
        pool: &PgPool,
        entity_id: i64,
        entity_type: &str,
    ) -> Result<bool, sqlx::Error> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) as count
            FROM blockchain_transactions
            WHERE entity_id = $1 AND entity_type = $2 AND status = $3
            "#,
        )
        .bind(entity_id)
        .bind(entity_type)
        .bind(TX_STATUS_CONFIRMED)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// Delete transaction (for cleanup purposes)
    pub async fn delete(pool: &PgPool, tx_hash: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM blockchain_transactions WHERE tx_hash = $1")
            .bind(tx_hash)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get a transaction by its hash
    pub async fn get_by_hash(
        pool: &PgPool,
        tx_hash: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, BlockchainTransaction>("SELECT * FROM blockchain_transactions WHERE tx_hash = $1")
            .bind(tx_hash)
            .fetch_optional(pool)
            .await
    }

    pub async fn set_confirmed(
        pool: &PgPool,
        tx_hash: &str,
        block_number: i64,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            UPDATE blockchain_transactions
            SET status = $1, block_number = $2, updated_at = NOW()
            WHERE tx_hash = $3
            RETURNING *
            "#,
        )
        .bind(TX_STATUS_CONFIRMED)
        .bind(block_number)
        .bind(tx_hash)
        .fetch_one(pool)
        .await
    }

    pub async fn set_failed(
        pool: &PgPool,
        tx_hash: &str,
        error_message: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, BlockchainTransaction>(
            r#"
            UPDATE blockchain_transactions
            SET status = $1, error = $2, updated_at = NOW()
            WHERE tx_hash = $3
            RETURNING *
            "#,
        )
        .bind(TX_STATUS_FAILED)
        .bind(error_message)
        .bind(tx_hash)
        .fetch_one(pool)
        .await
    }

    pub async fn has_pending_transactions(pool: &PgPool) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) as count
            FROM blockchain_transactions
            WHERE status = 'PENDING'
            "#,
        )
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
    }
} 