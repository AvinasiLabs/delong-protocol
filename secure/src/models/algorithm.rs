use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, FromRow, PgPool};

/// Represents an algorithm in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Algorithm {
    pub id: i64,
    pub name: String,
    pub algo_link: String,
    pub cid: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating a new algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgorithmRequest {
    pub name: String,
    pub algo_link: String,
    pub cid: String,
}

impl Algorithm {
    /// Create a new algorithm. Must be used within a transaction.
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        req: CreateAlgorithmRequest,
    ) -> Result<Algorithm, sqlx::Error> {
        sqlx::query_as(
            r#"
            INSERT INTO algorithms (name, algo_link, cid)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(req.name)
        .bind(req.algo_link)
        .bind(req.cid)
        .fetch_one(&mut **tx)
        .await
    }

    /// Get an algorithm by its ID.
    pub async fn get_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM algorithms WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Get an algorithm by its unique link.
    pub async fn get_by_link(pool: &PgPool, algo_link: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM algorithms WHERE algo_link = $1")
            .bind(algo_link)
            .fetch_optional(pool)
            .await
    }
} 