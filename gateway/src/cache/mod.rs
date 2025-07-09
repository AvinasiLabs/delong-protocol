use redis::{AsyncCommands, RedisError, RedisResult, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::config::RedisConfig;
use crate::middleware::auth::AuthContext;
use common::models::{Permission, RateLimitTier};

/// Redis cache client for authentication data
#[derive(Clone)]
pub struct RedisCache {
    /// Redis connection manager
    connection_manager: Option<ConnectionManager>,
    /// Cache configuration
    config: RedisConfig,
}

/// Cached JWT token data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedJwtData {
    pub user_id: String,
    pub role: String,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
    pub expires_at: i64, // Unix timestamp
}

/// Cached API key data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedApiKeyData {
    pub user_id: String,
    pub api_key_id: String,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
    pub is_active: bool,
    pub expires_at: Option<i64>, // Unix timestamp, None means no expiration
}

/// Cache operation results
#[derive(Debug)]
pub enum CacheResult<T> {
    /// Data found in cache
    Hit(T),
    /// Data not found in cache
    Miss,
    /// Cache operation failed
    Error(String),
}

impl RedisCache {
    /// Create a new Redis cache instance
    pub async fn new(config: RedisConfig) -> Self {
        if !config.enabled {
            info!("Redis caching is disabled");
            return Self {
                connection_manager: None,
                config,
            };
        }

        match Self::create_connection_manager(&config).await {
            Ok(manager) => {
                info!(
                    redis_url = %config.url,
                    pool_size = config.pool_size,
                    "Redis cache initialized successfully"
                );
                Self {
                    connection_manager: Some(manager),
                    config,
                }
            }
            Err(err) => {
                error!(
                    error = %err,
                    redis_url = %config.url,
                    "Failed to initialize Redis cache, operating without cache"
                );
                Self {
                    connection_manager: None,
                    config,
                }
            }
        }
    }

    /// Create a new Redis cache instance with caching disabled
    pub fn new_disabled(config: RedisConfig) -> Self {
        Self {
            connection_manager: None,
            config,
        }
    }

    /// Create Redis connection manager
    async fn create_connection_manager(config: &RedisConfig) -> RedisResult<ConnectionManager> {
        let client = redis::Client::open(config.url.as_str())?;

        // Test connection with timeout
        let manager = timeout(
            Duration::from_secs(config.connection_timeout),
            ConnectionManager::new(client),
        )
        .await
        .map_err(|_| RedisError::from((redis::ErrorKind::IoError, "Connection timeout")))?;

        manager
    }

    /// Check if cache is available
    pub fn is_available(&self) -> bool {
        self.connection_manager.is_some()
    }

    /// Generate cache key for JWT token
    fn jwt_cache_key(&self, token_hash: &str) -> String {
        format!("auth:jwt:{}", token_hash)
    }

    /// Generate cache key for API key
    fn api_key_cache_key(&self, key_hash: &str) -> String {
        format!("auth:api_key:{}", key_hash)
    }

    /// Cache JWT token data
    pub async fn cache_jwt_data(&self, token_hash: &str, data: &CachedJwtData) -> CacheResult<()> {
        if let Some(manager) = &self.connection_manager {
            let key = self.jwt_cache_key(token_hash);

            match self
                .set_with_ttl(manager, &key, data, self.config.jwt_cache_ttl)
                .await
            {
                Ok(_) => {
                    debug!(
                        token_hash = token_hash,
                        user_id = %data.user_id,
                        ttl = self.config.jwt_cache_ttl,
                        "JWT data cached successfully"
                    );
                    CacheResult::Hit(())
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        token_hash = token_hash,
                        "Failed to cache JWT data"
                    );
                    CacheResult::Error(err.to_string())
                }
            }
        } else {
            CacheResult::Miss
        }
    }

    /// Get cached JWT token data
    pub async fn get_jwt_data(&self, token_hash: &str) -> CacheResult<CachedJwtData> {
        if let Some(manager) = &self.connection_manager {
            let key = self.jwt_cache_key(token_hash);

            match self.get_json::<CachedJwtData>(manager, &key).await {
                Ok(Some(data)) => {
                    debug!(
                        token_hash = token_hash,
                        user_id = %data.user_id,
                        "JWT data cache hit"
                    );
                    CacheResult::Hit(data)
                }
                Ok(None) => {
                    debug!(token_hash = token_hash, "JWT data cache miss");
                    CacheResult::Miss
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        token_hash = token_hash,
                        "Failed to get JWT data from cache"
                    );
                    CacheResult::Error(err.to_string())
                }
            }
        } else {
            CacheResult::Miss
        }
    }

    /// Cache API key data
    pub async fn cache_api_key_data(
        &self,
        key_hash: &str,
        data: &CachedApiKeyData,
    ) -> CacheResult<()> {
        if let Some(manager) = &self.connection_manager {
            let cache_key = self.api_key_cache_key(key_hash);

            match self
                .set_with_ttl(manager, &cache_key, data, self.config.api_key_cache_ttl)
                .await
            {
                Ok(_) => {
                    debug!(
                        key_hash = key_hash,
                        user_id = %data.user_id,
                        api_key_id = %data.api_key_id,
                        ttl = self.config.api_key_cache_ttl,
                        "API key data cached successfully"
                    );
                    CacheResult::Hit(())
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        key_hash = key_hash,
                        "Failed to cache API key data"
                    );
                    CacheResult::Error(err.to_string())
                }
            }
        } else {
            CacheResult::Miss
        }
    }

    /// Get cached API key data
    pub async fn get_api_key_data(&self, key_hash: &str) -> CacheResult<CachedApiKeyData> {
        if let Some(manager) = &self.connection_manager {
            let cache_key = self.api_key_cache_key(key_hash);

            match self.get_json::<CachedApiKeyData>(manager, &cache_key).await {
                Ok(Some(data)) => {
                    debug!(
                        key_hash = key_hash,
                        user_id = %data.user_id,
                        api_key_id = %data.api_key_id,
                        "API key data cache hit"
                    );
                    CacheResult::Hit(data)
                }
                Ok(None) => {
                    debug!(key_hash = key_hash, "API key data cache miss");
                    CacheResult::Miss
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        key_hash = key_hash,
                        "Failed to get API key data from cache"
                    );
                    CacheResult::Error(err.to_string())
                }
            }
        } else {
            CacheResult::Miss
        }
    }

    /// Invalidate JWT token cache
    pub async fn invalidate_jwt(&self, token_hash: &str) -> CacheResult<()> {
        if let Some(manager) = &self.connection_manager {
            let key = self.jwt_cache_key(token_hash);

            match self.delete_key(manager, &key).await {
                Ok(_) => {
                    debug!(token_hash = token_hash, "JWT cache invalidated");
                    CacheResult::Hit(())
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        token_hash = token_hash,
                        "Failed to invalidate JWT cache"
                    );
                    CacheResult::Error(err.to_string())
                }
            }
        } else {
            CacheResult::Miss
        }
    }

    /// Invalidate API key cache
    pub async fn invalidate_api_key(&self, key_hash: &str) -> CacheResult<()> {
        if let Some(manager) = &self.connection_manager {
            let cache_key = self.api_key_cache_key(key_hash);

            match self.delete_key(manager, &cache_key).await {
                Ok(_) => {
                    debug!(key_hash = key_hash, "API key cache invalidated");
                    CacheResult::Hit(())
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        key_hash = key_hash,
                        "Failed to invalidate API key cache"
                    );
                    CacheResult::Error(err.to_string())
                }
            }
        } else {
            CacheResult::Miss
        }
    }

    /// Set JSON value with TTL
    async fn set_with_ttl<T: Serialize>(
        &self,
        manager: &ConnectionManager,
        key: &str,
        value: &T,
        ttl_seconds: u64,
    ) -> RedisResult<()> {
        let json_value = serde_json::to_string(value).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::TypeError,
                "JSON serialization failed",
                e.to_string(),
            ))
        })?;

        let mut conn = manager.clone();
        conn.set_ex(key, json_value, ttl_seconds).await
    }

    /// Get JSON value
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        manager: &ConnectionManager,
        key: &str,
    ) -> RedisResult<Option<T>> {
        let mut conn = manager.clone();
        let json_value: Option<String> = conn.get(key).await?;

        match json_value {
            Some(json_str) => {
                let value = serde_json::from_str(&json_str).map_err(|e| {
                    RedisError::from((
                        redis::ErrorKind::TypeError,
                        "JSON deserialization failed",
                        e.to_string(),
                    ))
                })?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Delete key
    async fn delete_key(&self, manager: &ConnectionManager, key: &str) -> RedisResult<()> {
        let mut conn = manager.clone();
        conn.del(key).await
    }

    /// Health check for Redis connection
    pub async fn health_check(&self) -> bool {
        if let Some(manager) = &self.connection_manager {
            match timeout(Duration::from_secs(1), async {
                let mut conn = manager.clone();
                let _: Option<String> = conn.get("__health_check__").await?;
                Ok::<(), RedisError>(())
            })
            .await
            {
                Ok(Ok(_)) => {
                    debug!("Redis health check passed");
                    true
                }
                Ok(Err(err)) => {
                    warn!(error = %err, "Redis health check failed");
                    false
                }
                Err(_) => {
                    warn!("Redis health check timed out");
                    false
                }
            }
        } else {
            false
        }
    }
}

impl From<CachedJwtData> for AuthContext {
    fn from(cached_data: CachedJwtData) -> Self {
        AuthContext::JwtUser {
            user_id: cached_data.user_id,
            role: cached_data.role,
            permissions: cached_data.permissions,
            rate_limit_tier: cached_data.rate_limit_tier,
        }
    }
}

impl From<CachedApiKeyData> for AuthContext {
    fn from(cached_data: CachedApiKeyData) -> Self {
        AuthContext::ApiKeyClient {
            user_id: cached_data.user_id,
            api_key_id: cached_data.api_key_id,
            permissions: cached_data.permissions,
            rate_limit_tier: cached_data.rate_limit_tier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        let cache = RedisCache {
            connection_manager: None,
            config: RedisConfig::default(),
        };

        assert_eq!(cache.jwt_cache_key("abc123"), "auth:jwt:abc123");

        assert_eq!(cache.api_key_cache_key("def456"), "auth:api_key:def456");
    }

    #[test]
    fn test_cache_disabled() {
        let mut config = RedisConfig::default();
        config.enabled = false;

        let cache = RedisCache {
            connection_manager: None,
            config,
        };

        assert!(!cache.is_available());
    }

    #[tokio::test]
    async fn test_cache_operations_without_redis() {
        let cache = RedisCache {
            connection_manager: None,
            config: RedisConfig::default(),
        };

        let jwt_data = CachedJwtData {
            user_id: "user123".to_string(),
            role: "user".to_string(),
            permissions: vec![Permission::DataRead],
            rate_limit_tier: RateLimitTier::Basic,
            expires_at: chrono::Utc::now().timestamp() + 3600,
        };

        // All operations should return Miss when Redis is not available
        assert!(matches!(
            cache.cache_jwt_data("token_hash", &jwt_data).await,
            CacheResult::Miss
        ));

        assert!(matches!(
            cache.get_jwt_data("token_hash").await,
            CacheResult::Miss
        ));

        assert!(!cache.health_check().await);
    }
}
