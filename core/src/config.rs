//! Configuration management for DeLong Protocol Core Service
//!
//! This module handles all configuration loading and validation for the core service,
//! including server settings, logging, OpenTelemetry, and database configuration.

use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;

/// Main configuration structure for the core service
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// OpenTelemetry configuration
    pub opentelemetry: OpenTelemetryConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// API configuration
    pub api: ApiConfig,
    /// Proxy configuration for forwarding to Secure service
    pub proxy: ProxyConfig,
    /// JWT secret for authentication
    pub jwt_secret: String,
    /// Internal JWT secret for service-to-service communication
    pub internal_jwt_secret: String,
    /// Development mode flag
    pub development_mode: bool,
    /// Redis pool for caching and session storage
    #[serde(skip)]
    pub redis_pool: Option<Arc<deadpool_redis::Pool>>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Use JSON format for logs
    pub json_format: bool,
}

/// OpenTelemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// Enable OpenTelemetry tracing
    pub enabled: bool,
    /// Service name for tracing
    pub service_name: String,
    /// OTLP endpoint URL
    pub otlp_endpoint: String,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connect_timeout_seconds: u64,
    /// Query timeout in seconds
    pub query_timeout_seconds: u64,
    /// Enable automatic migrations
    pub auto_migrate: bool,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Enable API key validation
    pub enable_api_key_validation: bool,
    /// Default API key expiration in days
    pub default_key_expiration_days: u32,
    /// Maximum API keys per user
    pub max_keys_per_user: u32,
    /// Enable request rate limiting
    pub enable_rate_limiting: bool,
}

/// Proxy configuration for forwarding requests to Secure service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Secure service base URL (e.g., https://secure.phala.network)
    pub secure_service_url: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Enable request/response logging
    pub enable_debug_logging: bool,
    /// Internal JWT expiration in seconds (default: 60)
    pub jwt_expiration_seconds: u64,
    /// Retry attempts for failed requests
    pub max_retries: u32,
}

impl ProxyConfig {
    /// Load proxy configuration from environment variables
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        use std::env;

        Ok(Self {
            secure_service_url: env::var("SECURE_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            timeout_seconds: env::var("PROXY_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            enable_debug_logging: env::var("PROXY_DEBUG")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            jwt_expiration_seconds: env::var("INTERNAL_JWT_EXPIRATION")
                .unwrap_or_else(|_| "60".to_string())
                .parse()?,
            max_retries: env::var("PROXY_MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()?,
        })
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file if it exists
        dotenv().ok();

        Ok(Self {
            server: ServerConfig {
                host: env::var("SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("SERVICE_PORT")
                    .unwrap_or_else(|_| "8080".to_string())
                    .parse()?,
                timeout_seconds: env::var("REQUEST_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()?,
            },
            logging: LoggingConfig {
                level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
                json_format: env::var("LOG_JSON")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()?,
            },
            opentelemetry: OpenTelemetryConfig {
                enabled: env::var("OTEL_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()?,
                service_name: env::var("SERVICE_NAME")
                    .unwrap_or_else(|_| "delong-core".to_string()),
                otlp_endpoint: env::var("OTEL_ENDPOINT")
                    .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            },
            database: DatabaseConfig {
                url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgresql://postgres:core_password@localhost:5433/db_core".to_string()
                }),
                max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()?,
                connect_timeout_seconds: 10,
                query_timeout_seconds: 30,
                auto_migrate: env::var("AUTO_MIGRATE")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()?,
            },
            api: ApiConfig {
                enable_api_key_validation: false,
                default_key_expiration_days: 30,
                max_keys_per_user: 10,
                enable_rate_limiting: env::var("TEST_MODE").is_err()
                    && env::var("CARGO_TARGET_DIR").is_err(),
            },
            proxy: ProxyConfig {
                secure_service_url: env::var("SECURE_SERVICE_URL")
                    .unwrap_or_else(|_| "http://localhost:8081".to_string()),
                timeout_seconds: env::var("PROXY_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()?,
                enable_debug_logging: env::var("PROXY_DEBUG")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()?,
                jwt_expiration_seconds: env::var("INTERNAL_JWT_EXPIRATION")
                    .unwrap_or_else(|_| "60".to_string())
                    .parse()?,
                max_retries: env::var("PROXY_MAX_RETRIES")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()?,
            },
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-secret-key-change-in-production".to_string()),
            internal_jwt_secret: env::var("INTERNAL_JWT_SECRET")
                .unwrap_or_else(|_| "internal-secret-key-change-in-production".to_string()),
            development_mode: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
                == "development",
            redis_pool: None,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                timeout_seconds: 30,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                json_format: false,
            },
            opentelemetry: OpenTelemetryConfig {
                enabled: false,
                service_name: "delong-core".to_string(),
                otlp_endpoint: "http://localhost:4317".to_string(),
            },
            database: DatabaseConfig {
                url: "postgresql://postgres:core_password@localhost:5433/db_core".to_string(),
                max_connections: 10,
                connect_timeout_seconds: 10,
                query_timeout_seconds: 30,
                auto_migrate: false,
            },
            api: ApiConfig {
                enable_api_key_validation: false,
                default_key_expiration_days: 30,
                max_keys_per_user: 10,
                enable_rate_limiting: false,
            },
            proxy: ProxyConfig {
                secure_service_url: "http://localhost:8081".to_string(),
                timeout_seconds: 30,
                enable_debug_logging: false,
                jwt_expiration_seconds: 60,
                max_retries: 3,
            },
            jwt_secret: "test-secret-key".to_string(),
            internal_jwt_secret: "internal-test-secret-key".to_string(),
            development_mode: false,
            redis_pool: None,
        }
    }
}
