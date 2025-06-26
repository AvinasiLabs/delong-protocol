use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Server listening address
    pub server: ServerConfig,
    /// API configuration
    pub api: ApiConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// OpenTelemetry configuration
    pub opentelemetry: OpenTelemetryConfig,
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
    /// Core service base URL
    pub core_service_url: String,
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Enable API key validation
    pub enable_api_key_validation: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    /// Enable JSON formatted logs
    pub json_format: bool,
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
            logging: LoggingConfig::default(),
            opentelemetry: OpenTelemetryConfig::default(),
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
            core_service_url: "http://localhost:8081".to_string(),
            max_body_size: 1024 * 1024 * 10, // 10MB
            enable_api_key_validation: true,
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
            otlp_endpoint: "http://localhost:4317".to_string(),
            service_name: "delong-gateway".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
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

        // API configuration
        if let Ok(core_url) = std::env::var("CORE_SERVICE_URL") {
            config.api.core_service_url = core_url;
        }
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
        assert_eq!(config.api.core_service_url, "http://localhost:8081");
        assert!(config.api.enable_api_key_validation);
    }

    #[test]
    fn test_socket_addr() {
        let config = GatewayConfig::default();
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.to_string(), "0.0.0.0:8080");
    }
}
