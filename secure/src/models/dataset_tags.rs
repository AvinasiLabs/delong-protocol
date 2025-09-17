use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use validator::Validate;

use crate::Result;

/// Dataset tag entity for categorizing datasets
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DatasetTag {
    pub id: i64,
    pub dataset_id: i64,
    pub tag: String,
    pub created_at: DateTime<Utc>,
}

/// Request to add tags to a dataset
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AddTagsRequest {
    pub tags: Vec<String>,
}

/// Request to remove tags from a dataset
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RemoveTagsRequest {
    pub tags: Vec<String>,
}

impl DatasetTag {
    /// Add a single tag to a dataset
    pub async fn add_tag(pool: &PgPool, dataset_id: i64, tag: &str) -> Result<Self> {
        let tag = sqlx::query_as!(
            DatasetTag,
            r#"
            INSERT INTO dataset_tags (dataset_id, tag)
            VALUES ($1, $2)
            ON CONFLICT (dataset_id, tag) DO UPDATE
            SET created_at = CURRENT_TIMESTAMP
            RETURNING id, dataset_id, tag, created_at as "created_at!"
            "#,
            dataset_id,
            tag
        )
        .fetch_one(pool)
        .await?;

        Ok(tag)
    }

    /// Add multiple tags to a dataset
    pub async fn add_tags(pool: &PgPool, dataset_id: i64, tags: &[String]) -> Result<Vec<Self>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = pool.begin().await?;
        let mut added_tags = Vec::new();

        for tag in tags {
            let dataset_tag = sqlx::query_as!(
                DatasetTag,
                r#"
                INSERT INTO dataset_tags (dataset_id, tag)
                VALUES ($1, $2)
                ON CONFLICT (dataset_id, tag) DO UPDATE
                SET created_at = dataset_tags.created_at
                RETURNING id, dataset_id, tag, created_at as "created_at!"
                "#,
                dataset_id,
                tag
            )
            .fetch_one(&mut *tx)
            .await?;

            added_tags.push(dataset_tag);
        }

        tx.commit().await?;
        Ok(added_tags)
    }

    /// Remove a single tag from a dataset
    pub async fn remove_tag(pool: &PgPool, dataset_id: i64, tag: &str) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            DELETE FROM dataset_tags
            WHERE dataset_id = $1 AND tag = $2
            "#,
            dataset_id,
            tag
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Remove multiple tags from a dataset
    pub async fn remove_tags(pool: &PgPool, dataset_id: i64, tags: &[String]) -> Result<u64> {
        if tags.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query!(
            r#"
            DELETE FROM dataset_tags
            WHERE dataset_id = $1 AND tag = ANY($2)
            "#,
            dataset_id,
            tags
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get all tags for a dataset
    pub async fn get_dataset_tags(pool: &PgPool, dataset_id: i64) -> Result<Vec<String>> {
        let tags = sqlx::query_scalar!(
            r#"
            SELECT tag
            FROM dataset_tags
            WHERE dataset_id = $1
            ORDER BY tag
            "#,
            dataset_id
        )
        .fetch_all(pool)
        .await?;

        Ok(tags)
    }

    /// Find all datasets with a specific tag
    pub async fn find_datasets_by_tag(
        pool: &PgPool,
        tag: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<i64>, u64)> {
        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT dataset_id) as "count!"
            FROM dataset_tags
            WHERE tag = $1
            "#,
            tag
        )
        .fetch_one(pool)
        .await?;

        // Get paginated dataset IDs
        let dataset_ids = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT dataset_id
            FROM dataset_tags
            WHERE tag = $1
            ORDER BY dataset_id DESC
            LIMIT $2 OFFSET $3
            "#,
            tag,
            per_page as i64,
            ((page - 1) * per_page) as i64
        )
        .fetch_all(pool)
        .await?;

        Ok((dataset_ids, total as u64))
    }

    /// Find all datasets with multiple tags (AND condition)
    pub async fn find_datasets_by_tags_all(
        pool: &PgPool,
        tags: &[String],
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<i64>, u64)> {
        if tags.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let tag_count = tags.len() as i64;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM (
                SELECT dataset_id
                FROM dataset_tags
                WHERE tag = ANY($1)
                GROUP BY dataset_id
                HAVING COUNT(DISTINCT tag) = $2
            ) AS datasets_with_all_tags
            "#,
            tags,
            tag_count
        )
        .fetch_one(pool)
        .await?;

        // Get paginated dataset IDs
        let dataset_ids = sqlx::query_scalar!(
            r#"
            SELECT dataset_id
            FROM dataset_tags
            WHERE tag = ANY($1)
            GROUP BY dataset_id
            HAVING COUNT(DISTINCT tag) = $2
            ORDER BY dataset_id DESC
            LIMIT $3 OFFSET $4
            "#,
            tags,
            tag_count,
            per_page as i64,
            ((page - 1) * per_page) as i64
        )
        .fetch_all(pool)
        .await?;

        Ok((dataset_ids, total as u64))
    }

    /// Find all datasets with any of the tags (OR condition)
    pub async fn find_datasets_by_tags_any(
        pool: &PgPool,
        tags: &[String],
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<i64>, u64)> {
        if tags.is_empty() {
            return Ok((Vec::new(), 0));
        }

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT dataset_id) as "count!"
            FROM dataset_tags
            WHERE tag = ANY($1)
            "#,
            tags
        )
        .fetch_one(pool)
        .await?;

        // Get paginated dataset IDs
        let dataset_ids = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT dataset_id
            FROM dataset_tags
            WHERE tag = ANY($1)
            ORDER BY dataset_id DESC
            LIMIT $2 OFFSET $3
            "#,
            tags,
            per_page as i64,
            ((page - 1) * per_page) as i64
        )
        .fetch_all(pool)
        .await?;

        Ok((dataset_ids, total as u64))
    }

    /// Get all unique tags with their counts
    pub async fn get_all_tags_with_count(pool: &PgPool) -> Result<Vec<TagWithCount>> {
        let tags = sqlx::query_as!(
            TagWithCount,
            r#"
            SELECT
                tag,
                COUNT(*) as "count!"
            FROM dataset_tags
            GROUP BY tag
            ORDER BY COUNT(*) DESC, tag
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(tags)
    }

    /// Get popular tags (most used tags)
    pub async fn get_popular_tags(pool: &PgPool, limit: i32) -> Result<Vec<TagWithCount>> {
        let tags = sqlx::query_as!(
            TagWithCount,
            r#"
            SELECT
                tag,
                COUNT(*) as "count!"
            FROM dataset_tags
            GROUP BY tag
            ORDER BY COUNT(*) DESC
            LIMIT $1
            "#,
            limit as i64
        )
        .fetch_all(pool)
        .await?;

        Ok(tags)
    }

    /// Clear all tags for a dataset
    pub async fn clear_dataset_tags(pool: &PgPool, dataset_id: i64) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM dataset_tags
            WHERE dataset_id = $1
            "#,
            dataset_id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Update tags for a dataset (replace all existing tags)
    pub async fn update_dataset_tags(
        pool: &PgPool,
        dataset_id: i64,
        tags: &[String],
    ) -> Result<Vec<String>> {
        let mut tx = pool.begin().await?;

        // Remove all existing tags
        sqlx::query!(
            r#"
            DELETE FROM dataset_tags
            WHERE dataset_id = $1
            "#,
            dataset_id
        )
        .execute(&mut *tx)
        .await?;

        // Add new tags
        for tag in tags {
            sqlx::query!(
                r#"
                INSERT INTO dataset_tags (dataset_id, tag)
                VALUES ($1, $2)
                "#,
                dataset_id,
                tag
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(tags.to_vec())
    }
}

/// Tag with count information
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TagWithCount {
    pub tag: String,
    pub count: i64,
}
