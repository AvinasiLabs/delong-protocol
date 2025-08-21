//! API Key data model and database access functions
//!
//! This module contains the API Key data model and all database access
//! functions for API key management. API keys are used for third-party
//! developer authentication and access control.

use crate::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use tracing::error;
use utoipa::ToSchema;
use uuid::Uuid;

/// Rate limit tier for API keys
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, ToSchema)]
#[sqlx(type_name = "rate_limit_tier", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum RateLimitTier {
    Basic,
    Premium,
    Enterprise,
}

impl Default for RateLimitTier {
    fn default() -> Self {
        Self::Basic
    }
}

impl std::fmt::Display for RateLimitTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateLimitTier::Basic => write!(f, "basic"),
            RateLimitTier::Premium => write!(f, "premium"),
            RateLimitTier::Enterprise => write!(f, "enterprise"),
        }
    }
}

impl std::str::FromStr for RateLimitTier {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "basic" => Ok(RateLimitTier::Basic),
            "premium" => Ok(RateLimitTier::Premium),
            "enterprise" => Ok(RateLimitTier::Enterprise),
            _ => Err(AppError::Validation(format!(
                "Invalid rate limit tier: {}",
                s
            ))),
        }
    }
}

/// API Key database model
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiKey {
    pub id: i32,
    pub api_key: String,
    pub name: String,
    pub user_id: i32,
    pub permissions: Vec<String>,
    pub rate_limit_tier: RateLimitTier,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Statistics for API keys
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiKeyStats {
    pub total_keys: i64,
    pub active_keys: i64,
    pub inactive_keys: i64,
    pub expired_keys: i64,
    pub by_tier: serde_json::Value,
}

impl ApiKey {
    /// Create a new API key
    pub async fn create(
        pool: &PgPool,
        user_id: i32,
        name: String,
        permissions: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, AppError> {
        // Check if a key with the same name already exists for this user
        let existing = sqlx::query!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM api_keys
            WHERE user_id = $1 AND name = $2
            "#,
            user_id,
            name
        )
        .fetch_one(pool)
        .await?;

        if existing.count > 0 {
            return Err(AppError::Conflict(format!(
                "API key with name '{}' already exists",
                name
            )));
        }

        // Generate a new API key
        let api_key = generate_api_key();
        let permissions_json = serde_json::to_value(&permissions)?;
        let rate_limit_tier = RateLimitTier::default();

        let record = sqlx::query!(
            r#"
            INSERT INTO api_keys (api_key, name, user_id, permissions, rate_limit_tier, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id,
                api_key,
                name,
                user_id,
                permissions,
                rate_limit_tier as "rate_limit_tier: RateLimitTier",
                is_active,
                expires_at,
                last_used_at,
                created_at,
                updated_at
            "#,
            api_key,
            name,
            user_id,
            permissions_json,
            rate_limit_tier as RateLimitTier,
            expires_at
        )
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to create API key: {}", e);
            if e.to_string().contains("duplicate") {
                AppError::Conflict("API key with this name already exists".to_string())
            } else {
                AppError::Database(e)
            }
        })?;

        // Convert permissions from Value to Vec<String>
        let permissions = serde_json::from_value(record.permissions).unwrap_or_else(|_| Vec::new());

        Ok(ApiKey {
            id: record.id,
            api_key: record.api_key,
            name: record.name,
            user_id: record.user_id,
            permissions,
            rate_limit_tier: record.rate_limit_tier,
            is_active: record.is_active,
            expires_at: record.expires_at,
            last_used_at: record.last_used_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    /// Find API key by key value
    pub async fn find_by_key(pool: &PgPool, api_key: &str) -> Result<Self, AppError> {
        let record = sqlx::query!(
            r#"
            SELECT
                id,
                api_key,
                name,
                user_id,
                permissions,
                rate_limit_tier as "rate_limit_tier: RateLimitTier",
                is_active,
                expires_at,
                last_used_at,
                created_at,
                updated_at
            FROM api_keys
            WHERE api_key = $1
            "#,
            api_key
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("API key not found".to_string()))?;

        // Convert permissions from Value to Vec<String>
        let permissions = serde_json::from_value(record.permissions).unwrap_or_else(|_| Vec::new());

        Ok(ApiKey {
            id: record.id,
            api_key: record.api_key,
            name: record.name,
            user_id: record.user_id,
            permissions,
            rate_limit_tier: record.rate_limit_tier,
            is_active: record.is_active,
            expires_at: record.expires_at,
            last_used_at: record.last_used_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    /// Find API key by ID
    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Self, AppError> {
        let record = sqlx::query!(
            r#"
            SELECT
                id,
                api_key,
                name,
                user_id,
                permissions,
                rate_limit_tier as "rate_limit_tier: RateLimitTier",
                is_active,
                expires_at,
                last_used_at,
                created_at,
                updated_at
            FROM api_keys
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("API key not found".to_string()))?;

        // Convert permissions from Value to Vec<String>
        let permissions = serde_json::from_value(record.permissions).unwrap_or_else(|_| Vec::new());

        Ok(ApiKey {
            id: record.id,
            api_key: record.api_key,
            name: record.name,
            user_id: record.user_id,
            permissions,
            rate_limit_tier: record.rate_limit_tier,
            is_active: record.is_active,
            expires_at: record.expires_at,
            last_used_at: record.last_used_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    /// Get user's API keys with pagination and filters
    pub async fn get_user_keys(
        pool: &PgPool,
        user_id: i32,
        is_active: Option<bool>,
        rate_limit_tier: Option<String>,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<ApiKey>, i64), AppError> {
        let offset = ((page - 1) * per_page) as i64;
        let limit = per_page as i64;

        // Keep rate_limit_tier as string for SQL comparison
        let tier_filter = rate_limit_tier.clone();

        // Count total records - using a single query with optional filters
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM api_keys
            WHERE user_id = $1
              AND ($2::boolean IS NULL OR is_active = $2)
              AND ($3::text IS NULL OR rate_limit_tier::text = $3)
            "#,
            user_id,
            is_active,
            tier_filter.as_deref()
        )
        .fetch_one(pool)
        .await?;

        // Fetch paginated records - using a single query with optional filters
        let records = sqlx::query!(
            r#"
            SELECT
                id,
                api_key,
                name,
                user_id,
                permissions,
                rate_limit_tier as "rate_limit_tier: RateLimitTier",
                is_active,
                expires_at,
                last_used_at,
                created_at,
                updated_at
            FROM api_keys
            WHERE user_id = $1
              AND ($2::boolean IS NULL OR is_active = $2)
              AND ($3::text IS NULL OR rate_limit_tier::text = $3)
            ORDER BY created_at DESC
            LIMIT $4 OFFSET $5
            "#,
            user_id,
            is_active,
            tier_filter.as_deref(),
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;

        // Convert records to ApiKey structs
        let keys: Vec<ApiKey> = records
            .into_iter()
            .map(|record| {
                let permissions =
                    serde_json::from_value(record.permissions).unwrap_or_else(|_| Vec::new());

                ApiKey {
                    id: record.id,
                    api_key: record.api_key,
                    name: record.name,
                    user_id: record.user_id,
                    permissions,
                    rate_limit_tier: record.rate_limit_tier,
                    is_active: record.is_active,
                    expires_at: record.expires_at,
                    last_used_at: record.last_used_at,
                    created_at: record.created_at,
                    updated_at: record.updated_at,
                }
            })
            .collect();

        Ok((keys, total))
    }

    /// Update an API key
    pub async fn update(
        pool: &PgPool,
        id: i32,
        name: Option<String>,
        permissions: Option<Vec<String>>,
        is_active: Option<bool>,
        expires_at: Option<Option<DateTime<Utc>>>,
    ) -> Result<Self, AppError> {
        // First get the current API key
        let current = Self::find_by_id(pool, id).await?;

        // Update fields if provided
        let final_name = name.unwrap_or(current.name);
        let final_permissions = permissions.unwrap_or(current.permissions);
        let final_is_active = is_active.unwrap_or(current.is_active);
        let final_expires_at = expires_at.unwrap_or(current.expires_at);

        let permissions_json = serde_json::to_value(&final_permissions)?;

        // Update in database
        let record = sqlx::query!(
            r#"
            UPDATE api_keys
            SET
                name = $2,
                permissions = $3,
                is_active = $4,
                expires_at = $5,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                api_key,
                name,
                user_id,
                permissions,
                rate_limit_tier as "rate_limit_tier: RateLimitTier",
                is_active,
                expires_at,
                last_used_at,
                created_at,
                updated_at
            "#,
            id,
            final_name,
            permissions_json,
            final_is_active,
            final_expires_at
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("API key not found".to_string()))?;

        // Convert permissions from Value to Vec<String>
        let permissions = serde_json::from_value(record.permissions).unwrap_or_else(|_| Vec::new());

        Ok(ApiKey {
            id: record.id,
            api_key: record.api_key,
            name: record.name,
            user_id: record.user_id,
            permissions,
            rate_limit_tier: record.rate_limit_tier,
            is_active: record.is_active,
            expires_at: record.expires_at,
            last_used_at: record.last_used_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    /// Delete an API key
    pub async fn delete(pool: &PgPool, id: i32) -> Result<(), AppError> {
        let result = sqlx::query!("DELETE FROM api_keys WHERE id = $1", id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("API key not found".to_string()));
        }

        Ok(())
    }

    /// Update last used timestamp
    pub async fn update_last_used(pool: &PgPool, id: i32) -> Result<(), AppError> {
        let now = Utc::now();
        sqlx::query!(
            "UPDATE api_keys SET last_used_at = $1 WHERE id = $2",
            now,
            id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get API key statistics for a user
    pub async fn get_user_stats(pool: &PgPool, user_id: i32) -> Result<ApiKeyStats, AppError> {
        // Get counts
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM api_keys WHERE user_id = $1"#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        let active = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM api_keys WHERE user_id = $1 AND is_active = true"#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        let expired = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM api_keys WHERE user_id = $1 AND expires_at < NOW()"#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        // Get counts by tier
        let tier_counts = sqlx::query!(
            r#"
            SELECT
                rate_limit_tier as "rate_limit_tier: RateLimitTier",
                COUNT(*) as "count!"
            FROM api_keys
            WHERE user_id = $1
            GROUP BY rate_limit_tier
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        let mut by_tier = serde_json::Map::new();
        for tc in tier_counts {
            by_tier.insert(tc.rate_limit_tier.to_string(), json!(tc.count));
        }

        Ok(ApiKeyStats {
            total_keys: total,
            active_keys: active,
            inactive_keys: total - active,
            expired_keys: expired,
            by_tier: json!(by_tier),
        })
    }

    /// Check if the API key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// Check if the API key is valid (active and not expired)
    pub fn is_valid(&self) -> bool {
        self.is_active && !self.is_expired()
    }
}

/// Generate a new API key
fn generate_api_key() -> String {
    format!("dl_{}", Uuid::new_v4().to_string().replace("-", ""))
}

/// Validate API key format
pub fn is_valid_api_key_format(api_key: &str) -> bool {
    api_key.starts_with("dl_") && api_key.len() == 35
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key() {
        let key = generate_api_key();
        assert!(key.starts_with("dl_"));
        assert_eq!(key.len(), 35);
    }

    #[test]
    fn test_is_valid_api_key_format() {
        let key = generate_api_key();
        assert!(is_valid_api_key_format(&key));

        let invalid_key1 = "invalid_key";
        let invalid_key2 = "dl_short";

        assert!(!is_valid_api_key_format(invalid_key1));
        assert!(!is_valid_api_key_format(invalid_key2));
    }

    #[test]
    fn test_rate_limit_tier_from_str() {
        assert_eq!(
            "basic".parse::<RateLimitTier>().unwrap(),
            RateLimitTier::Basic
        );
        assert_eq!(
            "premium".parse::<RateLimitTier>().unwrap(),
            RateLimitTier::Premium
        );
        assert_eq!(
            "enterprise".parse::<RateLimitTier>().unwrap(),
            RateLimitTier::Enterprise
        );
        assert!("invalid".parse::<RateLimitTier>().is_err());
    }

    #[test]
    fn test_api_key_expired() {
        let mut api_key = ApiKey {
            id: 1,
            api_key: "test_key".to_string(),
            name: "Test Key".to_string(),
            user_id: 1,
            permissions: vec![],
            rate_limit_tier: RateLimitTier::Basic,
            is_active: true,
            expires_at: Some(Utc::now() - chrono::Duration::days(1)),
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(api_key.is_expired());
        assert!(!api_key.is_valid());

        api_key.expires_at = Some(Utc::now() + chrono::Duration::days(1));
        assert!(!api_key.is_expired());
        assert!(api_key.is_valid());

        api_key.is_active = false;
        assert!(!api_key.is_valid());
    }
}
