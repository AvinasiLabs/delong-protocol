//! DeLong Protocol Secure Service
//!
//! The secure service provides TEE-enabled functionality for the DeLong Protocol,
//! including private data handling and secure algorithm execution.

use tracing::{error, info};
use secure::{create_app, init, shutdown};
use common::server::start_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // init() in lib.rs handles env loading, config, and logging
    let state = init().await?;
    info!("Secure service initialized successfully");
    
    // Create the application router from the library
    let app = create_app(state.clone());
    info!("Router created successfully");

    // Get server address from configuration
    let addr = state.config.server.socket_addr()?;
    info!("Secure service will listen on {}", addr);

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

    // Start the server using the common utility
    tokio::select! {
        result = start_server(app, addr, "DeLong Secure") => {
            if let Err(e) = result {
                error!("Server failed: {}", e);
            }
        }
        _ = shutdown_handle => {
            info!("Shutdown completed");
        }
    }

    Ok(())
}
