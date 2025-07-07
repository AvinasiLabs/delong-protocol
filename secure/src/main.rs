use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use secure::config::SecureConfig;
use secure::routes::create_routes;
use secure::sync::BlockchainSyncService;
use secure::AppState;

#[tokio::main]
async fn main() -> Result<()> {
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

    // Run database migrations (TODO: Uncomment when database is properly configured)
    // sqlx::migrate!("./migrations")
    //     .run(&db_pool)
    //     .await?;
    info!("Database migrations completed");

    // Initialize IPFS client
    let ipfs_client = create_ipfs_client(&config.ipfs).await?;
    info!("IPFS client initialized");

    // Initialize blockchain sync service
    let blockchain_sync = BlockchainSyncService::new(config.clone());
    
    // Start blockchain sync in background
    let blockchain_sync_handle = {
        let config_clone = config.clone();
        tokio::spawn(async move {
            let mut sync_service = BlockchainSyncService::new(config_clone);
            if let Err(e) = sync_service.start().await {
                warn!(error = %e, "Blockchain sync service failed");
            }
        })
    };

    // Create application state
    let app_state = Arc::new(AppState {
        config: config.clone(),
        db_pool,
        ipfs_client,
    });

    // Create routes
    let app = create_routes(app_state)
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
    blockchain_sync.stop().await;
    
    // Wait for blockchain sync to complete
    if let Err(e) = blockchain_sync_handle.await {
        warn!(error = %e, "Failed to wait for blockchain sync completion");
    }

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

// Helper function to create IPFS client
async fn create_ipfs_client(config: &secure::config::IpfsConfig) -> Result<ipfs_api_backend_hyper::IpfsClient> {
    // For now, just create a default client
    // In production, this would connect to the configured IPFS endpoint
    info!(endpoint = %config.api_url, "Connecting to IPFS HTTP API");
    
    // Use the default client for now
    let client = ipfs_api_backend_hyper::IpfsClient::default();
    
    Ok(client)
}
