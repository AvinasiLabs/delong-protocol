use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Type};
use std::fmt;

use super::{Create, FindById, Timestamped};
use crate::{AppError, Result};

/// Entity type that the transaction is associated with
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "entity_type", rename_all = "lowercase")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    Dataset,
    Execution,
    DataUsage,
    Vote,
    Committee,
}

// Note: The as_str() method is no longer needed!
// With rename_all = "lowercase", sqlx automatically handles conversion:
// - EntityType::Dataset <-> "dataset" in database
// - EntityType::Execution <-> "execution" in database
// - etc.
//
// You can now use the enum directly in queries:
// sqlx::query!("SELECT * FROM table WHERE entity_type = $1", entity_type as EntityType)

impl EntityType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "DATASET" => Some(Self::Dataset),
            "EXECUTION" => Some(Self::Execution),
            "DATAUSAGE" => Some(Self::DataUsage),
            "VOTE" => Some(Self::Vote),
            "COMMITTEE" => Some(Self::Committee),
            _ => None,
        }
    }
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_status", rename_all = "lowercase")]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

impl fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionStatus::Pending => write!(f, "pending"),
            TransactionStatus::Confirmed => write!(f, "confirmed"),
            TransactionStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Blockchain transaction tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BlockchainTransaction {
    pub id: i64,
    pub tx_hash: String,
    pub entity_id: i64,
    pub entity_type: EntityType,
    pub status: TransactionStatus,
    pub block_number: Option<i64>,
    pub block_timestamp: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BlockchainTransaction {
    /// Find transaction by hash
    /// Update transaction status
    pub async fn update_status(
        pool: &PgPool,
        tx_hash: &str,
        status: TransactionStatus,
        block_number: Option<u64>,
        block_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Self> {
        sqlx::query!(
            r#"
            UPDATE transaction
            SET status = $1, block_number = $2, block_timestamp = $3, updated_at = NOW()
            WHERE tx_hash = $4
            "#,
            status as _,
            block_number.map(|n| n as i64),
            block_timestamp,
            tx_hash
        )
        .execute(pool)
        .await?;

        Self::find_by_tx_hash(pool, tx_hash)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))
    }

    /// Create a new transaction with a specific status
    pub async fn create_with_status(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tx_hash: &str,
        entity_id: i64,
        entity_type: EntityType,
        status: TransactionStatus,
        block_number: Option<u64>,
        block_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Self> {
        let transaction = sqlx::query_as!(
            BlockchainTransaction,
            r#"
            INSERT INTO transaction (tx_hash, entity_id, entity_type, status, block_number, block_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, tx_hash, entity_id,
                     entity_type as "entity_type: _",
                     status as "status: _",
                     block_number, block_timestamp,
                     created_at, updated_at
            "#,
            tx_hash,
            entity_id,
            entity_type as _,
            status as _,
            block_number.map(|n| n as i64),
            block_timestamp
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.is_unique_violation() {
                    return AppError::Conflict(
                        "Transaction with this hash already exists".to_string(),
                    );
                }
            }
            e.into()
        })?;

        Ok(transaction)
    }

    /// Create a new transaction with PENDING status
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        tx_hash: String,
        entity_id: i64,
        entity_type: EntityType,
        status: TransactionStatus,
    ) -> Result<Self> {
        Self::create_with_status(tx, &tx_hash, entity_id, entity_type, status, None, None).await
    }

    /// Update transaction status by entity
    pub async fn update_status_by_entity(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        entity_id: i64,
        entity_type: EntityType,
        status: TransactionStatus,
        block_number: Option<u64>,
        block_time: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        let result = sqlx::query!(
            r#"
            UPDATE transaction
            SET status = $1, block_number = $2, block_timestamp = $3, updated_at = NOW()
            WHERE entity_id = $4 AND entity_type = $5
            "#,
            status as _,
            block_number.map(|n| n as i64),
            block_time,
            entity_id,
            entity_type as _
        )
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "BlockchainTransaction for entity_id {} and entity_type {:?} not found",
                entity_id, entity_type
            )));
        }

        Ok(())
    }

    /// Find transaction by hash
    pub async fn find_by_tx_hash(pool: &PgPool, tx_hash: &str) -> Result<Option<Self>> {
        let tx = sqlx::query_as!(
            Self,
            r#"
            SELECT id, tx_hash, entity_id,
                   entity_type as "entity_type: _",
                   status as "status: _",
                   block_number, block_timestamp,
                   created_at, updated_at
            FROM transaction
            WHERE tx_hash = $1
            "#,
            tx_hash
        )
        .fetch_optional(pool)
        .await?;

        Ok(tx)
    }

    /// UPSERT a blockchain transaction record for a dataset
    /// This will not overwrite if a confirmed record already exists
    pub async fn upsert_for_dataset(pool: &PgPool, tx_hash: &str, dataset_id: i64) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            INSERT INTO transaction (
                tx_hash, entity_id, entity_type, status, created_at, updated_at
            ) VALUES (
                $1, $2, 'dataset', 'pending', NOW(), NOW()
            )
            ON CONFLICT (tx_hash) DO UPDATE SET
                entity_id = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.entity_id
                    ELSE EXCLUDED.entity_id
                END,
                entity_type = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.entity_type
                    ELSE EXCLUDED.entity_type
                END,
                updated_at = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.updated_at
                    ELSE NOW()
                END
            "#,
            tx_hash,
            dataset_id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// UPSERT a blockchain transaction record for an execution
    /// This will not overwrite if a confirmed record already exists
    pub async fn upsert_for_execution(
        pool: &PgPool,
        tx_hash: &str,
        execution_id: i64,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            INSERT INTO transaction (
                tx_hash, entity_id, entity_type, status, created_at, updated_at
            ) VALUES (
                $1, $2, 'execution', 'pending', NOW(), NOW()
            )
            ON CONFLICT (tx_hash) DO UPDATE SET
                entity_id = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.entity_id
                    ELSE EXCLUDED.entity_id
                END,
                entity_type = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.entity_type
                    ELSE EXCLUDED.entity_type
                END,
                updated_at = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.updated_at
                    ELSE NOW()
                END
            "#,
            tx_hash,
            execution_id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// UPSERT a blockchain transaction from a chain event
    /// This is used by chainsync when processing DataRegistered events
    pub async fn upsert_from_event(
        pool: &PgPool,
        tx_hash: &str,
        entity_id: i64,
        entity_type: EntityType,
        status: TransactionStatus,
        block_number: i64,
        block_timestamp: Option<DateTime<Utc>>,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            INSERT INTO transaction (
                tx_hash, entity_id, entity_type, status, block_number, block_timestamp, created_at, updated_at
            ) VALUES (
                $1, $2, $3::entity_type, $4, $5, $6, NOW(), NOW()
            )
            ON CONFLICT (tx_hash) DO UPDATE SET
                status = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.status
                    ELSE EXCLUDED.status
                END,
                block_number = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.block_number
                    ELSE EXCLUDED.block_number
                END,
                block_timestamp = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.block_timestamp
                    ELSE EXCLUDED.block_timestamp
                END,
                updated_at = CASE
                    WHEN transaction.status = 'confirmed' THEN transaction.updated_at
                    ELSE NOW()
                END
            "#,
            tx_hash,
            entity_id,
            entity_type as _,
            status as TransactionStatus,
            block_number,
            block_timestamp
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Find pending transactions by entity
    pub async fn find_pending_by_entity(
        pool: &PgPool,
        entity_id: i64,
        entity_type: EntityType,
    ) -> Result<Vec<Self>> {
        let txs = sqlx::query_as!(
            Self,
            r#"
            SELECT id, tx_hash, entity_id,
                   entity_type as "entity_type: _",
                   status as "status: _",
                   block_number, block_timestamp,
                   created_at, updated_at
            FROM transaction
            WHERE entity_id = $1 AND entity_type = $2 AND status = $3
            ORDER BY created_at DESC
            "#,
            entity_id,
            entity_type as _,
            TransactionStatus::Pending as _
        )
        .fetch_all(pool)
        .await?;

        Ok(txs)
    }

    /// Check if entity has confirmed transaction
    pub async fn has_confirmed_transaction(
        pool: &PgPool,
        entity_id: i64,
        entity_type: EntityType,
    ) -> Result<bool> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM transaction
            WHERE entity_id = $1 AND entity_type = $2 AND status = $3
            "#,
            entity_id,
            entity_type as _,
            TransactionStatus::Confirmed as _
        )
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }
}

impl Timestamped for BlockchainTransaction {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlockchainTransactionRequest {
    pub tx_hash: String,
    pub entity_id: i64,
    pub entity_type: EntityType,
}

/// Request to create a new blockchain transaction (used in handlers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransaction {
    pub tx_hash: String,
    pub entity_id: i64,
    pub entity_type: EntityType,
}

impl CreateTransaction {
    /// Create a new blockchain transaction within a database transaction
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        request: Self,
    ) -> Result<BlockchainTransaction> {
        let transaction = sqlx::query_as!(
            BlockchainTransaction,
            r#"
            INSERT INTO transaction (tx_hash, entity_id, entity_type, status)
            VALUES ($1, $2, $3, $4)
            RETURNING id, tx_hash, entity_id,
                     entity_type as "entity_type: _",
                     status as "status: _",
                     block_number, block_timestamp,
                     created_at, updated_at
            "#,
            &request.tx_hash,
            request.entity_id,
            request.entity_type as _,
            TransactionStatus::Pending as _
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.is_unique_violation() {
                    return AppError::Conflict(
                        "Transaction with this hash already exists".to_string(),
                    );
                }
            }
            e.into()
        })?;

        Ok(transaction)
    }
}

#[async_trait::async_trait]
impl Create for BlockchainTransaction {
    type Request = CreateBlockchainTransactionRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let tx = sqlx::query_as!(
            BlockchainTransaction,
            r#"
            INSERT INTO transaction (tx_hash, entity_id, entity_type, status)
            VALUES ($1, $2, $3, $4)
            RETURNING id, tx_hash, entity_id,
                     entity_type as "entity_type: _",
                     status as "status: _",
                     block_number, block_timestamp,
                     created_at, updated_at
            "#,
            &request.tx_hash,
            request.entity_id,
            request.entity_type as _,
            TransactionStatus::Pending as _
        )
        .fetch_one(pool)
        .await?;

        Ok(tx)
    }
}

impl BlockchainTransaction {
    /// Find all pending transactions
    pub async fn find_all_pending(pool: &PgPool) -> Result<Vec<Self>> {
        let txs = sqlx::query_as!(
            Self,
            r#"
            SELECT id, tx_hash, entity_id,
                   entity_type as "entity_type: _",
                   status as "status: _",
                   block_number, block_timestamp,
                   created_at, updated_at
            FROM transaction
            WHERE status = $1
            ORDER BY created_at ASC
            "#,
            TransactionStatus::Pending as _
        )
        .fetch_all(pool)
        .await?;

        Ok(txs)
    }

    /// Create a failed transaction record (for immediate failures)
    pub async fn create_failed(
        pool: &PgPool,
        entity_id: i64,
        entity_type: EntityType,
        error_message: &str,
    ) -> Result<Self> {
        // Generate a pseudo tx hash for failed transactions
        let tx_hash = format!("failed_{}_{}", entity_id, Utc::now().timestamp());

        let tx = sqlx::query_as!(
            Self,
            r#"
            INSERT INTO transaction (
                tx_hash, entity_id, entity_type, status, created_at, updated_at
            )
            VALUES ($1, $2, $3::entity_type, $4, NOW(), NOW())
            RETURNING id, tx_hash, entity_id,
                     entity_type as "entity_type: _",
                     status as "status: _",
                     block_number, block_timestamp,
                     created_at, updated_at
            "#,
            tx_hash,
            entity_id,
            entity_type as _,
            TransactionStatus::Failed as _
        )
        .fetch_one(pool)
        .await?;

        // Log the error message (could be stored in a separate error log table)
        tracing::error!(
            "Transaction failed for {} {}: {}",
            format!("{:?}", entity_type),
            entity_id,
            error_message
        );

        Ok(tx)
    }
}

impl Default for BlockchainTransaction {
    fn default() -> Self {
        Self {
            id: 0,
            tx_hash: String::new(),
            entity_id: 0,
            entity_type: EntityType::Execution,
            status: TransactionStatus::Pending,
            block_number: None,
            block_timestamp: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[async_trait::async_trait]
impl FindById for BlockchainTransaction {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let tx = sqlx::query_as!(
            Self,
            r#"
            SELECT id, tx_hash, entity_id,
                   entity_type as "entity_type: _",
                   status as "status: _",
                   block_number, block_timestamp,
                   created_at, updated_at
            FROM transaction
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(tx)
    }
}
