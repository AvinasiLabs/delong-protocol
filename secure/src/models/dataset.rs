use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::blockchain_transaction::EntityType;
use super::pg_types::TransactionStatus;
use super::{Create, FindById, Timestamped};
use crate::{AppError, Result};

/// Dataset entity representing datasets stored in IPFS
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Dataset {
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

impl Timestamped for Dataset {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDatasetRequest {
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

impl Dataset {
    /// SQL join clause for confirmed transactions
    const JOIN_CONFIRMED_TX: &'static str = r#"
        JOIN blockchain_transaction bt
        ON bt.entity_id = dataset.id
           AND bt.status = $1
           AND bt.entity_type = $2
    "#;

    /// Find all datasets with confirmed blockchain transactions
    pub async fn find_all_confirmed(
        pool: &PgPool,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<Self>, u64)> {
        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM dataset
            JOIN blockchain_transaction bt
            ON bt.entity_id = dataset.id
               AND bt.status = $1
               AND bt.entity_type = $2
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Dataset.as_str()
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let query = format!(
            r#"
            SELECT dataset.*
            FROM dataset
            {}
            ORDER BY dataset.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let datasets = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::Dataset.as_str())
            .bind(per_page as i64)
            .bind(((page - 1) * per_page) as i64)
            .fetch_all(pool)
            .await?;

        Ok((datasets, total as u64))
    }

    /// Find dataset by file hash
    pub async fn find_by_file_hash(pool: &PgPool, file_hash: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM dataset
            WHERE file_hash = $1
            "#,
            file_hash
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Find dataset by name
    pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM dataset
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Find dataset by IPFS CID
    pub async fn find_by_ipfs_cid(pool: &PgPool, ipfs_cid: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM dataset
            WHERE ipfs_cid = $1
            "#,
            ipfs_cid
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Find dataset by ID with confirmed transaction
    pub async fn find_by_id_confirmed(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let query = format!(
            r#"
            SELECT dataset.*
            FROM dataset
            {}
            WHERE dataset.id = $3
            "#,
            Self::JOIN_CONFIRMED_TX
        );

        let dataset = sqlx::query_as::<_, Self>(&query)
            .bind(TransactionStatus::Confirmed)
            .bind(EntityType::Dataset.as_str())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(dataset)
    }

    /// Update dataset metadata
    pub async fn update_metadata(
        pool: &PgPool,
        id: i64,
        ui_name: &str,
        name: &str,
        desc: Option<&str>,
    ) -> Result<Self> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            UPDATE dataset
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

    /// Delete dataset
    pub async fn delete(pool: &PgPool, id: i64) -> Result<()> {
        let result = sqlx::query!("DELETE FROM dataset WHERE id = $1", id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Dataset with id {} not found",
                id
            )));
        }

        Ok(())
    }

    /// Convert to response format
    pub fn to_response(&self) -> crate::handlers::dataset::DatasetResponse {
        crate::handlers::dataset::DatasetResponse {
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
impl Create for Dataset {
    type Request = CreateDatasetRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let result = sqlx::query_as!(
            Dataset,
            r#"
            INSERT INTO dataset (
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
        .await;

        match result {
            Ok(dataset) => Ok(dataset),
            Err(sqlx::Error::Database(db_err)) => {
                // Check for unique constraint violation
                if db_err.is_unique_violation() {
                    // Determine which field caused the violation
                    let message = db_err.message();
                    if message.contains("file_hash") {
                        Err(AppError::Conflict(
                            "Dataset with this file hash already exists".to_string(),
                        ))
                    } else if message.contains("ipfs_cid") {
                        Err(AppError::Conflict(
                            "Dataset with this IPFS CID already exists".to_string(),
                        ))
                    } else if message.contains("ui_name") {
                        Err(AppError::Conflict(
                            "Dataset with this UI name already exists".to_string(),
                        ))
                    } else if message.contains("name") {
                        Err(AppError::Conflict(
                            "Dataset with this name already exists".to_string(),
                        ))
                    } else {
                        Err(AppError::Conflict("Dataset already exists".to_string()))
                    }
                } else {
                    Err(AppError::from(sqlx::Error::Database(db_err)))
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[async_trait::async_trait]
impl FindById for Dataset {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   created_at, updated_at
            FROM dataset
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }
}
