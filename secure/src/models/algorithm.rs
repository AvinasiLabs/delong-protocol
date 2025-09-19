use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::{Create, FindById, Timestamped};
use crate::Result;

/// Algorithm entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Algo {
    pub id: i64,
    pub name: String,
    pub algo_link: String,
    pub cid: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Timestamped for Algo {
    fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

/// Request to create a new algorithm
#[derive(Debug, Clone, Deserialize)]
pub struct CreateAlgo {
    pub name: String,
    pub algo_link: String,
    pub cid: String,
}

impl Algo {
    /// Find algorithm by link
    pub async fn find_by_link(pool: &PgPool, algo_link: &str) -> Result<Option<Self>> {
        let algo = sqlx::query_as!(
            Self,
            "SELECT * FROM algorithm WHERE algo_link = $1",
            algo_link
        )
        .fetch_optional(pool)
        .await?;

        Ok(algo)
    }

    /// Find algorithm by CID
    pub async fn find_by_cid(pool: &PgPool, cid: &str) -> Result<Option<Self>> {
        let algo = sqlx::query_as!(
            Self,
            "SELECT * FROM algorithm WHERE cid = $1",
            cid
        )
        .fetch_optional(pool)
        .await?;

        Ok(algo)
    }
}

#[async_trait::async_trait]
impl Create for Algo {
    type Request = CreateAlgo;

    async fn create(pool: &PgPool, request: Self::Request) -> Result<Self> {
        let algo = sqlx::query_as!(
            Algo,
            r#"
            INSERT INTO algorithm (name, algo_link, cid)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
            &request.name,
            &request.algo_link,
            &request.cid
        )
        .fetch_one(pool)
        .await?;

        Ok(algo)
    }
}

#[async_trait::async_trait]
impl FindById for Algo {
    async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let algo = sqlx::query_as!(Algo, "SELECT * FROM algorithm WHERE id = $1", id)
            .fetch_optional(pool)
            .await?;

        Ok(algo)
    }
}
