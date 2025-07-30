//! Secure TEE Service Entry Point
//!
//! This is the main entry point for the secure service that runs within the TEE environment.
//! It provides the core DeLong protocol functionality including secure computation,
//! data management, and blockchain synchronization.

use anyhow::Result;
use std::net::SocketAddr;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use secure::config::Config;
use secure::infra::db::Database;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "secure=debug,tower_http=debug".into()),
        )
        .init();

    info!("Starting DeLong Protocol Secure Service");

    // Load configuration from environment variables (supports .env file)
    let config = secure::config::init_config()
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
    info!("Configuration loaded successfully");

    // Initialize database
    let db = Database::new(&config.database).await?;
    info!("Database connection established");

    // Create cancellation token for graceful shutdown
    let shutdown_token = CancellationToken::new();

    // Start background tasks
    let chainsync_handle = spawn_chainsync_task(db.clone(), config.clone(), shutdown_token.clone());

    let runtime_handle = spawn_runtime_task(db.clone(), config.clone(), shutdown_token.clone());

    // Build the application
    let app = secure::routes::create_app(db.clone(), config.clone()).await;

    // Create TCP listener
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("API server listening on {}", addr);

    // Run the server
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_token.clone()))
        .await?;

    info!("Shutting down secure service...");

    // Signal background tasks to stop
    shutdown_token.cancel();

    // Wait for background tasks to complete
    let _ = tokio::join!(chainsync_handle, runtime_handle);

    info!("Secure service stopped gracefully");
    Ok(())
}

/// Spawn blockchain synchronization background task
fn spawn_chainsync_task(
    db: Database,
    config: Config,
    shutdown_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting blockchain synchronization task");

        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(config.chain.sync_interval));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match secure::workers::chainsync::sync_blockchain(&db, &config).await {
                        Ok(synced_count) => {
                            if synced_count > 0 {
                                info!("Synchronized {} blockchain events", synced_count);
                            }
                        }
                        Err(e) => {
                            error!("Blockchain sync error: {}", e);
                        }
                    }
                }
                _ = shutdown_token.cancelled() => {
                    info!("Blockchain sync task received shutdown signal");
                    break;
                }
            }
        }

        info!("Blockchain sync task stopped");
    })
}

/// Spawn algorithm runtime background task
fn spawn_runtime_task(
    db: Database,
    config: Config,
    shutdown_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting algorithm runtime task");

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
            config.runtime.poll_interval,
        ));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match secure::workers::runtime::process_pending_executions(&db, &config).await {
                        Ok(processed_count) => {
                            if processed_count > 0 {
                                info!("Processed {} algorithm executions", processed_count);
                            }
                        }
                        Err(e) => {
                            error!("Algorithm runtime error: {}", e);
                        }
                    }
                }
                _ = shutdown_token.cancelled() => {
                    info!("Algorithm runtime task received shutdown signal");
                    break;
                }
            }
        }

        info!("Algorithm runtime task stopped");
    })
}

/// Create shutdown signal handler
async fn shutdown_signal(shutdown_token: CancellationToken) {
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
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }

    // Cancel the token to signal all tasks
    shutdown_token.cancel();
}
