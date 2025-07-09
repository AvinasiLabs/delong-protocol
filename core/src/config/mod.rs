//! Configuration management for DeLong Protocol Core Service
//!
//! This module handles all configuration loading and validation for the core service,
//! including server settings, logging, OpenTelemetry, and database configuration.

use common::config::EnvLoader;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Main configuration structure for the core service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
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
    /// JWT secret for authentication
    pub jwt_secret: String,
    /// Database URL (for direct access)
    pub database_url: String,
    /// Development mode flag
    pub development_mode: bool,
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

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            logging: LoggingConfig::default(),
            opentelemetry: OpenTelemetryConfig::default(),
            database: DatabaseConfig::default(),
            api: ApiConfig::default(),
            jwt_secret: "delong_default_secret_change_in_production".to_string(),
            database_url: "sqlite://core.db".to_string(),
            development_mode: true,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 11112,
            timeout_seconds: 30,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
        }
    }
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            service_name: "delong-core".to_string(),
            otlp_endpoint: "http://localhost:4317".to_string(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://core.db".to_string(),
            max_connections: 10,
            connect_timeout_seconds: 10,
            query_timeout_seconds: 30,
            auto_migrate: true,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enable_api_key_validation: false,
            default_key_expiration_days: 30,
            max_keys_per_user: 10,
            enable_rate_limiting: false,
        }
    }
}

impl CoreConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("core");
        loader.load_env_files()?;

        let database_url = loader.get_string_or_default("DATABASE_URL", "sqlite://core.db");
        Ok(Self {
            server: ServerConfig::from_env()?,
            logging: LoggingConfig::from_env()?,
            opentelemetry: OpenTelemetryConfig::from_env()?,
            database: DatabaseConfig::from_env()?,
            api: ApiConfig::from_env()?,
            jwt_secret: loader
                .get_string_or_default("JWT_SECRET", "delong_default_secret_change_in_production"),
            database_url: database_url.clone(),
            development_mode: loader.get_bool_or_default("DEVELOPMENT_MODE", true),
        })
    }
}

impl ServerConfig {
    /// Load server configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("core");
        loader.load_env_files()?;

        Ok(Self {
            host: loader.get_string_or_default("HOST", "0.0.0.0"),
            port: loader.get_u16_or_default("PORT", 11112),
            timeout_seconds: loader.get_u64_or_default("TIMEOUT_SECONDS", 30),
        })
    }

    /// Get the socket address for the server
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}

impl LoggingConfig {
    /// Load logging configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("core");
        loader.load_env_files()?;

        Ok(Self {
            level: loader.get_string_or_default("LOG_LEVEL", "info"),
            json_format: loader.get_bool_or_default("LOG_JSON_FORMAT", false),
        })
    }
}

impl OpenTelemetryConfig {
    /// Load OpenTelemetry configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("core");
        loader.load_env_files()?;

        Ok(Self {
            enabled: loader.get_bool_or_default("OTEL_ENABLED", false),
            service_name: loader.get_string_or_default("OTEL_SERVICE_NAME", "delong-core"),
            otlp_endpoint: loader
                .get_string_or_default("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317"),
        })
    }
}

impl DatabaseConfig {
    /// Load database configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("core");
        loader.load_env_files()?;

        Ok(Self {
            url: loader.get_string_or_default("DATABASE_URL", "sqlite://core.db"),
            max_connections: loader.get_u32_or_default("DATABASE_MAX_CONNECTIONS", 10),
            connect_timeout_seconds: loader.get_u64_or_default("DATABASE_CONNECT_TIMEOUT", 10),
            query_timeout_seconds: loader.get_u64_or_default("DATABASE_QUERY_TIMEOUT", 30),
            auto_migrate: loader.get_bool_or_default("DATABASE_AUTO_MIGRATE", true),
        })
    }
}

impl ApiConfig {
    /// Load API configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("core");
        loader.load_env_files()?;

        Ok(Self {
            enable_api_key_validation: loader
                .get_bool_or_default("ENABLE_API_KEY_VALIDATION", false),
            default_key_expiration_days: loader
                .get_u32_or_default("DEFAULT_KEY_EXPIRATION_DAYS", 30),
            max_keys_per_user: loader.get_u32_or_default("MAX_KEYS_PER_USER", 10),
            enable_rate_limiting: loader.get_bool_or_default("ENABLE_RATE_LIMITING", false),
        })
    }
}

impl CoreConfig {
    /// Get Redis URL if configured
    pub fn redis_url(&self) -> Option<String> {
        // Use EnvLoader for consistent environment variable handling
        let loader = EnvLoader::new("core");
        let _ = loader.load_env_files();
        loader.get_string("REDIS_URL")
    }
}

/// Type alias for backward compatibility
pub type AppConfig = CoreConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_config_default() {
        let config = CoreConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 11112);
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.opentelemetry.service_name, "delong-core");
        assert_eq!(config.database.url, "sqlite://core.db");
        assert!(!config.api.enable_api_key_validation);
    }

    #[test]
    fn test_server_config_socket_addr() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 11112,
            timeout_seconds: 30,
        };

        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 11112);
    }

    #[test]
    fn test_config_from_env() {
        // This test mainly ensures the methods exist and don't panic
        let config = CoreConfig::from_env().unwrap();
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(!config.logging.level.is_empty());
        assert!(!config.opentelemetry.service_name.is_empty());
        assert!(!config.database.url.is_empty());
    }

    #[test]
    fn test_individual_config_from_env() {
        let server_config = ServerConfig::from_env().unwrap();
        assert!(!server_config.host.is_empty());

        let logging_config = LoggingConfig::from_env().unwrap();
        assert!(!logging_config.level.is_empty());

        let otel_config = OpenTelemetryConfig::from_env().unwrap();
        assert!(!otel_config.service_name.is_empty());

        let db_config = DatabaseConfig::from_env().unwrap();
        assert!(!db_config.url.is_empty());

        let api_config = ApiConfig::from_env().unwrap();
        assert!(api_config.max_keys_per_user > 0);
    }
}
