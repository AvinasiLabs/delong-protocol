//! Secure TEE Service Entry Point
//!
//! This is the main entry point for the secure service that runs within the TEE environment.
//! It provides the core DeLong protocol functionality, including secure computation,
//! data management, and blockchain synchronization.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use secure::config::Config;
use secure::infra::contracts::ContractCaller;
use secure::infra::db::Database;
// use secure::infra::tee::{ClientKind, KeyVault}; // TEE integration pending
use secure::infra::notification::NotificationService;
use secure::infra::ws::Hub;
use secure::workers::ChainSyncWorker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "secure=debug,tower_http=debug".into()),
        )
        .init();

    info!("Starting DeLong Protocol Secure Service");

    // Load configuration from environment variables (supports .env file)
    let config = secure::config::init_config()?;
    info!("Configuration loaded successfully");

    // Initialize database
    let db = Database::new(&config.database).await?;
    info!("Database connection established");

    // Initialize TEE key vault (temporarily disabled - TEE integration pending)
    // let key_vault = Arc::new(KeyVault::from_config(ClientKind::Dstack));
    // info!("TEE key vault initialized");

    // Initialize contract infrastructure
    let mut contract_caller = ContractCaller::new(config.chain.clone()).await?;

    // Deploy or load contracts
    info!("Ensuring contracts are deployed...");
    contract_caller.ensure_contracts_deployed().await?;
    info!("Contracts ready");

    let contract_caller = Arc::new(contract_caller);

    // Create WebSocket hub
    let ws_hub = Arc::new(Hub::new());

    // Create cancellation token for graceful shutdown
    let shutdown_token = CancellationToken::new();

    // Start background tasks
    let chainsync_handle = spawn_chainsync_task(
        db.clone(),
        config.clone(),
        contract_caller.clone(),
        ws_hub.clone(),
        shutdown_token.clone(),
    );

    let runtime_handle = spawn_runtime_task(
        db.clone(),
        config.clone(),
        contract_caller.clone(),
        ws_hub.clone(),
        shutdown_token.clone(),
    );

    // Build the application
    let app = secure::routes::create_app(db.clone(), config.clone(), ws_hub.clone()).await;

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
    contract_caller: Arc<ContractCaller>,
    ws_hub: Arc<Hub>,
    shutdown_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting blockchain synchronization service");

        // Note: ChainsyncService will coordinate with runtime through a database
        // instead of direct scheduler access
        let notification_service = Arc::new(NotificationService::new(ws_hub));
        let chainsync_worker = Arc::new(ChainSyncWorker::new(
            Arc::new(db),
            Arc::new(config),
            contract_caller,
            notification_service,
        ));

        tokio::select! {
            result = chainsync_worker.start() => {
                match result {
                    Ok(_) => info!("Chainsync service completed"),
                    Err(e) => error!("Chainsync service error: {}", e),
                }
            }
            _ = shutdown_token.cancelled() => {
                info!("Chainsync service received shutdown signal");
                // ChainSyncWorker will stop when shutdown signal is received
                info!("Stopping chainsync worker");
            }
        }

        info!("Chainsync service stopped");
    })
}

/// Spawn algorithm runtime background task
fn spawn_runtime_task(
    db: Database,
    config: Config,
    contract_caller: Arc<ContractCaller>,
    ws_hub: Arc<Hub>,
    shutdown_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting algorithm runtime worker");

        // Start the runtime worker with proper error handling
        tokio::select! {
            result = secure::workers::runtime::start_runtime_worker(db, config, contract_caller, ws_hub) => {
                match result {
                    Ok(_) => info!("Algorithm runtime worker completed"),
                    Err(e) => error!("Algorithm runtime worker error: {}", e),
                }
            }
            _ = shutdown_token.cancelled() => {
                info!("Algorithm runtime worker received shutdown signal");
            }
        }

        info!("Algorithm runtime worker stopped");
    })
}

/// Create a shutdown signal handler
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
