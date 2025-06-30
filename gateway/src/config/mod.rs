use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// Import LoggingConfig from common crate
pub use common::LoggingConfig;

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Server listening address
    pub server: ServerConfig,
    /// API configuration
    pub api: ApiConfig,
    /// Backend services configuration
    pub services: ServicesConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// OpenTelemetry configuration
    pub opentelemetry: OpenTelemetryConfig,
    /// Redis configuration for caching
    pub redis: RedisConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Enable API key validation
    pub enable_api_key_validation: bool,
}

/// Backend services configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesConfig {
    /// Core service base URL
    pub core_url: String,
    /// Secure service base URL (for static datasets and TEE operations)
    pub secure_url: String,
}

/// OpenTelemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// Enable OpenTelemetry tracing
    pub enabled: bool,
    /// OTLP exporter endpoint
    pub otlp_endpoint: String,
    /// Service name for tracing
    pub service_name: String,
    /// Service version
    pub service_version: String,
}

/// Redis configuration for caching
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
    /// Cache TTL for JWT tokens in seconds
    pub jwt_cache_ttl: u64,
    /// Cache TTL for API keys in seconds
    pub api_key_cache_ttl: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            api: ApiConfig::default(),
            services: ServicesConfig::default(),
            logging: LoggingConfig::default(),
            opentelemetry: OpenTelemetryConfig::default(),
            redis: RedisConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            timeout_seconds: 30,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_body_size: 1024 * 1024 * 10, // 10MB
            enable_api_key_validation: true,
        }
    }
}

impl Default for ServicesConfig {
    fn default() -> Self {
        Self {
            core_url: "http://localhost:8081".to_string(),
            secure_url: "http://localhost:8082".to_string(),
        }
    }
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: "http://localhost:4317".to_string(),
            service_name: "delong-gateway".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            enabled: true,
            pool_size: 10,
            connection_timeout: 5,
            jwt_cache_ttl: 300,     // 5 minutes
            api_key_cache_ttl: 600, // 10 minutes
        }
    }
}

impl GatewayConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Self::default();

        // Server configuration
        if let Ok(host) = std::env::var("GATEWAY_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("GATEWAY_PORT") {
            config.server.port = port.parse()?;
        }
        if let Ok(timeout) = std::env::var("GATEWAY_TIMEOUT") {
            config.server.timeout_seconds = timeout.parse()?;
        }

        // Services configuration
        if let Ok(core_url) = std::env::var("CORE_SERVICE_URL") {
            config.services.core_url = core_url;
        }
        if let Ok(secure_url) = std::env::var("SECURE_SERVICE_URL") {
            config.services.secure_url = secure_url;
        }

        // API configuration
        if let Ok(max_size) = std::env::var("MAX_BODY_SIZE") {
            config.api.max_body_size = max_size.parse()?;
        }
        if let Ok(enable_api_key) = std::env::var("ENABLE_API_KEY_VALIDATION") {
            config.api.enable_api_key_validation = enable_api_key.to_lowercase() == "true";
        }

        // Logging configuration
        if let Ok(level) = std::env::var("LOG_LEVEL") {
            config.logging.level = level;
        }
        if let Ok(json_format) = std::env::var("LOG_JSON_FORMAT") {
            config.logging.json_format = json_format.to_lowercase() == "true";
        }
        if let Ok(log_health_checks) = std::env::var("LOG_HEALTH_CHECKS") {
            config.logging.log_health_checks = log_health_checks.to_lowercase() == "true";
        }
        if let Ok(log_headers) = std::env::var("LOG_REQUEST_HEADERS") {
            config.logging.log_headers = log_headers.to_lowercase() == "true";
        }
        if let Ok(log_request_body) = std::env::var("LOG_REQUEST_BODY") {
            config.logging.log_request_body = log_request_body.to_lowercase() == "true";
        }
        if let Ok(max_body_log_size) = std::env::var("LOG_MAX_BODY_SIZE") {
            config.logging.max_body_log_size = max_body_log_size.parse()?;
        }
        if let Ok(log_response_body) = std::env::var("LOG_RESPONSE_BODY") {
            config.logging.log_response_body = log_response_body.to_lowercase() == "true";
        }

        // OpenTelemetry configuration
        if let Ok(enabled) = std::env::var("OTEL_ENABLED") {
            config.opentelemetry.enabled = enabled.to_lowercase() == "true";
        }
        if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            config.opentelemetry.otlp_endpoint = endpoint;
            config.opentelemetry.enabled = true; // Auto-enable if endpoint is provided
        }
        if let Ok(service_name) = std::env::var("OTEL_SERVICE_NAME") {
            config.opentelemetry.service_name = service_name;
        }
        if let Ok(service_version) = std::env::var("OTEL_SERVICE_VERSION") {
            config.opentelemetry.service_version = service_version;
        }

        // Redis configuration
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            config.redis.url = redis_url;
        }
        if let Ok(enabled) = std::env::var("REDIS_ENABLED") {
            config.redis.enabled = enabled.to_lowercase() == "true";
        }
        if let Ok(pool_size) = std::env::var("REDIS_POOL_SIZE") {
            config.redis.pool_size = pool_size.parse()?;
        }
        if let Ok(timeout) = std::env::var("REDIS_CONNECTION_TIMEOUT") {
            config.redis.connection_timeout = timeout.parse()?;
        }
        if let Ok(ttl) = std::env::var("REDIS_JWT_CACHE_TTL") {
            config.redis.jwt_cache_ttl = ttl.parse()?;
        }
        if let Ok(ttl) = std::env::var("REDIS_API_KEY_CACHE_TTL") {
            config.redis.api_key_cache_ttl = ttl.parse()?;
        }

        Ok(config)
    }

    /// Get the server socket address
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.server.host, self.server.port).parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.services.core_url, "http://localhost:8081");
        assert_eq!(config.services.secure_url, "http://localhost:8082");
        assert!(config.api.enable_api_key_validation);
    }

    #[test]
    fn test_socket_addr() {
        let config = GatewayConfig::default();
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.to_string(), "0.0.0.0:8080");
    }
}
