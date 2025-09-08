//! Secure TEE Service Entry Point
//!
//! This is the main entry point for the secure service that runs within the TEE environment.
//! It provides the core DeLong protocol functionality, including secure computation,
//! data management, and blockchain synchronization.

use ipfs_api_backend_hyper::TryFromUri;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use secure::config::Config;
use secure::infra::contracts::ContractCaller;
use secure::infra::db::Database;
use secure::infra::Notifier;
use secure::infra::{TeeClientBuilder, TeeEthereum};
use secure::workers::algo_executor::AlgoExecutor;
use secure::workers::ChainSyncWorker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment variables (supports .env file)
    let config = secure::Config::load()?;
    info!("Configuration loaded successfully");

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "secure=debug,tower_http=debug".into()),
        )
        .init();

    info!("Starting DeLong Protocol Secure Service");

    // Initialize database
    let db = Database::new(&config.database).await?;
    info!("Database connection established");

    // Initialize TEE services
    let tee_endpoint = std::env::var("DSTACK_SIMULATOR_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:11010".to_string());
    let tee_client = Arc::new(TeeClientBuilder::new().endpoint(tee_endpoint).build());
    info!("TEE client initialized");

    // Initialize TEE Ethereum manager
    let tee_ethereum = Arc::new(TeeEthereum::new(tee_client.clone()));
    info!("TEE Ethereum manager initialized");

    // Initialize Redis connection pool
    let mut redis_config = deadpool_redis::Config::from_url(config.redis.url.clone());
    redis_config.pool = Some(deadpool_redis::PoolConfig {
        max_size: config.redis.max_connections,
        timeouts: deadpool_redis::Timeouts {
            wait: Some(std::time::Duration::from_secs(
                config.redis.connection_timeout,
            )),
            create: Some(std::time::Duration::from_secs(
                config.redis.connection_timeout,
            )),
            recycle: Some(std::time::Duration::from_secs(
                config.redis.connection_timeout,
            )),
        },
        ..Default::default()
    });

    let redis_pool = redis_config
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("Failed to create Redis pool");

    info!("Redis connection pool initialized");

    // Initialize IPFS client
    let ipfs_client = Arc::new(
        ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
            .expect("Failed to create IPFS client"),
    );

    // Initialize contract infrastructure with TEE wallet
    let mut contract_caller = ContractCaller::new(config.chain.clone(), db.clone())
        .await?
        .with_tee(tee_ethereum.clone())
        .await?;

    // Ensure TEE wallet has sufficient balance for operations
    contract_caller.ensure_sufficient_balance(0.1).await?;
    info!("TEE wallet funded and ready");

    // Deploy or load contracts using TEE wallet
    info!("Ensuring contracts are deployed...");
    contract_caller.ensure_contracts_deployed().await?;
    info!("Contracts ready");

    let contract_caller = Arc::new(contract_caller);

    // Initialize algorithm executor - use config from environment
    let executor_config = config.executor.clone();

    // Initialize TEE crypto service for dataset encryption/decryption
    let tee_crypto = Arc::new(secure::infra::TeeCryptoService::new(tee_client.clone()));

    let algo_executor = secure::workers::algo_executor::create_executor_service(
        Arc::new(db.clone()),
        ipfs_client.clone(),
        contract_caller.clone(),
        tee_crypto.clone(),
        executor_config,
    )
    .await
    .expect("Failed to create algorithm executor");

    // Create notifier for WebSocket notifications
    let notifier = Arc::new(Notifier::new());

    // Create cancellation token for graceful shutdown
    let shutdown_token = CancellationToken::new();

    // Start background tasks
    let chainsync_handle = spawn_chainsync_task(
        db.clone(),
        contract_caller.clone(),
        notifier.clone(),
        algo_executor.clone(),
        Arc::new(config.clone()),
        redis_pool.clone(),
        shutdown_token.clone(),
    );

    // Build the application
    let app = secure::routes::create_app(
        db.clone(),
        config.clone(),
        ipfs_client.clone(),
        contract_caller.clone(),
        notifier.clone(),
        tee_client.clone(),
        tee_ethereum.clone(),
        redis_pool.clone(),
    )
    .await;

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
    chainsync_handle.await.unwrap();

    info!("Secure service stopped gracefully");
    Ok(())
}

/// Spawn blockchain synchronization background task
fn spawn_chainsync_task(
    db: Database,
    contract_caller: Arc<ContractCaller>,
    notifier: Arc<Notifier>,
    algo_executor: Arc<AlgoExecutor>,
    config: Arc<Config>,
    redis_pool: deadpool_redis::Pool,
    shutdown_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting blockchain synchronization service");

        // Note: ChainsyncService will coordinate with runtime through a database
        // instead of direct scheduler access
        let chain_sync = Arc::new(ChainSyncWorker::new(
            Arc::new(db),
            contract_caller,
            notifier,
            algo_executor,
            config,
        ));

        tokio::select! {
            result = chain_sync.start_with_monitor(redis_pool) => {
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

/// Graceful shutdown signal handler
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
