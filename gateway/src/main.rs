//! Delong Protocol Gateway
//!
//! A simplified gateway service for the Delong privacy-preserving computation platform.
//! This gateway handles API requests for biomedical data processing, algorithm execution,
//! and API key management.

use axum::serve;
use std::net::SocketAddr;
use tokio::net::TcpListener;

use tracing::{error, info, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// OpenTelemetry imports
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    trace::{self as sdktrace, SdkTracerProvider},
};

// Import from lib crate
use gateway::{config::GatewayConfig, create_router, utils::http_client::HttpBackendClient};

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration
    let config = GatewayConfig::from_env().unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {}", e);
        eprintln!("Using default configuration");
        GatewayConfig::default()
    });

    // Initialize logging
    init_logging(&config)?;

    info!("Configuration loaded: {:?}", config);

    // Create HTTP client for backend communication
    let backend_client = HttpBackendClient::new(config.server.timeout_seconds);
    info!(
        "HTTP client initialized with timeout: {}s",
        backend_client.timeout_seconds()
    );

    // Create the application router
    let app = create_router(&config, backend_client);
    info!("Router created");

    // Get server address
    let addr = config.socket_addr()?;
    info!("Gateway will listen on {}", addr);

    // Start the server
    start_server(app, addr).await?;

    Ok(())
}

/// Initialize logging and OpenTelemetry tracing based on configuration
fn init_logging(config: &GatewayConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(&format!("gateway={}", config.logging.level))
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
    if config.opentelemetry.enabled {
        info!(
            "Initializing OpenTelemetry with endpoint: {}",
            config.opentelemetry.otlp_endpoint
        );

        let tracer = init_opentelemetry_tracer(config)?;
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        if config.logging.json_format {
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
            config.opentelemetry.service_name
        );
    } else {
        // Standard logging without OpenTelemetry
        if config.logging.json_format {
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

    info!("Logging initialized with level: {}", config.logging.level);
    Ok(())
}

/// Initialize OpenTelemetry tracer
fn init_opentelemetry_tracer(
    config: &GatewayConfig,
) -> Result<sdktrace::Tracer, Box<dyn std::error::Error>> {
    let resource = Resource::builder()
        .with_service_name(config.opentelemetry.service_name.clone())
        .build();

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.opentelemetry.otlp_endpoint)
        .build()?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    global::set_tracer_provider(tracer_provider.clone());

    Ok(tracer_provider.tracer("delong-gateway"))
}

/// Start the HTTP server with graceful shutdown
#[instrument(skip(app))]
async fn start_server(
    app: axum::Router,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server on {}", addr);

    // Create TCP listener
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        error!("Failed to bind to address {}: {}", addr, e);
        e
    })?;

    info!("Gateway listening on {}", addr);
    info!("Health check available at: http://{}/health", addr);
    info!("API endpoints available at: http://{}/api/v1/", addr);

    // Start serving requests with graceful shutdown and ConnectInfo support
    serve(
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
async fn shutdown_signal() {
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
    // Note: In OpenTelemetry 0.30, shutdown is handled automatically
    info!("Shutting down OpenTelemetry tracer provider");
}
