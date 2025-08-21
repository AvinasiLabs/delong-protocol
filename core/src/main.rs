//! DeLong Protocol Core Service
//!
//! The core service provides fundamental functionality for the DeLong Protocol,
//! including API key management, authentication, and core business logic.

use std::sync::Arc;
use tracing::{error, info, instrument};

// Import from lib crate
use delong_core::{create_app, init, shutdown};

// Import utilities
use tokio::net::TcpListener;

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the application (this handles logging setup, config loading, etc.)
    // Config::load() already handles loading .env file internally
    let state = init().await?;

    info!("Core service initialized successfully");

    // Create the application router
    let app = create_app(state.clone());
    info!("Router created successfully");

    // Get server address from configuration
    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    info!("Core service will listen on {}", addr);

    // Set up graceful shutdown handler
    let shutdown_state = state.clone();
    let shutdown_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");

        info!("Shutdown signal received, performing graceful shutdown...");

        if let Err(e) = shutdown(Arc::new(shutdown_state)).await {
            error!("Error during shutdown: {}", e);
        }
    });

    // Start the server
    info!("Attempting to bind to address: {}", addr);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            info!("Successfully bound to address: {}", addr);
            l
        }
        Err(e) => {
            error!("Failed to bind to address {}: {}", addr, e);
            return Err(e.into());
        }
    };
    info!("TCP listener created, starting axum server...");

    info!("Starting tokio::select! block for server and shutdown handling");

    tokio::select! {
        result = axum::serve(listener, app) => {
            info!("axum::serve completed with result: {:?}", result);
            if let Err(e) = result {
                error!("Server error: {}", e);
                return Err(e.into());
            }
        }
        _ = shutdown_handle => {
            info!("Shutdown completed");
        }
    }

    info!("Server has stopped");

    Ok(())
}
