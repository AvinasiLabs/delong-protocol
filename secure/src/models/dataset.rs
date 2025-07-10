use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use common::{ApiResult, ApiError};

// Constants for entity types and transaction status
pub const ENTITY_TYPE_STATIC_DATASET: &str = "STATIC_DATASET";
pub const TX_STATUS_CONFIRMED: &str = "CONFIRMED";

// SQL join clause for confirmed transactions
pub const STC_DATASET_JOIN_CONFIRMED_TX: &str = r#"
JOIN blockchain_transactions bt
ON bt.entity_id = static_datasets.id
   AND bt.status = $1
   AND bt.entity_type = $2
"#;

/// Static dataset model representing immutable encrypted datasets in TEE
/// Matches the Go struct from /root/delong/internal/models/stc_dataset.go
#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize)]
pub struct StaticDataset {
    pub id: i64,
    pub user_wallet: String,
    pub name: String,
    pub ui_name: String,
    pub desc: String,
    pub file_hash: String,     // SHA-256 hash of original file for deduplication
    pub ipfs_cid: String,
    pub file_size: i64,
    pub file_format: String,   // csv, json, parquet...
    pub author: String,
    pub author_wallet: String,
    pub sample_url: String,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating a new static dataset
/// Matches CreateStcDatasetReq from the Go project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStcDatasetReq {
    pub name: String,
    pub ui_name: String,
    pub desc: String,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub file_size: i64,
    pub file_format: String,
    pub author: String,
    pub author_wallet: String,
    pub sample_url: String,
    pub file_path: String,
}

/// Request model for updating a static dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStaticDatasetRequest {
    pub name: String,
    pub ui_name: String,
    pub desc: String,
}


/// Response model for static dataset info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticDatasetInfo {
    pub id: u64,
    pub name: String,
    pub ui_name: String,
    pub desc: String,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub file_size: i64,
    pub file_format: String,
    pub author: String,
    pub author_wallet: String,
    pub sample_url: String,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<StaticDataset> for StaticDatasetInfo {
    fn from(dataset: StaticDataset) -> Self {
        StaticDatasetInfo {
            id: dataset.id as u64,
            name: dataset.name,
            ui_name: dataset.ui_name,
            desc: dataset.desc,
            file_hash: dataset.file_hash,
            ipfs_cid: dataset.ipfs_cid,
            file_size: dataset.file_size,
            file_format: dataset.file_format,
            author: dataset.author,
            author_wallet: dataset.author_wallet,
            sample_url: dataset.sample_url,
            file_path: dataset.file_path,
            created_at: dataset.created_at,
            updated_at: dataset.updated_at,
        }
    }
}

impl StaticDataset {
    /// Create a new static dataset within a database transaction.
    /// This is the new "fat model" approach, where the handler orchestrates the transaction.
    pub async fn create_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_wallet: &str,
        req: &CreateStcDatasetReq,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO static_datasets (user_wallet, name, ui_name, desc, file_hash, ipfs_cid, file_size, 
                file_format, author, author_wallet, sample_url, file_path)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *
            "#,
        )
        .bind(user_wallet)
        .bind(&req.name)
        .bind(&req.ui_name)
        .bind(&req.desc)
        .bind(&req.file_hash)
        .bind(&req.ipfs_cid)
        .bind(req.file_size)
        .bind(&req.file_format)
        .bind(&req.author)
        .bind(&req.author_wallet)
        .bind(&req.sample_url)
        .bind(&req.file_path)
        .fetch_one(&mut **tx)
        .await
    }

    /// Get paginated static datasets with confirmed transactions - matches GetStcDataset function
    pub async fn get_paginated(
        pool: &PgPool,
        page: i64,
        page_size: i64,
    ) -> ApiResult<(Vec<Self>, i64)> {
        let offset = (page - 1) * page_size;

        // Get total count with confirmed transactions
        let total_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM static_datasets
            JOIN blockchain_transactions bt ON bt.entity_id = static_datasets.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            "#,
            TX_STATUS_CONFIRMED,
            ENTITY_TYPE_STATIC_DATASET
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        // Get paginated results
        let datasets = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT static_datasets.id, static_datasets.user_wallet, static_datasets.name, static_datasets.ui_name, 
                   static_datasets.desc, static_datasets.file_hash, static_datasets.ipfs_cid,
                   static_datasets.file_size, static_datasets.file_format, static_datasets.author,
                   static_datasets.author_wallet, static_datasets.sample_url, static_datasets.file_path,
                   static_datasets.created_at, static_datasets.updated_at
            FROM static_datasets
            JOIN blockchain_transactions bt ON bt.entity_id = static_datasets.id
            WHERE bt.status = $1 AND bt.entity_type = $2
            ORDER BY static_datasets.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            TX_STATUS_CONFIRMED,
            ENTITY_TYPE_STATIC_DATASET,
            page_size,
            offset
        )
        .fetch_all(pool)
        .await?;

        Ok((datasets, total_count))
    }

    /// Get static dataset by ID with confirmed transaction - matches GetStcDatasetByID function
    pub async fn get_by_id(pool: &PgPool, id: i64) -> ApiResult<Self> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT static_datasets.id, static_datasets.user_wallet, static_datasets.name, static_datasets.ui_name, 
                   static_datasets.desc, static_datasets.file_hash, static_datasets.ipfs_cid,
                   static_datasets.file_size, static_datasets.file_format, static_datasets.author,
                   static_datasets.author_wallet, static_datasets.sample_url, static_datasets.file_path,
                   static_datasets.created_at, static_datasets.updated_at
            FROM static_datasets
            JOIN blockchain_transactions bt ON bt.entity_id = static_datasets.id
            WHERE static_datasets.id = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
            id,
            TX_STATUS_CONFIRMED,
            ENTITY_TYPE_STATIC_DATASET
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }

    /// Get static dataset by hash with confirmed transaction - matches GetStcDatasetByHash function
    pub async fn get_by_hash(pool: &PgPool, hash: &str) -> ApiResult<Self> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT static_datasets.id, static_datasets.user_wallet, static_datasets.name, static_datasets.ui_name, 
                   static_datasets.desc, static_datasets.file_hash, static_datasets.ipfs_cid,
                   static_datasets.file_size, static_datasets.file_format, static_datasets.author,
                   static_datasets.author_wallet, static_datasets.sample_url, static_datasets.file_path,
                   static_datasets.created_at, static_datasets.updated_at
            FROM static_datasets
            JOIN blockchain_transactions bt ON bt.entity_id = static_datasets.id
            WHERE static_datasets.file_hash = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
            hash,
            TX_STATUS_CONFIRMED,
            ENTITY_TYPE_STATIC_DATASET
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }

    /// Get static dataset by name with confirmed transaction - matches GetStcDatasetByName function
    pub async fn get_by_name(pool: &PgPool, name: &str) -> ApiResult<Self> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT static_datasets.id, static_datasets.user_wallet, static_datasets.name, static_datasets.ui_name, 
                   static_datasets.desc, static_datasets.file_hash, static_datasets.ipfs_cid,
                   static_datasets.file_size, static_datasets.file_format, static_datasets.author,
                   static_datasets.author_wallet, static_datasets.sample_url, static_datasets.file_path,
                   static_datasets.created_at, static_datasets.updated_at
            FROM static_datasets
            JOIN blockchain_transactions bt ON bt.entity_id = static_datasets.id
            WHERE static_datasets.name = $1 AND bt.status = $2 AND bt.entity_type = $3
            "#,
            name,
            TX_STATUS_CONFIRMED,
            ENTITY_TYPE_STATIC_DATASET
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }

    /// Update a static dataset's mutable fields
    pub async fn update(
        pool: &PgPool,
        id: i64,
        req: UpdateStaticDatasetRequest,
    ) -> ApiResult<Self> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            UPDATE static_datasets
            SET ui_name = $1, "desc" = $2, updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
            req.ui_name,
            req.desc,
            id
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }
}