//! DeLong Protocol Core Service
//!
//! The core service provides fundamental functionality for the DeLong Protocol,
//! including API key management, authentication, and core business logic.

use tracing::{error, info, instrument};

// Import from lib crate
use core::{create_app, init, shutdown};

// Import common server utilities
use common::{load_env_file, start_server};

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env.core file (if it exists)
    load_env_file("core")?;

    // Initialize the application (this handles logging setup, config loading, etc.)
    let state = init().await?;

    info!("Core service initialized successfully");

    // Create the application router
    let app = create_app(state.clone());
    info!("Router created successfully");

    // Get server address from configuration
    let addr = state.config.server.socket_addr()?;
    info!("Core service will listen on {}", addr);

    // Set up graceful shutdown handler
    let shutdown_state = state.clone();
    let shutdown_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");

        info!("Shutdown signal received, performing graceful shutdown...");

        if let Err(e) = shutdown(shutdown_state).await {
            error!("Error during shutdown: {}", e);
        }
    });

    // Start the server
    tokio::select! {
        result = start_server(app, addr, "DeLong Core") => {
            result?;
        }
        _ = shutdown_handle => {
            info!("Shutdown completed");
        }
    }

    Ok(())
}
