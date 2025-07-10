//! # Secure Service Crate
//!
//! This crate defines the core application logic for the secure service, which runs within
//! a Trusted Execution Environment (TEE). It follows the `lib.rs` pattern where the library
//! contains all business logic, and `main.rs` is a minimal binary entry point.

pub mod app_state;
pub mod auth;
pub mod config;
pub mod contracts;
pub mod handlers;
pub mod models;
pub mod routes;
pub mod services;
pub mod utils;

use crate::app_state::AppState;
use crate::config::SecureConfig;
use crate::routes::create_router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Runs the secure service application.
///
/// This function initializes the application state, creates the router, and starts the server.
pub async fn run_app() -> anyhow::Result<()> {
    // Load configuration
    let config = SecureConfig::from_env()?;

    // Initialize application state
    let app_state = match AppState::new(config.clone()).await {
        Ok(state) => Arc::new(state),
        Err(e) => {
            error!("Failed to initialize app state: {}", e);
            // Use panic here to ensure the application does not start in a broken state
            panic!("Critical application state initialization failed: {}", e);
        }
    };

    // Create the main router
    let app = create_router(app_state);

    // Get server address
    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr: SocketAddr = addr_str.parse().expect("Invalid address format");

    // Start the server
    info!("Secure service listening on {}", addr);
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
} 