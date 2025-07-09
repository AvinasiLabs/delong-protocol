//! API Key data model and database access functions
//!
//! This module contains the API Key data model and all database access
//! functions for API key management. API keys are used for third-party
//! developer authentication and access control.

use chrono::{DateTime, Utc};
use common::ApiError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use std::collections::HashMap;
use tracing::error;
use uuid::Uuid;
use validator::Validate;

/// Rate limit tier for API keys
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
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
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "basic" => Ok(RateLimitTier::Basic),
            "premium" => Ok(RateLimitTier::Premium),
            "enterprise" => Ok(RateLimitTier::Enterprise),
            _ => Err(ApiError::InvalidInput(format!(
                "Invalid rate limit tier: {}",
                s
            ))),
        }
    }
}

/// API Key database model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: i32,
    pub api_key: String,
    pub name: String,
    pub user_id: i32,
    pub permissions: serde_json::Value,
    pub rate_limit_tier: String,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request model for creating API keys
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateApiKeyRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub permissions: Option<Vec<String>>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Request model for updating API keys
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateApiKeyRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub is_active: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response model for API key creation and retrieval
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyResponse {
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

/// Response model for API key list (without sensitive data)
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyListResponse {
    pub id: i32,
    pub name: String,
    pub api_key_preview: String, // Only show first 8 chars + "..."
    pub permissions: Vec<String>,
    pub rate_limit_tier: RateLimitTier,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Query parameters for API key listing
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyQuery {
    pub user_id: Option<i32>,
    pub is_active: Option<bool>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// API key statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyStats {
    pub total_keys: i64,
    pub active_keys: i64,
    pub inactive_keys: i64,
    pub expired_keys: i64,
    pub by_tier: HashMap<String, i64>,
}

impl ApiKey {
    /// Create a new API key
    pub async fn create(
        pool: &PgPool,
        user_id: i32,
        request: CreateApiKeyRequest,
    ) -> Result<ApiKeyResponse, ApiError> {
        let api_key = generate_api_key();
        let permissions = request.permissions.unwrap_or_default();
        let permissions_json = serde_json::to_value(&permissions)?;

        let rate_limit_tier = request.rate_limit_tier.unwrap_or_default();

        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (api_key, name, user_id, permissions, rate_limit_tier, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, api_key, name, user_id, permissions, rate_limit_tier, is_active,
                      expires_at, last_used_at, created_at, updated_at
            "#,
        )
        .bind(&api_key)
        .bind(&request.name)
        .bind(user_id)
        .bind(&permissions_json)
        .bind(rate_limit_tier.to_string())
        .bind(request.expires_at)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Failed to create API key: {}", e);
            match e {
                sqlx::Error::Database(db_err) if db_err.constraint().is_some() => {
                    ApiError::Conflict
                }
                _ => ApiError::DatabaseError("Failed to create API key".to_string()),
            }
        })?;

        let permissions: Vec<String> =
            serde_json::from_value(row.get("permissions")).unwrap_or_default();
        let rate_limit_tier_str: String = row.get("rate_limit_tier");
        let rate_limit_tier = rate_limit_tier_str.parse().unwrap_or_default();

        Ok(ApiKeyResponse {
            id: row.get("id"),
            api_key: row.get("api_key"),
            name: row.get("name"),
            user_id: row.get("user_id"),
            permissions,
            rate_limit_tier,
            is_active: row.get("is_active"),
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Find API key by key value
    pub async fn find_by_key(pool: &PgPool, api_key: &str) -> Result<Option<ApiKey>, ApiError> {
        let result = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, api_key, name, user_id, permissions,
                   rate_limit_tier, is_active, expires_at, last_used_at,
                   created_at, updated_at
            FROM api_keys
            WHERE api_key = $1
            "#,
        )
        .bind(api_key)
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// Find API key by ID
    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<ApiKey>, ApiError> {
        let result = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, api_key, name, user_id, permissions,
                   rate_limit_tier, is_active, expires_at, last_used_at,
                   created_at, updated_at
            FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(result)
    }

    /// Get user's API keys
    pub async fn get_user_keys(
        pool: &PgPool,
        user_id: i32,
        query: ApiKeyQuery,
    ) -> Result<Vec<ApiKeyListResponse>, ApiError> {
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut sql = String::from(
            r#"
            SELECT id, api_key, name, user_id, permissions, rate_limit_tier,
                   is_active, expires_at, last_used_at, created_at, updated_at
            FROM api_keys
            WHERE user_id = $1
            "#,
        );

        let mut bind_count = 1;
        let mut params: Vec<String> = vec![user_id.to_string()];

        if let Some(is_active) = query.is_active {
            bind_count += 1;
            sql.push_str(&format!(" AND is_active = ${}", bind_count));
            params.push(is_active.to_string());
        }

        if let Some(tier) = query.rate_limit_tier {
            bind_count += 1;
            sql.push_str(&format!(" AND rate_limit_tier = ${}", bind_count));
            params.push(tier.to_string());
        }

        sql.push_str(" ORDER BY created_at DESC");
        sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        let mut query_builder = sqlx::query_as::<_, ApiKey>(&sql);

        // Bind parameters
        for param in params {
            query_builder = query_builder.bind(param);
        }

        let keys = query_builder.fetch_all(pool).await?;

        let response = keys
            .into_iter()
            .map(|key| {
                let permissions: Vec<String> =
                    serde_json::from_value(key.permissions).unwrap_or_default();
                let rate_limit_tier = key.rate_limit_tier.parse().unwrap_or_default();

                ApiKeyListResponse {
                    id: key.id,
                    name: key.name,
                    api_key_preview: format!("{}...", &key.api_key[..8.min(key.api_key.len())]),
                    permissions,
                    rate_limit_tier,
                    is_active: key.is_active,
                    expires_at: key.expires_at,
                    last_used_at: key.last_used_at,
                    created_at: key.created_at,
                    updated_at: key.updated_at,
                }
            })
            .collect();

        Ok(response)
    }

    /// Update API key
    pub async fn update(
        pool: &PgPool,
        id: i32,
        request: UpdateApiKeyRequest,
    ) -> Result<ApiKeyResponse, ApiError> {
        let row = sqlx::query(
            r#"
            UPDATE api_keys
            SET name = COALESCE($1, name),
                permissions = COALESCE($2, permissions),
                rate_limit_tier = COALESCE($3, rate_limit_tier),
                is_active = COALESCE($4, is_active),
                expires_at = COALESCE($5, expires_at),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $6
            RETURNING id, api_key, name, user_id, permissions, rate_limit_tier, is_active,
                      expires_at, last_used_at, created_at, updated_at
            "#,
        )
        .bind(&request.name)
        .bind(
            request
                .permissions
                .as_ref()
                .map(|p| serde_json::to_value(p).unwrap()),
        )
        .bind(request.rate_limit_tier.as_ref().map(|t| t.to_string()))
        .bind(request.is_active)
        .bind(request.expires_at)
        .bind(id)
        .fetch_one(pool)
        .await?;

        let permissions: Vec<String> =
            serde_json::from_value(row.get("permissions")).unwrap_or_default();
        let rate_limit_tier_str: String = row.get("rate_limit_tier");
        let rate_limit_tier = rate_limit_tier_str.parse().unwrap_or_default();

        Ok(ApiKeyResponse {
            id: row.get("id"),
            api_key: row.get("api_key"),
            name: row.get("name"),
            user_id: row.get("user_id"),
            permissions,
            rate_limit_tier,
            is_active: row.get("is_active"),
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Delete API key
    pub async fn delete(pool: &PgPool, id: i32) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Update last used timestamp
    pub async fn update_last_used(pool: &PgPool, api_key: &str) -> Result<(), ApiError> {
        sqlx::query("UPDATE api_keys SET last_used_at = CURRENT_TIMESTAMP WHERE api_key = $1")
            .bind(api_key)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Get API key statistics
    pub async fn get_stats(pool: &PgPool) -> Result<ApiKeyStats, ApiError> {
        let stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_keys,
                COUNT(CASE WHEN is_active = true THEN 1 END) as active_keys,
                COUNT(CASE WHEN is_active = false THEN 1 END) as inactive_keys,
                COUNT(CASE WHEN expires_at < CURRENT_TIMESTAMP THEN 1 END) as expired_keys
            FROM api_keys
            "#,
        )
        .fetch_one(pool)
        .await?;

        let tier_stats = sqlx::query(
            r#"
            SELECT rate_limit_tier, COUNT(*) as count
            FROM api_keys
            GROUP BY rate_limit_tier
            "#,
        )
        .fetch_all(pool)
        .await?;

        let mut by_tier = HashMap::new();
        for row in tier_stats {
            let tier: String = row.get("rate_limit_tier");
            let count: i64 = row.get("count");
            by_tier.insert(tier, count);
        }

        Ok(ApiKeyStats {
            total_keys: stats.get("total_keys"),
            active_keys: stats.get("active_keys"),
            inactive_keys: stats.get("inactive_keys"),
            expired_keys: stats.get("expired_keys"),
            by_tier,
        })
    }

    /// Check if API key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// Check if API key is valid (active and not expired)
    pub fn is_valid(&self) -> bool {
        self.is_active && !self.is_expired()
    }

    /// Get permissions as Vec<String>
    pub fn get_permissions(&self) -> Vec<String> {
        serde_json::from_value(self.permissions.clone()).unwrap_or_default()
    }

    /// Get rate limit tier
    pub fn get_rate_limit_tier(&self) -> RateLimitTier {
        self.rate_limit_tier.parse().unwrap_or_default()
    }
}

/// Generate a secure API key
fn generate_api_key() -> String {
    format!("ak_{}", Uuid::new_v4().to_string().replace('-', ""))
}

/// Validate API key format
pub fn is_valid_api_key_format(api_key: &str) -> bool {
    api_key.starts_with("ak_") && api_key.len() == 35
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key() {
        let key = generate_api_key();
        assert!(key.starts_with("ak_"));
        assert_eq!(key.len(), 35);
    }

    #[test]
    fn test_is_valid_api_key_format() {
        assert!(is_valid_api_key_format(
            "ak_12345678901234567890123456789012"
        ));
        assert!(!is_valid_api_key_format("invalid_key"));
        assert!(!is_valid_api_key_format("ak_short"));
        assert!(!is_valid_api_key_format(
            "12345678901234567890123456789012345"
        ));
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
    }

    #[test]
    fn test_api_key_expired() {
        let mut api_key = ApiKey {
            id: 1,
            api_key: "ak_test".to_string(),
            name: "Test Key".to_string(),
            user_id: 1,
            permissions: serde_json::json!([]),
            rate_limit_tier: "basic".to_string(),
            is_active: true,
            expires_at: Some(Utc::now() - chrono::Duration::days(1)), // Expired
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(api_key.is_expired());
        assert!(!api_key.is_valid());

        // Test not expired
        api_key.expires_at = Some(Utc::now() + chrono::Duration::days(1));
        assert!(!api_key.is_expired());
        assert!(api_key.is_valid());

        // Test no expiration
        api_key.expires_at = None;
        assert!(!api_key.is_expired());
        assert!(api_key.is_valid());
    }
}
