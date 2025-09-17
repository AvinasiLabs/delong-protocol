use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::blockchain_transaction::{EntityType, TransactionStatus};
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
    // New fields from migration
    pub author_id: Option<i64>,
    pub is_encrypted: bool,
    pub slug: Option<String>,
    pub license: Option<String>,
    pub thumbnail_url: Option<String>,
    pub version: Option<String>,
    pub detailed_desc: Option<String>,
    pub views_count: i64,
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
    // New fields from migration
    pub author_id: Option<i64>,
    pub is_encrypted: Option<bool>,
    pub slug: Option<String>,
    pub license: Option<String>,
    pub thumbnail_url: Option<String>,
    pub version: Option<String>,
    pub detailed_desc: Option<String>,
}

impl Dataset {
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
            EntityType::Dataset as _
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let datasets = sqlx::query_as!(
            Dataset,
            r#"
            SELECT dataset.id, dataset.name, dataset.ui_name, dataset."desc",
                   dataset.file_hash, dataset.ipfs_cid, dataset.file_size,
                   dataset.file_format, dataset.author, dataset.author_wallet,
                   dataset.sample_url, dataset.file_path,
                   dataset.author_id, dataset.is_encrypted, dataset.slug,
                   dataset.license, dataset.thumbnail_url,
                   dataset.version, dataset.detailed_desc,
                   COALESCE(dataset.views_count, 0) as "views_count!",
                   dataset.created_at, dataset.updated_at
            FROM dataset
            JOIN blockchain_transaction bt
            ON bt.entity_id = dataset.id
               AND bt.status = $1
               AND bt.entity_type = $2
            ORDER BY dataset.created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Dataset as _,
            per_page as i64,
            ((page - 1) * per_page) as i64
        )
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
                   author_id, is_encrypted, slug, license,
                   thumbnail_url, version, detailed_desc,
                   COALESCE(views_count, 0) as "views_count!",
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
                   author_id, is_encrypted, slug, license,
                   thumbnail_url, version, detailed_desc,
                   COALESCE(views_count, 0) as "views_count!",
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

    /// Find dataset by ID with confirmed transaction
    pub async fn find_by_id_confirmed(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            SELECT dataset.id, dataset.name, dataset.ui_name, dataset."desc",
                   dataset.file_hash, dataset.ipfs_cid, dataset.file_size,
                   dataset.file_format, dataset.author, dataset.author_wallet,
                   dataset.sample_url, dataset.file_path,
                   dataset.author_id, dataset.is_encrypted, dataset.slug,
                   dataset.license, dataset.thumbnail_url,
                   dataset.version, dataset.detailed_desc,
                   COALESCE(dataset.views_count, 0) as "views_count!",
                   dataset.created_at, dataset.updated_at
            FROM dataset
            JOIN blockchain_transaction bt
            ON bt.entity_id = dataset.id
               AND bt.status = $1
               AND bt.entity_type = $2
            WHERE dataset.id = $3
            "#,
            TransactionStatus::Confirmed as _,
            EntityType::Dataset as _,
            id
        )
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
                      author_id, is_encrypted, slug, license,
                      thumbnail_url, version, detailed_desc,
                      COALESCE(views_count, 0) as "views_count!",
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

    /// Convert to response format with tags
    pub async fn to_response(
        &self,
        pool: &PgPool,
    ) -> Result<crate::handlers::dataset::DatasetResponse> {
        use crate::models::DatasetTag;

        let tags = DatasetTag::get_dataset_tags(pool, self.id).await?;

        Ok(crate::handlers::dataset::DatasetResponse {
            id: self.id as u64,
            name: self.name.clone(),
            ui_name: self.ui_name.clone(),
            desc: self.desc.clone(),
            file_hash: self.file_hash.clone(),
            ipfs_cid: self.ipfs_cid.clone(),
            file_size: self.file_size as u64,
            file_format: self.file_format.clone(),
            author: self.author.clone(),
            author_wallet: self.author_wallet.clone(),
            sample_url: self.sample_url.clone(),
            file_path: self.file_path.clone(),
            author_id: self.author_id.map(|id| id as u64),
            is_encrypted: self.is_encrypted,
            slug: self.slug.clone(),
            license: self.license.clone(),
            thumbnail_url: self.thumbnail_url.clone(),
            version: self.version.clone(),
            detailed_desc: self.detailed_desc.clone(),
            views_count: self.views_count as u64,
            tags,
            author_avatar: None, // Will be populated from user data if available
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }

    /// Find dataset by file hash with transaction status
    pub async fn find_by_file_hash_with_status(
        pool: &PgPool,
        file_hash: &str,
    ) -> Result<Option<DatasetWithStatus>> {
        let result = sqlx::query!(
            r#"
            SELECT
                d.id, d.name, d.ui_name, d."desc", d.file_hash, d.ipfs_cid,
                d.file_size, d.file_format, d.author, d.author_wallet,
                d.sample_url, d.file_path, d.author_id, d.is_encrypted,
                d.slug, d.license, d.thumbnail_url,
                d.version, d.detailed_desc, COALESCE(d.views_count, 0) as "views_count!",
                d.created_at, d.updated_at,
                bt.status as "tx_status?: TransactionStatus",
                bt.tx_hash as "tx_hash?",
                bt.block_number as "block_number?",
                bt.created_at as "tx_created_at?"
            FROM dataset d
            LEFT JOIN blockchain_transaction bt
                ON bt.entity_id = d.id AND bt.entity_type = 'dataset'
            WHERE d.file_hash = $1
            ORDER BY bt.created_at DESC
            LIMIT 1
            "#,
            file_hash
        )
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|r| DatasetWithStatus {
            dataset: Dataset {
                id: r.id,
                name: r.name,
                ui_name: r.ui_name,
                desc: r.desc,
                file_hash: r.file_hash,
                ipfs_cid: r.ipfs_cid,
                file_size: r.file_size,
                file_format: r.file_format,
                author: r.author,
                author_wallet: r.author_wallet,
                sample_url: r.sample_url,
                file_path: r.file_path,
                author_id: r.author_id,
                is_encrypted: r.is_encrypted,
                slug: r.slug,
                license: r.license,
                thumbnail_url: r.thumbnail_url,
                version: r.version,
                detailed_desc: r.detailed_desc,
                views_count: r.views_count,
                created_at: r.created_at,
                updated_at: r.updated_at,
            },
            tx_status: r.tx_status,
            tx_hash: r.tx_hash,
            block_number: r.block_number.map(|n| n as u64),
            tx_created_at: r.tx_created_at,
        }))
    }

    /// Delete dataset by ID
    pub async fn delete_by_id(pool: &PgPool, id: i64) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            DELETE FROM dataset
            WHERE id = $1
            "#,
            id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Clean up timeout pending datasets (older than specified duration)
    pub async fn cleanup_timeout_datasets(pool: &PgPool, timeout_hours: i32) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM dataset
            WHERE id IN (
                SELECT d.id
                FROM dataset d
                LEFT JOIN blockchain_transaction bt
                    ON bt.entity_id = d.id AND bt.entity_type = 'dataset'
                WHERE (bt.status = 'pending' OR bt.status IS NULL)
                    AND d.created_at < NOW() - make_interval(hours => $1)
            )
            "#,
            timeout_hours
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
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
                file_format, author, author_wallet, sample_url, file_path,
                author_id, is_encrypted, slug, license,
                thumbnail_url, version, detailed_desc
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            RETURNING id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                      file_format, author, author_wallet, sample_url, file_path,
                      author_id, is_encrypted, slug, license,
                      thumbnail_url, version, detailed_desc,
                      COALESCE(views_count, 0) as "views_count!",
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
            request.file_path.as_deref(),
            request.author_id,
            request.is_encrypted.unwrap_or(false),
            request.slug.as_deref(),
            request.license.as_deref().or(Some("Custom")),
            request.thumbnail_url.as_deref(),
            request.version.as_deref().or(Some("v1.0")),
            request.detailed_desc.as_deref()
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
                   author_id, is_encrypted, slug, license,
                   thumbnail_url, version, detailed_desc,
                   COALESCE(views_count, 0) as "views_count!",
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

impl Dataset {
    /// Find dataset by slug
    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Self>> {
        let dataset = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   author_id, is_encrypted, slug, license,
                   thumbnail_url, version, detailed_desc,
                   COALESCE(views_count, 0) as "views_count!",
                   created_at, updated_at
            FROM dataset
            WHERE slug = $1
            "#,
            slug
        )
        .fetch_optional(pool)
        .await?;

        Ok(dataset)
    }

    /// Get featured datasets (based on usage in last 30 days)
    pub async fn get_featured(pool: &PgPool, limit: i32) -> Result<Vec<Self>> {
        let datasets = sqlx::query_as!(
            Dataset,
            r#"
            WITH usage_stats AS (
                SELECT
                    d.id,
                    COUNT(DISTINCT du.id) as usage_count
                FROM dataset d
                LEFT JOIN data_usage du ON du.dataset = d.name
                WHERE du.execution_status = 'completed'
                    AND du.used_at > NOW() - INTERVAL '30 days'
                GROUP BY d.id
            )
            SELECT d.id, d.name, d.ui_name, d."desc", d.file_hash, d.ipfs_cid,
                   d.file_size, d.file_format, d.author, d.author_wallet,
                   d.sample_url, d.file_path, d.author_id, d.is_encrypted,
                   d.slug, d.license, d.thumbnail_url,
                   d.version, d.detailed_desc, COALESCE(d.views_count, 0) as "views_count!",
                   d.created_at, d.updated_at
            FROM dataset d
            LEFT JOIN usage_stats us ON us.id = d.id
            ORDER BY COALESCE(us.usage_count, 0) DESC
            LIMIT $1
            "#,
            limit as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(datasets)
    }

    /// Get trending datasets (based on views in last 7 days)
    pub async fn get_trending(pool: &PgPool, limit: i32) -> Result<Vec<Self>> {
        let datasets = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   author_id, is_encrypted, slug, license,
                   thumbnail_url, version, detailed_desc,
                   COALESCE(views_count, 0) as "views_count!",
                   created_at, updated_at
            FROM dataset
            WHERE views_count > 0
                AND created_at > NOW() - INTERVAL '7 days'
            ORDER BY views_count DESC
            LIMIT $1
            "#,
            limit as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(datasets)
    }

    /// Increment view count for a dataset
    pub async fn increment_views(pool: &PgPool, id: i64) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE dataset
            SET views_count = views_count + 1
            WHERE id = $1
            "#,
            id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Search datasets by keyword
    pub async fn search(
        pool: &PgPool,
        keyword: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<Self>, u64)> {
        let search_pattern = format!("%{}%", keyword);

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM dataset
            WHERE name ILIKE $1
                OR ui_name ILIKE $1
                OR "desc" ILIKE $1
                OR detailed_desc ILIKE $1
            "#,
            &search_pattern
        )
        .fetch_one(pool)
        .await?;

        // Get paginated results
        let datasets = sqlx::query_as!(
            Dataset,
            r#"
            SELECT id, name, ui_name, "desc", file_hash, ipfs_cid, file_size,
                   file_format, author, author_wallet, sample_url, file_path,
                   author_id, is_encrypted, slug, license,
                   thumbnail_url, version, detailed_desc,
                   COALESCE(views_count, 0) as "views_count!",
                   created_at, updated_at
            FROM dataset
            WHERE name ILIKE $1
                OR ui_name ILIKE $1
                OR "desc" ILIKE $1
                OR detailed_desc ILIKE $1
            ORDER BY
                CASE
                    WHEN name ILIKE $1 THEN 1
                    WHEN ui_name ILIKE $1 THEN 2
                    ELSE 3
                END,
                views_count DESC,
                created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            &search_pattern,
            per_page as i64,
            ((page - 1) * per_page) as i64
        )
        .fetch_all(pool)
        .await?;

        Ok((datasets, total as u64))
    }
}

/// Dataset with blockchain transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetWithStatus {
    #[serde(flatten)]
    pub dataset: Dataset,
    pub tx_status: Option<TransactionStatus>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub tx_created_at: Option<DateTime<Utc>>,
}

impl Dataset {
    /// Find datasets that have ANY of the specified tags
    pub async fn find_by_any_tags(pool: &PgPool, tags: &[String]) -> Result<Vec<Self>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let datasets = sqlx::query_as!(
            Dataset,
            r#"
            SELECT DISTINCT ON (d.id) d.id, d.name, d.ui_name, d."desc", d.file_hash, d.ipfs_cid,
                   d.file_size, d.file_format, d.author, d.author_wallet, d.sample_url,
                   d.file_path, d.author_id, d.is_encrypted, d.slug, d.license,
                   d.thumbnail_url, d.version, d.detailed_desc,
                   COALESCE(d.views_count, 0) as "views_count!",
                   d.created_at, d.updated_at
            FROM dataset d
            JOIN dataset_tags dt ON d.id = dt.dataset_id
            WHERE dt.tag = ANY($1)
            ORDER BY d.id, d.views_count DESC, d.created_at DESC
            "#,
            tags
        )
        .fetch_all(pool)
        .await?;

        Ok(datasets)
    }

    /// Find datasets that have ALL of the specified tags
    pub async fn find_by_all_tags(pool: &PgPool, tags: &[String]) -> Result<Vec<Self>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let tag_count = tags.len() as i64;
        let datasets = sqlx::query_as!(
            Dataset,
            r#"
            SELECT d.id, d.name, d.ui_name, d."desc", d.file_hash, d.ipfs_cid,
                   d.file_size, d.file_format, d.author, d.author_wallet, d.sample_url,
                   d.file_path, d.author_id, d.is_encrypted, d.slug, d.license,
                   d.thumbnail_url, d.version, d.detailed_desc,
                   COALESCE(d.views_count, 0) as "views_count!",
                   d.created_at, d.updated_at
            FROM dataset d
            WHERE d.id IN (
                SELECT dataset_id
                FROM dataset_tags
                WHERE tag = ANY($1)
                GROUP BY dataset_id
                HAVING COUNT(DISTINCT tag) = $2
            )
            ORDER BY d.views_count DESC, d.created_at DESC
            "#,
            tags,
            tag_count
        )
        .fetch_all(pool)
        .await?;

        Ok(datasets)
    }
}
