use avinapi::{query::PaginationQuery, transport::response::PaginatedData};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{Create, FindById, Timestamped};
use crate::Result;

/// Data usage tracking entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DataUsage {
    pub id: i64,
    pub scientist_wallet: String,
    pub cid: String,
    pub dataset: String,
    pub used_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New data usage record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDataUsage {
    pub scientist_wallet: String,
    pub cid: String,
    pub dataset: String,
    pub used_at: DateTime<Utc>,
}

impl Timestamped for DataUsage {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new data usage record
#[derive(Debug, Clone, Deserialize)]
pub struct CreateDataUsageRequest {
    pub scientist_wallet: String,
    pub cid: String,
    pub dataset: String,
    pub used_at: Option<DateTime<Utc>>,
}

impl DataUsage {
    /// Find data usage by scientist wallet
    pub async fn find_by_wallet(
        pool: &PgPool,
        scientist_wallet: &str,
        pagination: &PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM data_usage WHERE scientist_wallet = $1",
            scientist_wallet
        )
        .fetch_one(pool)
        .await?;

        let usages = sqlx::query_as!(
            DataUsage,
            r#"
            SELECT * FROM data_usage
            WHERE scientist_wallet = $1
            ORDER BY used_at DESC
            LIMIT $2 OFFSET $3
            "#,
            scientist_wallet,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: usages,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Find data usage by CID (algorithm identifier)
    pub async fn find_by_cid(
        pool: &PgPool,
        cid: &str,
        pagination: &PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM data_usage WHERE cid = $1",
            cid
        )
        .fetch_one(pool)
        .await?;

        let usages = sqlx::query_as!(
            DataUsage,
            r#"
            SELECT * FROM data_usage
            WHERE cid = $1
            ORDER BY used_at DESC
            LIMIT $2 OFFSET $3
            "#,
            cid,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: usages,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Find data usage by dataset
    pub async fn find_by_dataset(
        pool: &PgPool,
        dataset: &str,
        pagination: &PaginationQuery,
    ) -> Result<PaginatedData<Self>> {
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) as \"count!\" FROM data_usage WHERE dataset = $1",
            dataset
        )
        .fetch_one(pool)
        .await?;

        let usages = sqlx::query_as!(
            DataUsage,
            r#"
            SELECT * FROM data_usage
            WHERE dataset = $1
            ORDER BY used_at DESC
            LIMIT $2 OFFSET $3
            "#,
            dataset,
            pagination.get_limit() as i64,
            pagination.get_offset() as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(PaginatedData {
            items: usages,
            n_page: pagination.get_page(),
            per_page: pagination.get_per_page(),
            total: total as u64,
        })
    }

    /// Get usage statistics for a dataset
    pub async fn get_usage_stats(pool: &PgPool, dataset: &str) -> Result<UsageStats> {
        let stats = sqlx::query_as!(
            UsageStats,
            r#"
            SELECT
                COUNT(DISTINCT cid) as "unique_algorithms!",
                COUNT(*) as "total_uses!",
                MIN(used_at) as first_used,
                MAX(used_at) as last_used,
                COUNT(DISTINCT scientist_wallet) as "unique_wallets!"
            FROM data_usage
            WHERE dataset = $1
            "#,
            dataset
        )
        .fetch_one(pool)
        .await?;

        Ok(stats)
    }

    /// Convert to API response
    pub fn to_response(&self) -> DataUsageResponse {
        DataUsageResponse {
            id: self.id as u64,
            scientist_wallet: self.scientist_wallet.clone(),
            cid: self.cid.clone(),
            dataset: self.dataset.clone(),
            used_at: self.used_at,
        }
    }

    /// Create a new data usage record with transaction support
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        scientist_wallet: String,
        cid: String,
        dataset: String,
        used_at: DateTime<Utc>,
    ) -> Result<Self> {
        let usage = sqlx::query_as!(
            DataUsage,
            r#"
            INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            &scientist_wallet,
            &cid,
            &dataset,
            used_at
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(usage)
    }
}

#[async_trait::async_trait]
impl Create for DataUsage {
    type Request = CreateDataUsageRequest;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let used_at = request.used_at.unwrap_or_else(Utc::now);

        let usage = sqlx::query_as!(
            DataUsage,
            r#"
            INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
            request.scientist_wallet,
            request.cid,
            request.dataset,
            used_at
        )
        .fetch_one(pool)
        .await?;

        Ok(usage)
    }
}

#[async_trait::async_trait]
impl FindById for DataUsage {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let usage = sqlx::query_as!(DataUsage, "SELECT * FROM data_usage WHERE id = $1", id)
            .fetch_optional(pool)
            .await?;

        Ok(usage)
    }
}

/// API response for data usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUsageResponse {
    pub id: u64,
    pub scientist_wallet: String,
    pub cid: String,
    pub dataset: String,
    pub used_at: DateTime<Utc>,
}

/// Usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageStats {
    pub unique_algorithms: i64,
    pub total_uses: i64,
    pub first_used: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
    pub unique_wallets: i64,
}
