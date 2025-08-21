//! Rate limiting middleware
//!
//! This module provides rate limiting functionality to protect API endpoints
//! from abuse and ensure fair usage across all users.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, Response, StatusCode},
    middleware::Next,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::{AppError, middleware::AuthUser, routes::AppState};

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Window duration in seconds
    pub window_seconds: u64,
    /// Maximum requests per window for anonymous users
    pub anonymous_limit: u32,
    /// Maximum requests per window for authenticated users
    pub authenticated_limit: u32,
    /// Maximum requests per window for API key users
    pub api_key_limit: u32,
    /// Maximum requests per window for admin users
    pub admin_limit: u32,
    /// Enable rate limiting (can be disabled for testing)
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_seconds: 60,       // 1 minute window
            anonymous_limit: 10,      // 10 requests per minute for anonymous
            authenticated_limit: 100, // 100 requests per minute for authenticated
            api_key_limit: 1000,      // 1000 requests per minute for API keys
            admin_limit: 10000,       // 10000 requests per minute for admins
            enabled: true,
        }
    }
}

/// Rate limiter using Redis
#[derive(Clone)]
pub struct RedisRateLimiter {
    redis_pool: Arc<deadpool_redis::Pool>,
    config: RateLimitConfig,
    /// Fallback in-memory rate limiter
    fallback: Arc<InMemoryRateLimiter>,
}

impl RedisRateLimiter {
    /// Create a new Redis-based rate limiter
    pub fn new(redis_pool: Arc<deadpool_redis::Pool>, config: RateLimitConfig) -> Self {
        Self {
            redis_pool,
            config: config.clone(),
            fallback: Arc::new(InMemoryRateLimiter::new(config)),
        }
    }

    /// Check if a request should be rate limited
    pub async fn check_rate_limit(
        &self,
        identifier: &str,
        limit: u32,
    ) -> Result<RateLimitStatus, AppError> {
        // Try Redis first
        match self.check_redis(identifier, limit).await {
            Ok(status) => Ok(status),
            Err(e) => {
                debug!("Redis rate limit check failed, using fallback: {}", e);
                self.fallback.check_rate_limit(identifier, limit).await
            }
        }
    }

    /// Check rate limit using Redis
    async fn check_redis(&self, identifier: &str, limit: u32) -> Result<RateLimitStatus, AppError> {
        let mut conn =
            self.redis_pool.get().await.map_err(|e| {
                AppError::Internal(format!("Failed to get Redis connection: {}", e))
            })?;

        let key = format!("rate_limit:{}", identifier);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Use Redis INCR with expiration
        use deadpool_redis::redis::AsyncCommands;

        // Get current count
        let count: Option<u32> = conn.get(&key).await.ok();

        if let Some(current) = count {
            if current >= limit {
                // Get TTL for retry-after header
                let ttl: i64 = conn.ttl(&key).await.unwrap_or(0);
                return Ok(RateLimitStatus {
                    allowed: false,
                    limit,
                    remaining: 0,
                    reset_at: now + ttl as u64,
                });
            }

            // Increment counter
            let new_count: u32 = conn.incr(&key, 1).await.map_err(|e| {
                AppError::Internal(format!("Failed to increment rate limit: {}", e))
            })?;

            Ok(RateLimitStatus {
                allowed: true,
                limit,
                remaining: limit.saturating_sub(new_count),
                reset_at: now + self.config.window_seconds,
            })
        } else {
            // First request in window
            let _: () = conn
                .set_ex(&key, 1u32, self.config.window_seconds)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to set rate limit: {}", e)))?;

            Ok(RateLimitStatus {
                allowed: true,
                limit,
                remaining: limit - 1,
                reset_at: now + self.config.window_seconds,
            })
        }
    }
}

/// In-memory rate limiter (fallback when Redis is unavailable)
#[derive(Clone)]
pub struct InMemoryRateLimiter {
    buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
    config: RateLimitConfig,
}

#[derive(Debug, Clone)]
struct RateLimitBucket {
    count: u32,
    window_start: u64,
}

impl InMemoryRateLimiter {
    /// Create a new in-memory rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        let limiter = Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        // Start cleanup task
        limiter.start_cleanup_task();
        limiter
    }

    /// Check if a request should be rate limited
    pub async fn check_rate_limit(
        &self,
        identifier: &str,
        limit: u32,
    ) -> Result<RateLimitStatus, AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut buckets = self.buckets.write().await;

        let bucket = buckets
            .entry(identifier.to_string())
            .or_insert_with(|| RateLimitBucket {
                count: 0,
                window_start: now,
            });

        // Check if window has expired
        if now - bucket.window_start >= self.config.window_seconds {
            // Reset bucket
            bucket.count = 1;
            bucket.window_start = now;

            Ok(RateLimitStatus {
                allowed: true,
                limit,
                remaining: limit - 1,
                reset_at: now + self.config.window_seconds,
            })
        } else if bucket.count >= limit {
            // Rate limit exceeded
            Ok(RateLimitStatus {
                allowed: false,
                limit,
                remaining: 0,
                reset_at: bucket.window_start + self.config.window_seconds,
            })
        } else {
            // Increment and allow
            bucket.count += 1;

            Ok(RateLimitStatus {
                allowed: true,
                limit,
                remaining: limit - bucket.count,
                reset_at: bucket.window_start + self.config.window_seconds,
            })
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let buckets = self.buckets.clone();
        let window_seconds = self.config.window_seconds;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Clean every 5 minutes

            loop {
                interval.tick().await;

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let mut buckets_guard = buckets.write().await;
                let before_count = buckets_guard.len();

                buckets_guard.retain(|_, bucket| {
                    now - bucket.window_start < window_seconds * 2 // Keep for 2 windows
                });

                let after_count = buckets_guard.len();
                if before_count != after_count {
                    debug!(
                        "Rate limiter cleanup: removed {} expired buckets",
                        before_count - after_count
                    );
                }
            }
        });
    }
}

/// Rate limit status
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    /// Whether the request is allowed
    pub allowed: bool,
    /// The limit for this identifier
    pub limit: u32,
    /// Remaining requests in current window
    pub remaining: u32,
    /// Unix timestamp when the window resets
    pub reset_at: u64,
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    // Skip if rate limiting is disabled
    if !state.config.api.enable_rate_limiting {
        return Ok(next.run(request).await);
    }

    let config = RateLimitConfig::default();

    // Try to extract user from extensions (set by auth middleware)
    let auth_user = request.extensions().get::<AuthUser>().cloned();

    // Determine rate limit based on user type
    let (identifier, limit) = if let Some(user) = auth_user {
        if user.roles.contains(&"admin".to_string()) {
            (format!("user:{}", user.id), config.admin_limit)
        } else {
            (format!("user:{}", user.id), config.authenticated_limit)
        }
    } else {
        // For anonymous users, use IP address
        let ip = request
            .headers()
            .get("x-real-ip")
            .or_else(|| request.headers().get("x-forwarded-for"))
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown");

        (format!("ip:{}", ip), config.anonymous_limit)
    };

    // Check rate limit
    let limiter = if let Some(redis_pool) = state.config.redis_pool.as_ref() {
        let redis_limiter = RedisRateLimiter::new(redis_pool.clone(), config);
        redis_limiter
            .check_rate_limit(&identifier, limit)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        let memory_limiter = InMemoryRateLimiter::new(config);
        memory_limiter
            .check_rate_limit(&identifier, limit)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    if !limiter.allowed {
        warn!("Rate limit exceeded for identifier: {}", identifier);

        // Build rate limit headers
        let response = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("X-RateLimit-Limit", limiter.limit.to_string())
            .header("X-RateLimit-Remaining", "0")
            .header("X-RateLimit-Reset", limiter.reset_at.to_string())
            .header(
                "Retry-After",
                (limiter.reset_at - current_timestamp()).to_string(),
            )
            .body(Body::from(
                serde_json::json!({
                    "error": "Rate limit exceeded",
                    "message": "Too many requests. Please try again later.",
                    "retry_after": limiter.reset_at - current_timestamp()
                })
                .to_string(),
            ))
            .unwrap();

        return Ok(response);
    }

    // Add rate limit headers to response
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "X-RateLimit-Limit",
        HeaderValue::from_str(&limiter.limit.to_string()).unwrap(),
    );
    headers.insert(
        "X-RateLimit-Remaining",
        HeaderValue::from_str(&limiter.remaining.to_string()).unwrap(),
    );
    headers.insert(
        "X-RateLimit-Reset",
        HeaderValue::from_str(&limiter.reset_at.to_string()).unwrap(),
    );

    Ok(response)
}

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Rate limit layer for specific endpoints
pub async fn api_key_rate_limit(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, AppError> {
    // Extract API key from headers
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok());

    if let Some(key) = api_key {
        let config = RateLimitConfig::default();
        let identifier = format!("api_key:{}", key);

        let status = if let Some(redis_pool) = state.config.redis_pool.as_ref() {
            let limiter = RedisRateLimiter::new(redis_pool.clone(), config.clone());
            limiter
                .check_rate_limit(&identifier, config.api_key_limit)
                .await?
        } else {
            let limiter = InMemoryRateLimiter::new(config.clone());
            limiter
                .check_rate_limit(&identifier, config.api_key_limit)
                .await?
        };

        if !status.allowed {
            warn!("API key rate limit exceeded: {}", key);
            return Err(AppError::Validation(
                "API key rate limit exceeded".to_string(),
            ));
        }
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_rate_limiter() {
        let config = RateLimitConfig {
            window_seconds: 1,
            anonymous_limit: 3,
            ..Default::default()
        };

        let limiter = InMemoryRateLimiter::new(config);
        let identifier = "test_user";

        // First 3 requests should be allowed
        for i in 1..=3 {
            let status = limiter.check_rate_limit(identifier, 3).await.unwrap();
            assert!(status.allowed, "Request {} should be allowed", i);
            assert_eq!(status.remaining, 3 - i);
        }

        // 4th request should be blocked
        let status = limiter.check_rate_limit(identifier, 3).await.unwrap();
        assert!(!status.allowed, "4th request should be blocked");
        assert_eq!(status.remaining, 0);

        // Wait for window to reset
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be allowed again
        let status = limiter.check_rate_limit(identifier, 3).await.unwrap();
        assert!(
            status.allowed,
            "Request should be allowed after window reset"
        );
    }

    #[test]
    fn test_rate_limit_config_defaults() {
        let config = RateLimitConfig::default();
        assert_eq!(config.window_seconds, 60);
        assert_eq!(config.anonymous_limit, 10);
        assert_eq!(config.authenticated_limit, 100);
        assert_eq!(config.api_key_limit, 1000);
        assert_eq!(config.admin_limit, 10000);
        assert!(config.enabled);
    }
}
