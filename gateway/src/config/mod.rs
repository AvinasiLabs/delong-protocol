use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// Import configuration utilities from common crate

pub use common::prelude::{EnvLoader, LoggingConfig, RedisConfig};

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
            port: 11111,
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
            core_url: "http://localhost:11112".to_string(),
            secure_url: "http://localhost:11113".to_string(),
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

impl GatewayConfig {
    /// Load configuration from environment variables using EnvLoader
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let loader = EnvLoader::new("gateway");
        loader.load_env_files()?;

        let mut config = Self::default();

        // Server configuration
        config.server.host = loader.get_string_or_default("HOST", &config.server.host);
        config.server.port = loader.get_u16_or_default("PORT", config.server.port);
        config.server.timeout_seconds =
            loader.get_u64_or_default("TIMEOUT_SECONDS", config.server.timeout_seconds);

        // Services configuration
        config.services.core_url =
            loader.get_string_or_default("CORE_SERVICE_URL", &config.services.core_url);
        config.services.secure_url =
            loader.get_string_or_default("SECURE_SERVICE_URL", &config.services.secure_url);

        // API configuration
        config.api.max_body_size =
            loader.get_u32_or_default("MAX_BODY_SIZE", config.api.max_body_size as u32) as usize;
        config.api.enable_api_key_validation = loader.get_bool_or_default(
            "ENABLE_API_KEY_VALIDATION",
            config.api.enable_api_key_validation,
        );

        // Logging configuration
        config.logging = LoggingConfig::from_env(&loader);

        // OpenTelemetry configuration
        config.opentelemetry.enabled =
            loader.get_bool_or_default("OTEL_ENABLED", config.opentelemetry.enabled);
        config.opentelemetry.otlp_endpoint = loader.get_string_or_default(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            &config.opentelemetry.otlp_endpoint,
        );
        config.opentelemetry.service_name =
            loader.get_string_or_default("OTEL_SERVICE_NAME", &config.opentelemetry.service_name);
        config.opentelemetry.service_version = loader.get_string_or_default(
            "OTEL_SERVICE_VERSION",
            &config.opentelemetry.service_version,
        );

        // Auto-enable OpenTelemetry if endpoint is provided
        if loader.has_key("OTEL_EXPORTER_OTLP_ENDPOINT") {
            config.opentelemetry.enabled = true;
        }

        // Redis configuration
        config.redis = RedisConfig::from_env(&loader);

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
        assert_eq!(config.server.port, 11111);
        assert_eq!(config.services.core_url, "http://localhost:11112");
        assert_eq!(config.services.secure_url, "http://localhost:11113");
        assert!(config.api.enable_api_key_validation);
    }

    #[test]
    fn test_socket_addr() {
        let config = GatewayConfig::default();
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.to_string(), "0.0.0.0:11111");
    }
}
