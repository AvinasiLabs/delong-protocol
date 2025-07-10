use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info};

use secure::config::SecureConfig;
use secure::create_router;
use secure::services::blockchain_sync::BlockchainSyncService;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if it exists
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Starting DeLong Protocol Secure Service");

    // Load configuration
    let config = SecureConfig::from_env()
        .expect("Failed to load configuration");

    // Initialize database connection
    let db_pool = PgPool::connect(&config.database.url)
        .await
        .expect("Failed to connect to database");
    
    info!("Database connection established");

    // Initialize blockchain sync service
    let blockchain_sync_service = Arc::new(BlockchainSyncService::new().await?);
    
    // Start blockchain sync in background
    blockchain_sync_service.start().await?;
    info!("Blockchain sync service started");

    // Create routes
    let app = create_router(&config, db_pool, blockchain_sync_service.clone())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
        );

    // Start server
    let listen_addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Starting server on {}", listen_addr);
    
    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .expect("Failed to bind to address");

    // Run the server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server failed to start");

    // Stop blockchain sync service
    blockchain_sync_service.stop().await?;
    
    info!("Secure service shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
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
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Signal received, starting graceful shutdown");
}
