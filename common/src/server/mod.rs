//! Server utilities for DeLong Protocol services
//!
//! This module provides common server functionality that can be shared across
//! all services in the DeLong Protocol ecosystem, including server startup,
//! graceful shutdown, logging, and OpenTelemetry initialization.

use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::middleware::logging::LoggingConfig;

// OpenTelemetry imports
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    trace::{self as sdktrace, SdkTracerProvider},
};

/// Configuration for OpenTelemetry
#[derive(Debug, Clone)]
pub struct OpenTelemetryConfig {
    pub enabled: bool,
    pub service_name: String,
    pub otlp_endpoint: String,
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            service_name: "delong-service".to_string(),
            otlp_endpoint: "http://localhost:4317".to_string(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
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

impl ServerConfig {
    /// Get the socket address for the server
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.host, self.port).parse()
    }
}

/// Initialize logging with optional OpenTelemetry tracing
pub fn init_logging(
    service_name: &str,
    logging_config: &LoggingConfig,
    otel_config: Option<&OpenTelemetryConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(&format!("{}={}", service_name, logging_config.level))
            .add_directive("tower=off".parse().unwrap())
            .add_directive("hyper=off".parse().unwrap())
            .add_directive("h2=off".parse().unwrap())
            .add_directive("tonic=off".parse().unwrap())
            .add_directive("opentelemetry=off".parse().unwrap())
            .add_directive("axum=off".parse().unwrap())
            .add_directive("tokio=off".parse().unwrap())
            .add_directive("runtime=off".parse().unwrap())
    });

    let subscriber = tracing_subscriber::registry().with(env_filter);

    // Initialize OpenTelemetry if enabled
    if let Some(otel_config) = otel_config {
        if otel_config.enabled {
            info!(
                "Initializing OpenTelemetry with endpoint: {}",
                otel_config.otlp_endpoint
            );

            let tracer = init_opentelemetry_tracer(otel_config)?;
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

            if logging_config.json_format {
                // JSON formatted logs for production with OpenTelemetry
                subscriber
                    .with(otel_layer)
                    .with(tracing_subscriber::fmt::layer().json())
                    .init();
            } else {
                // Human-readable logs for development with OpenTelemetry
                subscriber
                    .with(otel_layer)
                    .with(tracing_subscriber::fmt::layer().pretty())
                    .init();
            }

            info!(
                "OpenTelemetry tracing initialized for service: {}",
                otel_config.service_name
            );
        } else {
            // Standard logging without OpenTelemetry
            if logging_config.json_format {
                // JSON formatted logs for production
                subscriber
                    .with(tracing_subscriber::fmt::layer().json())
                    .init();
            } else {
                // Human-readable logs for development
                subscriber
                    .with(tracing_subscriber::fmt::layer().pretty())
                    .init();
            }
        }
    } else {
        // Standard logging without OpenTelemetry
        if logging_config.json_format {
            // JSON formatted logs for production
            subscriber
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        } else {
            // Human-readable logs for development
            subscriber
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    }

    info!(
        "Logging initialized for service: {} with level: {}",
        service_name, logging_config.level
    );
    Ok(())
}

/// Initialize OpenTelemetry tracer
fn init_opentelemetry_tracer(
    config: &OpenTelemetryConfig,
) -> Result<sdktrace::Tracer, Box<dyn std::error::Error>> {
    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .build();

    let exporter = SpanExporter::builder()
        .with_http()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    global::set_tracer_provider(tracer_provider.clone());

    Ok(tracer_provider.tracer(config.service_name.clone()))
}

/// Start the HTTP server with graceful shutdown
#[instrument(skip(app))]
pub async fn start_server(
    app: Router,
    addr: SocketAddr,
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server for {} on {}", service_name, addr);

    // Create TCP listener
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        error!("Failed to bind to address {}: {}", addr, e);
        e
    })?;

    info!("{} listening on {}", service_name, addr);
    info!("Health check available at: http://{}/health", addr);
    info!("API endpoints available at: http://{}/api/", addr);

    // Start serving requests with graceful shutdown and ConnectInfo support
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| {
        error!("Server error: {}", e);
        e.into()
    })
}

/// Graceful shutdown signal handler
pub async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal, shutting down gracefully");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down gracefully");
        },
    }

    // Shutdown OpenTelemetry to flush remaining spans
    info!("Shutting down OpenTelemetry tracer provider");
    // TODO: Fix shutdown method for current opentelemetry version
    // global::shutdown_tracer_provider();
}

/// Load environment variables from service-specific .env file
pub fn load_env_file(service_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let env_file = format!(".env.{}", service_name);

    match dotenvy::from_filename(&env_file) {
        Ok(path) => {
            println!("\n📄 Loaded {} file from: {}\n", env_file, path.display());
            Ok(())
        }
        Err(_) => {
            println!(
                "\n📄 No {} file found, using system environment variables and defaults\n",
                env_file
            );
            Ok(())
        }
    }
}

/// Parse socket address from host and port
pub fn parse_socket_addr(host: &str, port: u16) -> Result<SocketAddr, std::net::AddrParseError> {
    format!("{}:{}", host, port).parse()
}

/// Get environment variable as string with default
pub fn get_env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get environment variable as u16 with default
pub fn get_env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Get environment variable as u64 with default
pub fn get_env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Get environment variable as bool with default
pub fn get_env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_socket_addr() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout_seconds: 30,
        };

        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_parse_socket_addr() {
        let addr = parse_socket_addr("localhost", 3000).unwrap();
        assert_eq!(addr.port(), 3000);
    }

    #[test]
    fn test_get_env_string() {
        let result = get_env_string("NONEXISTENT_VAR", "default_value");
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_get_env_u16() {
        let result = get_env_u16("NONEXISTENT_VAR", 8080);
        assert_eq!(result, 8080);
    }

    #[test]
    fn test_get_env_bool() {
        let result = get_env_bool("NONEXISTENT_VAR", true);
        assert_eq!(result, true);
    }

    #[test]
    fn test_opentelemetry_config_default() {
        let config = OpenTelemetryConfig::default();
        assert_eq!(config.enabled, false);
        assert_eq!(config.service_name, "delong-service");
        assert_eq!(config.otlp_endpoint, "http://localhost:4317");
    }
}
