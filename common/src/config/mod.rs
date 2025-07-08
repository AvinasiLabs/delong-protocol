//! Configuration management utilities for DeLong Protocol
//!
//! This module provides unified configuration management across all services,
//! including environment variable loading, logging configuration, and more.

pub mod env_loader;

pub use env_loader::EnvLoader;

use serde::{Deserialize, Serialize};

/// Logging configuration shared across all services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Use JSON format for structured logging
    pub json_format: bool,
    /// Log health check requests
    pub log_health_checks: bool,
    /// Log request headers
    pub log_headers: bool,
    /// Log request body
    pub log_request_body: bool,
    /// Maximum body size to log (in bytes)
    pub max_body_log_size: usize,
    /// Log response body
    pub log_response_body: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            log_health_checks: false,
            log_headers: false,
            log_request_body: false,
            max_body_log_size: 1024,
            log_response_body: false,
        }
    }
}

impl LoggingConfig {
    /// Load logging configuration from environment variables using EnvLoader
    pub fn from_env(loader: &EnvLoader) -> Self {
        let mut config = Self::default();

        config.level = loader.get_string_or_default("LOG_LEVEL", &config.level);
        config.json_format = loader.get_bool_or_default("LOG_JSON_FORMAT", config.json_format);
        config.log_health_checks =
            loader.get_bool_or_default("LOG_HEALTH_CHECKS", config.log_health_checks);
        config.log_headers = loader.get_bool_or_default("LOG_REQUEST_HEADERS", config.log_headers);
        config.log_request_body =
            loader.get_bool_or_default("LOG_REQUEST_BODY", config.log_request_body);
        config.max_body_log_size = loader
            .get_u32_or_default("LOG_MAX_BODY_SIZE", config.max_body_log_size as u32)
            as usize;
        config.log_response_body =
            loader.get_bool_or_default("LOG_RESPONSE_BODY", config.log_response_body);

        config
    }
}

/// Database configuration shared across services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Idle timeout in seconds
    pub idle_timeout: u64,
    /// Maximum lifetime of a connection in seconds
    pub max_lifetime: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost:5432/delong_protocol".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: 30,
            idle_timeout: 600,
            max_lifetime: 1800,
        }
    }
}

impl DatabaseConfig {
    /// Load database configuration from environment variables using EnvLoader
    pub fn from_env(loader: &EnvLoader) -> Self {
        let mut config = Self::default();

        config.url = loader.get_string_or_default("DATABASE_URL", &config.url);
        config.max_connections =
            loader.get_u32_or_default("DATABASE_MAX_CONNECTIONS", config.max_connections);
        config.min_connections =
            loader.get_u32_or_default("DATABASE_MIN_CONNECTIONS", config.min_connections);
        config.connect_timeout =
            loader.get_u64_or_default("DATABASE_CONNECT_TIMEOUT", config.connect_timeout);
        config.idle_timeout =
            loader.get_u64_or_default("DATABASE_IDLE_TIMEOUT", config.idle_timeout);
        config.max_lifetime =
            loader.get_u64_or_default("DATABASE_MAX_LIFETIME", config.max_lifetime);

        config
    }
}

/// Redis configuration shared across services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    /// Enable Redis caching
    pub enabled: bool,
    /// Connection pool size
    pub pool_size: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Default TTL for cache entries in seconds
    pub default_ttl: u64,
    /// JWT cache TTL in seconds
    pub jwt_cache_ttl: u64,
    /// API key cache TTL in seconds
    pub api_key_cache_ttl: u64,
    /// Key prefix for this service
    pub key_prefix: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            enabled: true,
            pool_size: 10,
            connection_timeout: 5,
            default_ttl: 3600,
            jwt_cache_ttl: 1800,     // 30 minutes
            api_key_cache_ttl: 3600, // 1 hour
            key_prefix: "delong_".to_string(),
        }
    }
}

impl RedisConfig {
    /// Load Redis configuration from environment variables using EnvLoader
    pub fn from_env(loader: &EnvLoader) -> Self {
        let mut config = Self::default();

        config.url = loader.get_string_or_default("REDIS_URL", &config.url);
        config.enabled = loader.get_bool_or_default("REDIS_ENABLED", config.enabled);
        config.pool_size = loader.get_u32_or_default("REDIS_POOL_SIZE", config.pool_size);
        config.connection_timeout =
            loader.get_u64_or_default("REDIS_CONNECTION_TIMEOUT", config.connection_timeout);
        config.default_ttl = loader.get_u64_or_default("REDIS_DEFAULT_TTL", config.default_ttl);
        config.jwt_cache_ttl =
            loader.get_u64_or_default("REDIS_JWT_CACHE_TTL", config.jwt_cache_ttl);
        config.api_key_cache_ttl =
            loader.get_u64_or_default("REDIS_API_KEY_CACHE_TTL", config.api_key_cache_ttl);

        if let Some(prefix) = loader.get_string("REDIS_KEY_PREFIX") {
            config.key_prefix = prefix;
        }

        config
    }
}

/// JWT configuration shared across services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// JWT secret key
    pub secret: String,
    /// Access token expiration time in seconds
    pub access_token_expiration: u64,
    /// Refresh token expiration time in seconds
    pub refresh_token_expiration: u64,
    /// JWT issuer
    pub issuer: String,
    /// JWT audience
    pub audience: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".to_string(),
            access_token_expiration: 86400,   // 24 hours
            refresh_token_expiration: 604800, // 7 days
            issuer: "delong-protocol".to_string(),
            audience: "delong-users".to_string(),
        }
    }
}

impl JwtConfig {
    /// Load JWT configuration from environment variables using EnvLoader
    pub fn from_env(loader: &EnvLoader) -> Self {
        let mut config = Self::default();

        config.secret = loader.get_string_or_default("JWT_SECRET", &config.secret);
        config.access_token_expiration = loader.get_u64_or_default(
            "JWT_ACCESS_TOKEN_EXPIRATION",
            config.access_token_expiration,
        );
        config.refresh_token_expiration = loader.get_u64_or_default(
            "JWT_REFRESH_TOKEN_EXPIRATION",
            config.refresh_token_expiration,
        );
        config.issuer = loader.get_string_or_default("JWT_ISSUER", &config.issuer);
        config.audience = loader.get_string_or_default("JWT_AUDIENCE", &config.audience);

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_logging_config_from_env() {
        let loader = EnvLoader::new("test");

        unsafe {
            env::set_var("TEST_LOG_LEVEL", "debug");
            env::set_var("TEST_LOG_JSON_FORMAT", "true");
        }

        let config = LoggingConfig::from_env(&loader);

        assert_eq!(config.level, "debug");
        assert_eq!(config.json_format, true);

        unsafe {
            env::remove_var("TEST_LOG_LEVEL");
            env::remove_var("TEST_LOG_JSON_FORMAT");
        }
    }

    #[test]
    #[serial]
    fn test_database_config_from_env() {
        let loader = EnvLoader::new("test");

        unsafe {
            env::set_var(
                "TEST_DATABASE_URL",
                "postgresql://test:test@localhost:5432/test",
            );
            env::set_var("TEST_DATABASE_MAX_CONNECTIONS", "50");
        }

        let config = DatabaseConfig::from_env(&loader);

        assert_eq!(config.url, "postgresql://test:test@localhost:5432/test");
        assert_eq!(config.max_connections, 50);

        unsafe {
            env::remove_var("TEST_DATABASE_URL");
            env::remove_var("TEST_DATABASE_MAX_CONNECTIONS");
        }
    }

    #[test]
    #[serial]
    fn test_redis_config_from_env() {
        let loader = EnvLoader::new("test");

        unsafe {
            env::set_var("TEST_REDIS_URL", "redis://localhost:6380");
            env::set_var("TEST_REDIS_ENABLED", "false");
            env::set_var("TEST_REDIS_JWT_CACHE_TTL", "900");
            env::set_var("TEST_REDIS_API_KEY_CACHE_TTL", "1800");
        }

        let config = RedisConfig::from_env(&loader);

        assert_eq!(config.url, "redis://localhost:6380");
        assert_eq!(config.enabled, false);
        assert_eq!(config.jwt_cache_ttl, 900);
        assert_eq!(config.api_key_cache_ttl, 1800);

        unsafe {
            env::remove_var("TEST_REDIS_URL");
            env::remove_var("TEST_REDIS_ENABLED");
            env::remove_var("TEST_REDIS_JWT_CACHE_TTL");
            env::remove_var("TEST_REDIS_API_KEY_CACHE_TTL");
        }
    }

    #[test]
    #[serial]
    fn test_jwt_config_from_env() {
        let loader = EnvLoader::new("test");

        unsafe {
            env::set_var("TEST_JWT_SECRET", "test-secret");
            env::set_var("TEST_JWT_ACCESS_TOKEN_EXPIRATION", "3600");
        }

        let config = JwtConfig::from_env(&loader);

        assert_eq!(config.secret, "test-secret");
        assert_eq!(config.access_token_expiration, 3600);

        unsafe {
            env::remove_var("TEST_JWT_SECRET");
            env::remove_var("TEST_JWT_ACCESS_TOKEN_EXPIRATION");
        }
    }
}
