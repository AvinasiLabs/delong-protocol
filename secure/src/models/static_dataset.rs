use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::blockchain_transaction::EntityType;
use super::pg_types::TransactionStatus;
use super::{Create, FindById, PaginatedResponse, PaginationParams, Timestamped};
use crate::error::{DbErrorExt, Result};

/// Static dataset entity representing datasets stored in IPFS
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StaticDataset {
    pub id: i64,
    pub name: String,
    pub ui_name: String,
    #[serde(rename = "desc")]
    #[sqlx(rename = "desc")]
    pub desc: Option<String>,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub file_size: i64,
    pub file_format: String,
    pub author: Option<String>,
    pub author_wallet: String,
    pub sample_url: Option<String>,
    pub file_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Timestamped for StaticDataset {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new static dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStaticDatasetRequest {
    pub name: String,
    pub ui_name: String,
    pub desc: Option<String>,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub file_size: i64,
    pub file_format: String,
    pub author: Option<String>,
    pub author_wallet: String,
    pub sample_url: Option<String>,
    pub file_path: Option<String>,
}

impl StaticDataset {
    /// SQL join clause for confirmed transactions
    const JOIN_CONFIRMED_TX: &'static str = r#"
        JOIN blockchain_transactions bt
        ON bt.entity_id = static_datasets.id
           AND bt.status = $1
           AND bt.entity_type = $2
    "#;

    /// Find all static datasets with confirmed blockchain transactions
    pub async fn find_all_confirmed(
        pool: &PgPool,
        pagination: PaginationParams,
    ) -> Result<PaginatedResponse<Self>> {
        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM static_datasets
            JOIN blockchain_transactions bt
            ON bt.entity_id = static_datasets.id
               AND bt.status = $1
               AND bt.entity_type = $2
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::StaticDataset.as_str()
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let query = format!(
            r#"
            SELECT static_datasets.*
            FROM static_datasets
            {}
            ORDER BY static_datasets.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let datasets = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::StaticDataset.as_str())
            .bind(pagination.get_limit() as i64)
            .bind(pagination.get_offset() as i64)
            .fetch_all(pool)
            .await?;

        Ok(PaginatedResponse::new(datasets, &pagination, total as u64))
    }

    /// Find static dataset by file hash
    pub async fn find_by_file_hash(pool: &PgPool, file_hash: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM static_datasets
            WHERE file_hash = $1
            "#,
            file_hash
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Find static dataset by name
    pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM static_datasets
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Find static dataset by IPFS CID
    pub async fn find_by_ipfs_cid(pool: &PgPool, ipfs_cid: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM static_datasets
            WHERE ipfs_cid = $1
            "#,
            ipfs_cid
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Find static dataset by ID with confirmed transaction
    pub async fn find_by_id_confirmed(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let query = format!(
            r#"
            SELECT static_datasets.*
            FROM static_datasets
            {}
            WHERE static_datasets.id = $3
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let dataset = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::StaticDataset.as_str())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(dataset)
    }

    /// Update static dataset metadata
    pub async fn update_metadata(
        pool: &PgPool,
        id: i64,
        ui_name: &str,
        name: &str,
        desc: Option<&str>,
    ) -> Result<Self> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            UPDATE static_datasets
            SET ui_name = $1, name = $2, "desc" = $3, updated_at = NOW()
            WHERE id = $4
            RETURNING id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                      file_format, author, author_wallet, sample_url, file_path,
                      created_at, updated_at
            "#,
            ui_name,
            name,
            desc,
            id
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }

    /// Delete static dataset
    pub async fn delete(pool: &PgPool, id: i64) -> Result<()> {
        let result = sqlx::query!("DELETE FROM static_datasets WHERE id = $1", id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AppError::not_found(format!(
                "Static dataset with id {} not found",
                id
            )));
        }

        Ok(())
    }

    /// Convert to response format
    pub fn to_response(&self) -> crate::handlers::static_dataset::StaticDatasetResponse {
        crate::handlers::static_dataset::StaticDatasetResponse {
            id: self.id as u64,
            name: self.name.clone(),
            file_hash: self.file_hash.clone(),
            ipfs_cid: self.ipfs_cid.clone(),
            author_wallet: self.author_wallet.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[async_trait::async_trait]
impl Create for StaticDataset {
    type Request = CreateStaticDatasetRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            INSERT INTO static_datasets (
                name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                file_format, author, author_wallet, sample_url, file_path
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                      file_format, author, author_wallet, sample_url, file_path,
                      created_at, updated_at
            "#,
            &request.name,
            &request.ui_name,
            request.desc.as_deref(),
            &request.file_hash,
            &request.ipfs_cid,
            request.file_size,
            &request.file_format,
            request.author.as_deref(),
            &request.author_wallet,
            request.sample_url.as_deref(),
            request.file_path.as_deref()
        )
        .fetch_one(pool)
        .await
        .conflict_msg("Dataset with this name, ui_name or file_hash already exists")?;

        Ok(dataset)
    }
}

#[async_trait::async_trait]
impl FindById for StaticDataset {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM static_datasets
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_create_static_dataset() {
        // Test will be implemented when we have test database setup
    }
}
