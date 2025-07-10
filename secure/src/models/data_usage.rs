use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize)]
pub struct DataUsage {
    pub id: i64,
    pub scientist_wallet: String,
    pub cid: String,
    pub dataset: String,
    pub used_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUsageInfo {
    pub id: i64,
    pub scientist_wallet: String,
    pub cid: String,
    pub dataset: String,
    pub used_at: i64,
}

impl From<DataUsage> for DataUsageInfo {
    fn from(usage: DataUsage) -> Self {
        DataUsageInfo {
            id: usage.id,
            scientist_wallet: usage.scientist_wallet,
            cid: usage.cid,
            dataset: usage.dataset,
            used_at: usage.used_at.timestamp(),
        }
    }
}

impl DataUsage {
    pub async fn create(
        pool: &PgPool,
        scientist_wallet: &str,
        cid: &str,
        dataset: &str,
    ) -> Result<Self, sqlx::Error> {
        let used_at = Utc::now();
        sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO data_usages (scientist_wallet, cid, dataset, used_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(scientist_wallet)
        .bind(cid)
        .bind(dataset)
        .bind(used_at)
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_scientist(
        pool: &PgPool,
        scientist_wallet: &str,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM data_usages WHERE scientist_wallet = $1")
            .bind(scientist_wallet)
            .fetch_all(pool)
            .await
    }

    pub async fn get_by_dataset(pool: &PgPool, dataset: &str) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>("SELECT * FROM data_usages WHERE dataset = $1")
            .bind(dataset)
            .fetch_all(pool)
            .await
    }

    pub async fn get_paginated(
        pool: &PgPool,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Self>, i64), sqlx::Error> {
        let offset = (page - 1) * page_size;
        let records =
            sqlx::query_as::<_, Self>("SELECT * FROM data_usages ORDER BY used_at DESC LIMIT $1 OFFSET $2")
                .bind(page_size)
                .bind(offset)
                .fetch_all(pool)
                .await?;
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM data_usages")
            .fetch_one(pool)
            .await?;
        Ok((records, total.0))
    }
} 