//! Delong Protocol Gateway
//!
//! A simplified gateway service for the Delong privacy-preserving computation platform.
//! This gateway handles API requests for biomedical data processing, algorithm execution,
//! and API key management.

use std::sync::Arc;
use tracing::{error, info, instrument};

// Import from lib crate
use gateway::{
    cache::RedisCache, config::GatewayConfig, create_router,
    services::http_client::HttpBackendClient,
};

// Import common server utilities
use core::{LoggingConfig, OpenTelemetryConfig, init_logging, load_env_file, start_server};

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env.gateway file (if it exists)
    load_env_file("gateway")?;

    // Initialize configuration
    let config = GatewayConfig::from_env().unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {}", e);
        eprintln!("Using default configuration");
        GatewayConfig::default()
    });

    // Initialize logging with OpenTelemetry support
    let logging_config = LoggingConfig {
        level: config.logging.level.clone(),
        json_format: config.logging.json_format,
        log_health_checks: config.logging.log_health_checks,
        log_headers: config.logging.log_headers,
        log_request_body: config.logging.log_request_body,
        max_body_log_size: config.logging.max_body_log_size,
        log_response_body: config.logging.log_response_body,
    };

    let otel_config = if config.opentelemetry.enabled {
        Some(OpenTelemetryConfig {
            enabled: config.opentelemetry.enabled,
            service_name: config.opentelemetry.service_name.clone(),
            otlp_endpoint: config.opentelemetry.otlp_endpoint.clone(),
        })
    } else {
        None
    };

    init_logging("gateway", &logging_config, otel_config.as_ref())?;

    info!("Configuration loaded: {:?}", config);

    // Initialize Redis cache
    let redis_cache = Arc::new(RedisCache::new(config.redis.clone()).await);
    if redis_cache.is_available() {
        info!("Redis cache initialized successfully");

        // Perform health check
        if redis_cache.health_check().await {
            info!("Redis health check passed");
        } else {
            error!("Redis health check failed, continuing without cache");
        }
    } else {
        info!("Redis cache not available, continuing without cache");
    }

    // Create HTTP client for backend communication
    let backend_client = HttpBackendClient::new(config.server.timeout_seconds);
    info!(
        "HTTP client initialized with timeout: {}s",
        backend_client.timeout_seconds()
    );

    // Create the application router with Redis cache
    let app = create_router(&config, backend_client, redis_cache);
    info!("Router created with Redis cache support");

    // Get server address
    let addr = config.socket_addr()?;
    info!("Gateway will listen on {}", addr);

    // Start the server
    start_server(app, addr, "DeLong Gateway").await?;

    Ok(())
}
