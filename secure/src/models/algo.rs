use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

/// Algorithm model for storing algorithm metadata
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Algo {
    pub id: i32,
    pub name: String,
    pub algo_link: String,
    pub cid: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating an algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlgoRequest {
    pub name: String,
    pub algo_link: String,
    pub cid: String,
}

impl Algo {
    /// Create a new algorithm
    pub async fn create(pool: &PgPool, req: CreateAlgoRequest) -> Result<Self, sqlx::Error> {
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
        .fetch_one(pool)
        .await
    }

    /// Get an algorithm by its link
    pub async fn get_by_link(pool: &PgPool, link: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM algorithms WHERE algo_link = $1")
            .bind(link)
            .fetch_optional(pool)
            .await
    }

    /// Get an algorithm by ID
    pub async fn get_by_id(pool: &PgPool, id: i32) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM algorithms WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// Get all algorithms with pagination
    pub async fn get_paginated(
        pool: &PgPool,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<Self>, i64), sqlx::Error> {
        let offset = (page - 1) * limit;
        let algos = sqlx::query_as(
            "SELECT * FROM algorithms ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM algorithms")
            .fetch_one(pool)
            .await?;
        Ok((algos, total.0))
    }
} 