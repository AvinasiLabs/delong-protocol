use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// DynamicDataset model for mutable datasets stored locally
/// Matches the Go struct from /root/delong/internal/models/dyn_dataset.go
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicDataset {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating a dynamic dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDynamicDatasetRequest {
    pub name: String,
    pub ui_name: String,  // Note: Go function has this parameter but struct doesn't use it
    pub description: String,
    pub file_path: String,
}

/// Request model for updating a dynamic dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDynamicDatasetRequest {
    pub description: String,
}

/// Response model for dynamic dataset info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicDatasetInfo {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DynamicDataset> for DynamicDatasetInfo {
    fn from(dataset: DynamicDataset) -> Self {
        DynamicDatasetInfo {
            id: dataset.id,
            name: dataset.name,
            description: dataset.description,
            file_path: dataset.file_path,
            created_at: dataset.created_at,
            updated_at: dataset.updated_at,
        }
    }
}

impl DynamicDataset {
    /// Create a new dynamic dataset - matches CreateDataset function
    pub async fn create(
        pool: &PgPool,
        name: &str,
        _ui_name: &str,  // Parameter exists in Go function but not used
        description: &str,
        file_path: &str,
    ) -> Result<Self, sqlx::Error> {
        let dataset = sqlx::query_as!(
            DynamicDataset,
            r#"
            INSERT INTO dynamic_datasets (name, description, file_path)
            VALUES ($1, $2, $3)
            RETURNING id, name, description, file_path, created_at, updated_at
            "#,
            name,
            description,
            file_path
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }

    /// Get paginated dynamic datasets - matches GetDatasets function
    pub async fn get_paginated(
        pool: &PgPool,
        page: i32,
        page_size: i32,
    ) -> Result<(Vec<Self>, i64), sqlx::Error> {
        let offset = (page - 1) * page_size;

        // Get total count
        let total_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM dynamic_datasets
            "#
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0);

        // Get paginated results
        let datasets = sqlx::query_as!(
            DynamicDataset,
            r#"
            SELECT id, name, description, file_path, created_at, updated_at
            FROM dynamic_datasets
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            page_size,
            offset
        )
        .fetch_all(pool)
        .await?;

        Ok((datasets, total_count))
    }

    /// Get dynamic dataset by ID - matches GetDatasetByID function
    pub async fn get_by_id(pool: &PgPool, id: i32) -> Result<Self, sqlx::Error> {
        let dataset = sqlx::query_as!(
            DynamicDataset,
            r#"
            SELECT id, name, description, file_path, created_at, updated_at
            FROM dynamic_datasets
            WHERE id = $1
            "#,
            id
        )
        .fetch_one(pool)
        .await?;

        Ok(dataset)
    }

    /// Update dynamic dataset - matches UpdateDataset function
    pub async fn update(
        pool: &PgPool,
        id: i32,
        description: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE dynamic_datasets 
            SET description = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            "#,
            description,
            id
        )
        .execute(pool)
        .await?;

        // Fetch the updated dataset
        Self::get_by_id(pool, id).await
    }

    /// Delete dynamic dataset - matches DeleteDataset function
    pub async fn delete(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            DELETE FROM dynamic_datasets
            WHERE id = $1
            "#,
            id
        )
        .execute(pool)
        .await?;

        Ok(())
    }
} 