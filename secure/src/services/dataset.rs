use common::{ApiError, ApiResult, PaginationParams, PaginatedResponse};
use sqlx::PgPool;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Static dataset database model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StaticDataset {
    pub id: i64,
    pub name: String,
    pub ui_name: String,
    pub description: Option<String>,
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

/// Request for creating a new static dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStaticDatasetRequest {
    pub name: String,
    pub ui_name: String,
    pub description: Option<String>,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub file_size: i64,
    pub file_format: String,
    pub author: Option<String>,
    pub author_wallet: String,
    pub sample_url: Option<String>,
    pub file_path: Option<String>,
}

/// Request for updating a static dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStaticDatasetRequest {
    pub name: Option<String>,
    pub ui_name: Option<String>,
    pub description: Option<String>,
}

/// Static dataset service for database operations
pub struct DatasetService;

impl DatasetService {
    /// Create a new static dataset
    pub async fn create_static_dataset(
        pool: &PgPool,
        req: CreateStaticDatasetRequest,
    ) -> ApiResult<StaticDataset> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            INSERT INTO static_datasets (
                name, ui_name, description, file_hash, ipfs_cid, file_size, 
                file_format, author, author_wallet, sample_url, file_path
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
            req.name,
            req.ui_name,
            req.description,
            req.file_hash,
            req.ipfs_cid,
            req.file_size,
            req.file_format,
            req.author,
            req.author_wallet,
            req.sample_url,
            req.file_path
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create static dataset");
            ApiError::InternalError("Failed to create dataset".to_string())
        })?;

        Ok(dataset)
    }

    /// Get static datasets with pagination, only confirmed ones
    pub async fn get_static_datasets(
        pool: &PgPool,
        params: PaginationParams,
    ) -> ApiResult<PaginatedResponse<StaticDataset>> {
        let offset = (params.page - 1) * params.limit;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM static_datasets sd
            INNER JOIN blockchain_transactions bt ON bt.entity_id = sd.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'STATIC_DATASET'
            "#
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to count static datasets");
            ApiError::InternalError("Failed to count datasets".to_string())
        })?
        .unwrap_or(0);

        // Get paginated results
        let datasets = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT sd.*
            FROM static_datasets sd
            INNER JOIN blockchain_transactions bt ON bt.entity_id = sd.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'STATIC_DATASET'
            ORDER BY sd.created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            params.limit as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get static datasets");
            ApiError::InternalError("Failed to get datasets".to_string())
        })?;

        Ok(PaginatedResponse::new(
            datasets,
            params.page,
            params.limit,
            total as u64,
        ))
    }

    /// Get a static dataset by ID, only confirmed ones
    pub async fn get_static_dataset_by_id(
        pool: &PgPool,
        id: i64,
    ) -> ApiResult<StaticDataset> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT sd.*
            FROM static_datasets sd
            INNER JOIN blockchain_transactions bt ON bt.entity_id = sd.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'STATIC_DATASET'
            WHERE sd.id = $1
            "#,
            id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, dataset_id = %id, "Failed to get static dataset");
            ApiError::NotFound("Dataset not found".to_string())
        })?;

        Ok(dataset)
    }

    /// Get a static dataset by name, only confirmed ones
    pub async fn get_static_dataset_by_name(
        pool: &PgPool,
        name: &str,
    ) -> ApiResult<StaticDataset> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT sd.*
            FROM static_datasets sd
            INNER JOIN blockchain_transactions bt ON bt.entity_id = sd.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'STATIC_DATASET'
            WHERE sd.name = $1
            "#,
            name
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, dataset_name = %name, "Failed to get static dataset by name");
            ApiError::NotFound("Dataset not found".to_string())
        })?;

        Ok(dataset)
    }

    /// Get a static dataset by file hash, only confirmed ones
    pub async fn get_static_dataset_by_hash(
        pool: &PgPool,
        file_hash: &str,
    ) -> ApiResult<StaticDataset> {
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            SELECT sd.*
            FROM static_datasets sd
            INNER JOIN blockchain_transactions bt ON bt.entity_id = sd.id
                AND bt.status = 'CONFIRMED'
                AND bt.entity_type = 'STATIC_DATASET'
            WHERE sd.file_hash = $1
            "#,
            file_hash
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, file_hash = %file_hash, "Failed to get static dataset by hash");
            ApiError::NotFound("Dataset not found".to_string())
        })?;

        Ok(dataset)
    }

    /// Update a static dataset
    pub async fn update_static_dataset(
        pool: &PgPool,
        id: i64,
        req: UpdateStaticDatasetRequest,
    ) -> ApiResult<StaticDataset> {
        // Build dynamic update query
        let mut query = "UPDATE static_datasets SET".to_string();
        let mut params = Vec::new();
        let mut param_count = 1;

        if let Some(name) = &req.name {
            query.push_str(&format!(" name = ${},", param_count));
            params.push(name.clone());
            param_count += 1;
        }

        if let Some(ui_name) = &req.ui_name {
            query.push_str(&format!(" ui_name = ${},", param_count));
            params.push(ui_name.clone());
            param_count += 1;
        }

        if let Some(description) = &req.description {
            query.push_str(&format!(" description = ${},", param_count));
            params.push(description.clone());
            param_count += 1;
        }

        if param_count == 1 {
            return Err(ApiError::BadRequest("No fields to update".to_string()));
        }

        // Remove trailing comma and add WHERE clause
        query.pop();
        query.push_str(&format!(" WHERE id = ${} RETURNING *", param_count));
        params.push(id.to_string());

        // Execute update (this is a simplified version - in production, use a proper query builder)
        // For now, let's implement a simple version with all possible fields
        let dataset = sqlx::query_as!(
            StaticDataset,
            r#"
            UPDATE static_datasets 
            SET name = COALESCE($1, name),
                ui_name = COALESCE($2, ui_name),
                description = COALESCE($3, description),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $4
            RETURNING *
            "#,
            req.name,
            req.ui_name,
            req.description,
            id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, dataset_id = %id, "Failed to update static dataset");
            ApiError::InternalError("Failed to update dataset".to_string())
        })?;

        Ok(dataset)
    }
} 