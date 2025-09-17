use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use validator::Validate;

use crate::{AppError, Result};

/// Dataset schema entity for describing dataset fields
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DatasetSchema {
    pub id: i64,
    pub dataset_id: i64,
    pub field_name: String,
    pub field_type: String,
    pub description: Option<String>,
    pub is_nullable: bool,
    pub field_order: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Request to create a new schema field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSchemaFieldRequest {
    pub dataset_id: i64,
    pub field_name: String,
    pub field_type: String,
    pub description: Option<String>,
    pub is_nullable: Option<bool>,
    pub field_order: Option<i32>,
}

/// Request to update a schema field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSchemaFieldRequest {
    pub field_type: Option<String>,
    pub description: Option<String>,
    pub is_nullable: Option<bool>,
    pub field_order: Option<i32>,
}

/// Request to set complete schema for a dataset
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SetDatasetSchemaRequest {
    pub fields: Vec<SchemaFieldDefinition>,
}

/// Schema field definition
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SchemaFieldDefinition {
    pub field_name: String,
    pub field_type: String,
    pub description: Option<String>,
    pub is_nullable: Option<bool>,
    pub field_order: Option<i32>,
}

impl DatasetSchema {
    /// Create a new schema field
    pub async fn create(pool: &PgPool, request: CreateSchemaFieldRequest) -> Result<Self> {
        let result = sqlx::query_as!(
            DatasetSchema,
            r#"
            INSERT INTO dataset_schema (
                dataset_id, field_name, field_type, description,
                is_nullable, field_order
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, dataset_id, field_name, field_type, description,
                      COALESCE(is_nullable, true) as "is_nullable!", field_order,
                      created_at as "created_at!"
            "#,
            request.dataset_id,
            &request.field_name,
            &request.field_type,
            request.description.as_deref(),
            request.is_nullable.unwrap_or(true),
            request.field_order
        )
        .fetch_one(pool)
        .await;

        match result {
            Ok(schema) => Ok(schema),
            Err(sqlx::Error::Database(db_err)) => {
                if db_err.is_unique_violation() {
                    Err(AppError::Conflict(format!(
                        "Field '{}' already exists in dataset schema",
                        request.field_name
                    )))
                } else {
                    Err(AppError::from(sqlx::Error::Database(db_err)))
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Get all schema fields for a dataset
    pub async fn get_by_dataset_id(pool: &PgPool, dataset_id: i64) -> Result<Vec<Self>> {
        let fields = sqlx::query_as!(
            DatasetSchema,
            r#"
            SELECT id, dataset_id, field_name, field_type, description,
                   COALESCE(is_nullable, true) as "is_nullable!", field_order,
                   created_at as "created_at!"
            FROM dataset_schema
            WHERE dataset_id = $1
            ORDER BY field_order ASC NULLS LAST, field_name ASC
            "#,
            dataset_id
        )
        .fetch_all(pool)
        .await?;

        Ok(fields)
    }

    /// Get a specific schema field by ID
    pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Self>> {
        let field = sqlx::query_as!(
            DatasetSchema,
            r#"
            SELECT id, dataset_id, field_name, field_type, description,
                   COALESCE(is_nullable, true) as "is_nullable!", field_order,
                   created_at as "created_at!"
            FROM dataset_schema
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(field)
    }

    /// Find a schema field by dataset_id and field_name
    pub async fn find_by_dataset_and_field(
        pool: &PgPool,
        dataset_id: i64,
        field_name: &str,
    ) -> Result<Option<Self>> {
        let field = sqlx::query_as!(
            DatasetSchema,
            r#"
            SELECT id, dataset_id, field_name, field_type, description,
                   COALESCE(is_nullable, true) as "is_nullable!", field_order,
                   created_at as "created_at!"
            FROM dataset_schema
            WHERE dataset_id = $1 AND field_name = $2
            "#,
            dataset_id,
            field_name
        )
        .fetch_optional(pool)
        .await?;

        Ok(field)
    }

    /// Update a schema field
    pub async fn update(pool: &PgPool, id: i64, request: UpdateSchemaFieldRequest) -> Result<Self> {
        let field = sqlx::query_as!(
            DatasetSchema,
            r#"
            UPDATE dataset_schema
            SET field_type = COALESCE($1, field_type),
                description = COALESCE($2, description),
                is_nullable = COALESCE($3, is_nullable),
                field_order = COALESCE($4, field_order)
            WHERE id = $5
            RETURNING id, dataset_id, field_name, field_type, description,
                      COALESCE(is_nullable, true) as "is_nullable!", field_order,
                      created_at as "created_at!"
            "#,
            request.field_type.as_deref(),
            request.description.as_deref(),
            request.is_nullable,
            request.field_order,
            id
        )
        .fetch_one(pool)
        .await?;

        Ok(field)
    }

    /// Delete a schema field
    pub async fn delete(pool: &PgPool, id: i64) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            DELETE FROM dataset_schema
            WHERE id = $1
            "#,
            id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete all schema fields for a dataset
    pub async fn delete_by_dataset_id(pool: &PgPool, dataset_id: i64) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM dataset_schema
            WHERE dataset_id = $1
            "#,
            dataset_id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Set complete schema for a dataset (replace existing schema)
    pub async fn set_dataset_schema(
        pool: &PgPool,
        dataset_id: i64,
        request: SetDatasetSchemaRequest,
    ) -> Result<Vec<Self>> {
        let mut tx = pool.begin().await?;

        // Delete existing schema
        sqlx::query!(
            r#"
            DELETE FROM dataset_schema
            WHERE dataset_id = $1
            "#,
            dataset_id
        )
        .execute(&mut *tx)
        .await?;

        // Insert new schema fields
        let mut created_fields = Vec::new();
        for (index, field) in request.fields.iter().enumerate() {
            let field_order = field.field_order.or(Some(index as i32));

            let created_field = sqlx::query_as!(
                DatasetSchema,
                r#"
                INSERT INTO dataset_schema (
                    dataset_id, field_name, field_type, description,
                    is_nullable, field_order
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, dataset_id, field_name, field_type, description,
                          COALESCE(is_nullable, true) as "is_nullable!", field_order,
                          created_at as "created_at!"
                "#,
                dataset_id,
                &field.field_name,
                &field.field_type,
                field.description.as_deref(),
                field.is_nullable.unwrap_or(true),
                field_order
            )
            .fetch_one(&mut *tx)
            .await?;

            created_fields.push(created_field);
        }

        tx.commit().await?;
        Ok(created_fields)
    }

    /// Add multiple schema fields to a dataset
    pub async fn add_fields(
        pool: &PgPool,
        dataset_id: i64,
        fields: Vec<SchemaFieldDefinition>,
    ) -> Result<Vec<Self>> {
        let mut tx = pool.begin().await?;
        let mut created_fields = Vec::new();

        // Get the current maximum field_order
        let max_order = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(MAX(field_order), -1) as "max_order!"
            FROM dataset_schema
            WHERE dataset_id = $1
            "#,
            dataset_id
        )
        .fetch_one(&mut *tx)
        .await?;

        for (index, field) in fields.iter().enumerate() {
            let field_order = field.field_order.or(Some(max_order + 1 + index as i32));

            let created_field = sqlx::query_as!(
                DatasetSchema,
                r#"
                INSERT INTO dataset_schema (
                    dataset_id, field_name, field_type, description,
                    is_nullable, field_order
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, dataset_id, field_name, field_type, description,
                          COALESCE(is_nullable, true) as "is_nullable!", field_order,
                          created_at as "created_at!"
                "#,
                dataset_id,
                &field.field_name,
                &field.field_type,
                field.description.as_deref(),
                field.is_nullable.unwrap_or(true),
                field_order
            )
            .fetch_one(&mut *tx)
            .await?;

            created_fields.push(created_field);
        }

        tx.commit().await?;
        Ok(created_fields)
    }

    /// Reorder schema fields for a dataset
    pub async fn reorder_fields(
        pool: &PgPool,
        dataset_id: i64,
        field_orders: Vec<(i64, i32)>, // (field_id, new_order)
    ) -> Result<()> {
        let mut tx = pool.begin().await?;

        for (field_id, new_order) in field_orders {
            sqlx::query!(
                r#"
                UPDATE dataset_schema
                SET field_order = $1
                WHERE id = $2 AND dataset_id = $3
                "#,
                new_order,
                field_id,
                dataset_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Count schema fields for a dataset
    pub async fn count_fields(pool: &PgPool, dataset_id: i64) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM dataset_schema
            WHERE dataset_id = $1
            "#,
            dataset_id
        )
        .fetch_one(pool)
        .await?;

        Ok(count)
    }

    /// Validate field types (ensure they are in allowed list)
    pub fn validate_field_type(field_type: &str) -> bool {
        matches!(
            field_type,
            "string"
                | "number"
                | "boolean"
                | "date"
                | "datetime"
                | "integer"
                | "float"
                | "decimal"
                | "text"
                | "json"
                | "array"
        )
    }
}
