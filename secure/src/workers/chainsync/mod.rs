//! Chain synchronization worker
//! Monitors blockchain events and updates database accordingly

pub mod event_handlers;
pub mod transaction_monitor;

use self::transaction_monitor::{TransactionMonitor, TransactionMonitorConfig};
use crate::{
    error::Result as AppResult,
    infra::{
        contracts::{ContractCaller, ParsedEvent},
        db::Database,
        Notifier,
    },
    workers::executor::Executor,
};
use alloy::rpc::types::Log;
use std::sync::Arc;

use tokio::time::{self, Duration};
use tracing::{debug, error, info, trace, warn};

/// Chain sync worker that monitors blockchain events
#[derive(Clone)]
pub struct ChainSyncWorker {
    /// Database connection
    pub(crate) db: Arc<Database>,
    /// Contract caller for blockchain interactions
    pub(crate) contract_caller: Arc<ContractCaller>,
    /// Notifier for WebSocket notifications
    pub(crate) notifier: Arc<Notifier>,
    /// Algorithm executor
    pub(crate) executor: Arc<Executor>,
}

impl ChainSyncWorker {
    /// Create a new chain sync worker
    pub fn new(
        db: Arc<Database>,
        contract_caller: Arc<ContractCaller>,
        notifier: Arc<Notifier>,
        algo_executor: Arc<Executor>,
    ) -> Self {
        Self {
            db,
            contract_caller,
            notifier,
            executor: algo_executor,
        }
    }

    /// Start the worker
    pub async fn start(self: Arc<Self>) -> AppResult<()> {
        info!("Starting chain sync worker");

        // Start event processing in background
        let worker = self.clone();
        let event_handle = tokio::spawn(async move {
            if let Err(e) = worker.process_events().await {
                error!("Event processing error: {}", e);
            }
        });

        // Start periodic tasks in background
        let worker = self.clone();
        let periodic_handle = tokio::spawn(async move {
            if let Err(e) = worker.run_periodic_tasks().await {
                error!("Periodic tasks error: {}", e);
            }
        });

        // Wait for tasks to complete (they run indefinitely)
        tokio::select! {
            _ = event_handle => info!("Event processing task completed"),
            _ = periodic_handle => info!("Periodic tasks completed"),
        }

        info!("Chain sync worker stopped");
        Ok(())
    }

    /// Start the worker with transaction monitor
    pub async fn start_with_monitor(
        self: Arc<Self>,
        redis_pool: deadpool_redis::Pool,
    ) -> AppResult<()> {
        info!("Starting chain sync worker with transaction monitor");

        // Create transaction monitor
        let monitor_config = TransactionMonitorConfig::default();
        let monitor = Arc::new(TransactionMonitor::new(
            self.db.clone(),
            self.contract_caller.clone(),
            redis_pool,
            monitor_config,
        ));

        // Start transaction monitor in background
        let monitor_clone = monitor.clone();
        let monitor_handle = tokio::spawn(async move {
            if let Err(e) = monitor_clone.monitor_loop().await {
                error!("Transaction monitor error: {}", e);
            }
        });

        // Start event processing in background
        let worker = self.clone();
        let event_handle = tokio::spawn(async move {
            if let Err(e) = worker.process_events().await {
                error!("Event processing error: {}", e);
            }
        });

        // Start periodic tasks in background
        let worker = self.clone();
        let periodic_handle = tokio::spawn(async move {
            if let Err(e) = worker.run_periodic_tasks().await {
                error!("Periodic tasks error: {}", e);
            }
        });

        // Wait for tasks to complete (they run indefinitely)
        tokio::select! {
            _ = monitor_handle => info!("Transaction monitor task completed"),
            _ = event_handle => info!("Event processing task completed"),
            _ = periodic_handle => info!("Periodic tasks completed"),
        }

        info!("Chain sync worker with monitor stopped");
        Ok(())
    }

    /// Process blockchain events using WebSocket subscription
    async fn process_events(&self) -> AppResult<()> {
        info!("Starting event processing with WebSocket");

        loop {
            info!("Establishing WebSocket subscription...");

            let worker = self.clone();
            match self
                .contract_caller
                .subscribe_events(move |log| {
                    let worker = worker.clone();
                    tokio::spawn(async move {
                        if let Err(e) = worker.handle_log(log).await {
                            error!("Failed to handle log: {}", e);
                        }
                    });
                })
                .await
            {
                Ok(handle) => {
                    info!("WebSocket subscription established successfully");

                    // Wait for the subscription task to complete (it shouldn't in normal operation)
                    match handle.await {
                        Ok(_) => {
                            warn!("WebSocket subscription task ended normally");
                        }
                        Err(e) => {
                            error!("WebSocket subscription task panicked: {}", e);
                        }
                    }

                    // If subscription ends, try to reconnect after a delay
                    warn!("WebSocket connection lost, reconnecting in 5 seconds...");
                    time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    error!("Failed to establish WebSocket subscription: {}, retrying in 5 seconds...", e);
                    time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Handle a single log entry
    pub async fn handle_log(&self, log: Log) -> AppResult<()> {
        debug!("Processing log: {:?}", log);

        // Parse the event from the log
        let event = self.contract_caller.parse_event(&log)?;

        // Delegate to specific handlers in event_handlers module
        match event {
            ParsedEvent::DatasetRegistered {
                contributor,
                cid,
                dataset_id,
                dataset,
            } => {
                self.handle_data_registered(&log, contributor, cid, dataset_id, dataset)
                    .await?;
            }
            ParsedEvent::AlgorithmExecuted {
                scientist,
                algo_cid,
                dataset_id,
                dataset_cid,
                success: _,
                timestamp,
            } => {
                self.handle_data_used(&log, scientist, algo_cid, dataset_id, dataset_cid, timestamp)
                    .await?;
            }
            ParsedEvent::AlgorithmResolved {
                execution_id,
                cid,
                approved,
            } => {
                self.handle_algorithm_resolved(&log, execution_id, cid, approved)
                    .await?;
            }
            ParsedEvent::CommitteeMemberUpdated { member, approved } => {
                self.handle_committee_member_updated(&log, member, approved)
                    .await?;
            }
            ParsedEvent::ExecutionSubmitted {
                execution_id,
                cid,
                start_time,
                end_time,
            } => {
                self.handle_execution_submitted(&log, execution_id, cid, start_time, end_time)
                    .await?;
            }
            ParsedEvent::VoteCasted {
                member,
                cid,
                approved,
                vote_time,
            } => {
                self.handle_vote_casted(&log, member, cid, approved, vote_time)
                    .await?;
            }
        }

        Ok(())
    }

    /// Handle DataUsed event (kept simple for now)
    async fn handle_data_used(
        &self,
        _log: &Log,
        scientist: alloy::primitives::Address,
        cid: String,
        dataset_id: alloy::primitives::U256,
        dataset: String,
        when: alloy::primitives::U256,
    ) -> AppResult<()> {
        info!(
            "DataUsed: scientist={:?}, cid={}, dataset={}, when={}",
            scientist, cid, dataset, when
        );

        // Convert dataset_id and when to i64
        let _db_dataset_id = dataset_id.to::<i64>();
        let _when_timestamp = when.to::<i64>();

        // TODO: Implementation for data usage tracking
        // This will be handled in a future refactoring

        Ok(())
    }

    /// Run periodic maintenance tasks
    async fn run_periodic_tasks(&self) -> AppResult<()> {
        let mut interval = time::interval(Duration::from_secs(60)); // Run every minute

        loop {
            interval.tick().await;
            trace!("Running periodic tasks");

            // Task 1: Check for stuck transactions
            if let Err(e) = self.check_stuck_transactions().await {
                warn!("Failed to check stuck transactions: {}", e);
            }

            // Task 2: Cleanup old data (if needed)
            if let Err(e) = self.cleanup_old_data().await {
                warn!("Failed to cleanup old data: {}", e);
            }
        }
    }

    /// Check for stuck transactions and retry them
    async fn check_stuck_transactions(&self) -> AppResult<()> {
        // Check for transactions that have been pending for too long
        let stuck_threshold_minutes = 10.0;
        let stuck_txs = sqlx::query!(
            r#"
            SELECT tx_hash, entity_id, entity_type as "entity_type: crate::models::EntityType"
            FROM transaction
            WHERE status = 'pending'
            AND created_at < NOW() - INTERVAL '1 minute' * $1
            LIMIT 10
            "#,
            stuck_threshold_minutes
        )
        .fetch_all(self.db.pool())
        .await?;

        for tx in stuck_txs {
            info!(
                "Found stuck transaction: {} for entity {} (type: {:?})",
                tx.tx_hash, tx.entity_id, tx.entity_type
            );

            // TODO: Implement retry logic or mark as failed
        }

        Ok(())
    }

    /// Cleanup old data
    async fn cleanup_old_data(&self) -> AppResult<()> {
        // For example, cleanup very old failed transactions
        let days_to_keep = 30.0;
        let result = sqlx::query!(
            r#"
            DELETE FROM transaction
            WHERE status = 'failed'
            AND created_at < NOW() - INTERVAL '1 day' * $1
            "#,
            days_to_keep
        )
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() > 0 {
            info!(
                "Cleaned up {} old failed transactions",
                result.rows_affected()
            );
        }

        Ok(())
    }
}
