// Business logic services for the secure service

use std::sync::Arc;
use tracing::{info, warn};
use common::ApiResult;

pub mod algo_exe;
pub mod blockchain;
pub mod committee;
pub mod vote;
pub mod blockchain_sync;

// Re-export services for convenience
pub use algo_exe::AlgoExeService;
pub use blockchain::BlockchainService;
pub use committee::CommitteeService;
pub use vote::VoteService;
// Use the BlockchainSyncService from sync module instead
pub use crate::sync::blockchain_sync::BlockchainSyncService;

/// Service manager for coordinating all secure services
#[derive(Clone)]
pub struct ServiceManager {
    pub algo_exe_service: Arc<AlgoExeService>,
    pub blockchain_service: Arc<BlockchainService>,
    pub committee_service: Arc<CommitteeService>,
    pub vote_service: Arc<VoteService>,
    pub blockchain_sync: Arc<BlockchainSyncService>,
}

impl ServiceManager {
    /// Create a new service manager with all services
    pub async fn new(config: crate::config::SecureConfig, db_pool: sqlx::PgPool) -> ApiResult<Self> {
        info!("Initializing secure service manager");

        // Initialize individual services (these are stateless structs)
        let algo_exe_service = Arc::new(AlgoExeService);
        let blockchain_service = Arc::new(BlockchainService);
        let committee_service = Arc::new(CommitteeService);
        let vote_service = Arc::new(VoteService);
        
        // Initialize blockchain sync with dependencies
        let blockchain_sync = Arc::new(BlockchainSyncService::new(
            config,
            db_pool
        ));

        Ok(Self {
            algo_exe_service,
            blockchain_service,
            committee_service,
            vote_service,
            blockchain_sync,
        })
    }

    /// Start all services
    pub async fn start(&self) -> ApiResult<()> {
        info!("Starting all secure services");

        // Blockchain sync service will be started separately
        // as it requires a mutable reference and runs in its own task
        info!("Secure services initialization complete - blockchain sync should be started separately");
        Ok(())
    }

    /// Stop all services gracefully
    pub async fn stop(&self) -> ApiResult<()> {
        info!("Stopping all secure services");

        // Stop blockchain sync service
        self.blockchain_sync.stop().await;

        info!("All secure services stopped successfully");
        Ok(())
    }

    /// Health check for all services
    pub async fn health_check(&self) -> ApiResult<()> {
        // Check if blockchain sync is running
        let is_running = self.blockchain_sync.is_running().await;
        if !is_running {
            warn!("Blockchain sync service is not running");
        }

        info!("All secure services health check complete");
        Ok(())
    }
} 