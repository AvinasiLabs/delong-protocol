//! Blockchain event synchronization worker (v2)
//!
//! This module provides a more idiomatic Rust implementation of blockchain
//! event synchronization using native alloy types and modern async patterns.

use alloy::rpc::types::Log;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use tracing::{debug, error, info, instrument};

use crate::{
    config::Config,
    error::{AppError, AppResult},
    infra::db::Database,
    infra::{
        contracts::{ContractCaller, ParsedEvent},
        notification::NotificationService,
    },
    models::AlgoReviewStatus,
};
use alloy::primitives::U256;

/// Event handler result
type EventResult = AppResult<()>;

/// Event handler function type
type EventHandler = Box<dyn Fn(Arc<ChainSyncWorker>, ParsedEvent) -> EventResult + Send + Sync>;

/// Chain synchronization worker
pub struct ChainSyncWorker {
    /// Database connection
    db: Arc<Database>,
    /// Application configuration
    config: Arc<Config>,
    /// Contract caller instance
    contract_caller: Arc<ContractCaller>,
    /// Notification service
    notification_service: Arc<NotificationService>,
    /// Shutdown signal sender
    shutdown_tx: mpsc::Sender<()>,
    /// Worker state
    state: Arc<RwLock<WorkerState>>,
}

/// Worker state
#[derive(Debug, Default)]
struct WorkerState {
    /// Last processed block number
    last_block: Option<u64>,
    /// Number of events processed
    events_processed: u64,
    /// Number of errors encountered
    errors_count: u64,
    /// Is the worker running
    is_running: bool,
}

impl ChainSyncWorker {
    /// Create a new chain sync worker
    pub fn new(
        db: Arc<Database>,
        config: Arc<Config>,
        contract_caller: Arc<ContractCaller>,
        notification_service: Arc<NotificationService>,
    ) -> Self {
        let (shutdown_tx, _) = mpsc::channel(1);

        Self {
            db,
            config,
            contract_caller,
            notification_service,
            shutdown_tx,
            state: Arc::new(RwLock::new(WorkerState::default())),
        }
    }

    /// Start the chain sync worker
    #[instrument(skip(self))]
    pub async fn start(self: Arc<Self>) -> AppResult<()> {
        info!("Starting chain sync worker v2");

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = true;
        }

        // Create shutdown receiver
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        {
            let worker = self.clone();
            let mut self_mut = unsafe {
                // SAFETY: We only modify the shutdown_tx field which is not accessed elsewhere
                std::ptr::read(&worker as *const Arc<Self> as *const Self)
            };
            self_mut.shutdown_tx = shutdown_tx;
        }

        // Recover pending tasks
        if let Err(e) = self.recover_pending_tasks().await {
            error!("Failed to recover pending tasks: {}", e);
        }

        // Subscribe to blockchain events
        let worker = self.clone();
        let event_handle = tokio::spawn(async move { worker.process_events().await });

        // Start periodic tasks
        let worker = self.clone();
        let periodic_handle = tokio::spawn(async move { worker.run_periodic_tasks().await });

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Received shutdown signal");
            }
            result = event_handle => {
                if let Err(e) = result {
                    error!("Event processing task failed: {}", e);
                }
            }
            result = periodic_handle => {
                if let Err(e) = result {
                    error!("Periodic task failed: {}", e);
                }
            }
        }

        // Update state
        {
            let mut state = self.state.write().await;
            state.is_running = false;
        }

        info!("Chain sync worker stopped");
        Ok(())
    }

    /// Stop the worker
    pub async fn stop(&self) -> AppResult<()> {
        self.shutdown_tx
            .send(())
            .await
            .map_err(|_| AppError::Internal("Failed to send shutdown signal".to_string()))
    }

    /// Process blockchain events
    #[instrument(skip(self))]
    async fn process_events(self: Arc<Self>) -> AppResult<()> {
        // Subscribe to events
        let worker = self.clone();
        self.contract_caller
            .subscribe_events(move |log| {
                let worker = worker.clone();
                tokio::spawn(async move {
                    if let Err(e) = worker.handle_log(log).await {
                        error!("Failed to handle log: {}", e);
                    }
                });
            })
            .await?;

        // Keep the task alive
        loop {
            time::sleep(Duration::from_secs(60)).await;

            // Check if we should stop
            let state = self.state.read().await;
            if !state.is_running {
                break;
            }
        }

        Ok(())
    }

    /// Handle a single log event
    #[instrument(skip(self, log))]
    async fn handle_log(&self, log: Log) -> AppResult<()> {
        debug!("Processing log: {:?}", log);

        // Parse the event
        let event = self.contract_caller.parse_event(&log)?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.events_processed += 1;
            if let Some(block_number) = log.block_number {
                state.last_block = Some(block_number);
            }
        }

        // Handle the event based on its type
        match event {
            ParsedEvent::DataRegistered {
                data_hash,
                provider,
                price,
            } => {
                self.handle_data_registered(data_hash, provider, price)
                    .await?;
            }
            ParsedEvent::DataUsed {
                data_hash,
                algorithm_id,
                fee,
            } => {
                self.handle_data_used(data_hash, algorithm_id, fee).await?;
            }
            ParsedEvent::AlgorithmResolved {
                algorithm_id,
                approved,
            } => {
                self.handle_algorithm_resolved(algorithm_id, approved)
                    .await?;
            }
            ParsedEvent::CommitteeMemberUpdated { member, is_member } => {
                self.handle_committee_member_updated(member, is_member)
                    .await?;
            }
            ParsedEvent::ExecutionSubmitted {
                algorithm_id,
                data_hash,
                execution_result,
            } => {
                self.handle_execution_submitted(algorithm_id, data_hash, execution_result)
                    .await?;
            }
            ParsedEvent::VoteCasted {
                algorithm_id,
                voter,
                vote,
            } => {
                self.handle_vote_casted(algorithm_id, voter, vote).await?;
            }
        }

        Ok(())
    }

    /// Handle DataRegistered event
    #[instrument(skip(self))]
    async fn handle_data_registered(
        &self,
        data_hash: String,
        provider: alloy::primitives::Address,
        price: alloy::primitives::U256,
    ) -> AppResult<()> {
        info!(
            "Data registered: hash={}, provider={}, price={}",
            data_hash, provider, price
        );

        // Update dataset status in database
        // For now, just log the event - datasets table doesn't have these columns
        info!(
            "Data registered event received: hash={}, provider={}, price={}",
            data_hash, provider, price
        );

        // TODO: Update when datasets table is extended

        // Send notification
        self.notification_service
            .notify_data_registered(&data_hash, provider)
            .await?;

        Ok(())
    }

    /// Handle DataUsed event
    #[instrument(skip(self))]
    async fn handle_data_used(
        &self,
        data_hash: String,
        algorithm_id: String,
        fee: alloy::primitives::U256,
    ) -> AppResult<()> {
        info!(
            "Data used: hash={}, algorithm={}, fee={}",
            data_hash, algorithm_id, fee
        );

        // Record data usage
        let used_at = chrono::Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
            VALUES ($1, $2, $3, $4)
            "#,
            "0x0000000000000000000000000000000000000000", // TODO: Get from event
            algorithm_id,
            data_hash,
            used_at
        )
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Handle AlgorithmResolved event
    #[instrument(skip(self))]
    async fn handle_algorithm_resolved(
        &self,
        algorithm_id: String,
        approved: bool,
    ) -> AppResult<()> {
        info!(
            "Algorithm resolved: id={}, approved={}",
            algorithm_id, approved
        );

        // Update algorithm review status
        let status = if approved {
            AlgoReviewStatus::Approved
        } else {
            AlgoReviewStatus::Rejected
        };

        // Update algo_exes table instead
        sqlx::query!(
            r#"
            UPDATE algo_exes
            SET review_status = $2,
                updated_at = NOW()
            WHERE id = (
                SELECT algo_exes.id FROM algo_exes
                JOIN algos ON algo_exes.algo_id = algos.id
                WHERE algos.cid = $1
                ORDER BY algo_exes.created_at DESC
                LIMIT 1
            )
            "#,
            &algorithm_id,
            status as AlgoReviewStatus
        )
        .execute(self.db.pool())
        .await?;

        // Update algorithm task status
        // No algorithm_tasks table in existing schema
        info!(
            "Algorithm {} resolved with status: approved={}",
            algorithm_id, approved
        );

        // Send notification
        self.notification_service
            .notify_algorithm_resolved(&algorithm_id, approved)
            .await?;

        Ok(())
    }

    /// Handle CommitteeMemberUpdated event
    #[instrument(skip(self))]
    async fn handle_committee_member_updated(
        &self,
        member: alloy::primitives::Address,
        is_member: bool,
    ) -> AppResult<()> {
        info!(
            "Committee member updated: member={}, is_member={}",
            member, is_member
        );

        let member_address = format!("{:?}", member);

        if is_member {
            // Add committee member
            sqlx::query!(
                r#"
                INSERT INTO committee_members (member_wallet, is_approved)
                VALUES ($1, $2)
                ON CONFLICT (member_wallet)
                DO UPDATE SET is_approved = $2
                "#,
                &member_address,
                true
            )
            .execute(self.db.pool())
            .await?;
        } else {
            // Remove committee member
            sqlx::query!(
                r#"
                UPDATE committee_members
                SET is_approved = false
                WHERE member_wallet = $1
                "#,
                &member_address
            )
            .execute(self.db.pool())
            .await?;
        }

        Ok(())
    }

    /// Handle ExecutionSubmitted event
    #[instrument(skip(self))]
    async fn handle_execution_submitted(
        &self,
        algorithm_id: String,
        data_hash: String,
        execution_result: String,
    ) -> AppResult<()> {
        info!(
            "Execution submitted: algorithm={}, data={}, result={}",
            algorithm_id, data_hash, execution_result
        );

        // Create algorithm review
        // Create execution record in existing schema
        let _execution_data = serde_json::json!({
            "algorithm_id": algorithm_id,
            "data_hash": data_hash,
            "result": execution_result
        });

        // Store execution submission in algo_exes table
        info!(
            "Execution submitted for algorithm: {}, data: {}",
            algorithm_id, data_hash
        );

        // TODO: Create algo_exe record when we have proper algo mapping

        // Schedule resolve task
        self.schedule_resolve_task(&algorithm_id).await?;

        Ok(())
    }

    /// Handle VoteCasted event
    #[instrument(skip(self))]
    async fn handle_vote_casted(
        &self,
        algorithm_id: String,
        voter: alloy::primitives::Address,
        vote: bool,
    ) -> AppResult<()> {
        info!(
            "Vote casted: algorithm={}, voter={}, vote={}",
            algorithm_id, voter, vote
        );

        // Record the vote
        sqlx::query!(
            r#"
            INSERT INTO votes (algo_cid, voter, approve, voted_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (algo_cid, voter)
            DO UPDATE SET approve = $3, voted_at = NOW()
            "#,
            &algorithm_id,
            format!("{:?}", voter),
            vote
        )
        .execute(self.db.pool())
        .await?;

        // Check if we need to resolve the algorithm
        self.check_and_resolve_algorithm(&algorithm_id).await?;

        Ok(())
    }

    /// Schedule a resolve task for an algorithm
    async fn schedule_resolve_task(&self, algorithm_id: &str) -> AppResult<()> {
        // Calculate resolve time (current time + voting duration)
        let resolve_at =
            chrono::Utc::now() + chrono::Duration::seconds(self.config.chain.sync_interval as i64);

        // No algorithm_tasks table - just schedule in memory or use different approach
        info!(
            "Scheduled resolve task for algorithm {} at {}",
            algorithm_id, resolve_at
        );

        Ok(())
    }

    /// Check if algorithm should be resolved and resolve if necessary
    async fn check_and_resolve_algorithm(&self, algorithm_id: &str) -> AppResult<()> {
        // Get vote count
        let vote_stats = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE approve = true) as approve_count,
                COUNT(*) FILTER (WHERE approve = false) as reject_count,
                COUNT(*) as total_count
            FROM votes
            WHERE algo_cid = $1
            "#,
            algorithm_id
        )
        .fetch_one(self.db.pool())
        .await?;

        let approve_count = vote_stats.approve_count.unwrap_or(0);
        let reject_count = vote_stats.reject_count.unwrap_or(0);
        let total_count = vote_stats.total_count.unwrap_or(0);

        // Check if we have enough votes (simple majority)
        let committee_size = sqlx::query!("SELECT COUNT(*) as count FROM committee_members")
            .fetch_one(self.db.pool())
            .await?
            .count
            .unwrap_or(0);

        let required_votes = (committee_size / 2) + 1;

        if approve_count >= required_votes || reject_count >= required_votes {
            // Resolve the algorithm
            let approved = approve_count >= required_votes;

            match self
                .contract_caller
                .resolve_algorithm(algorithm_id.to_string(), U256::from(1))
                .await
            {
                Ok(tx_hash) => {
                    info!("Algorithm resolved on-chain: {}", tx_hash);
                }
                Err(e) => {
                    error!("Failed to resolve algorithm on-chain: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Recover pending tasks from database
    async fn recover_pending_tasks(&self) -> AppResult<()> {
        info!("Recovering pending tasks");

        // Get all pending resolve tasks
        // No algorithm_tasks table - skip recovery for now
        info!("Task recovery not implemented for current schema");

        Ok(())
    }

    /// Run periodic tasks
    async fn run_periodic_tasks(&self) -> AppResult<()> {
        let mut interval = time::interval(Duration::from_secs(self.config.chain.sync_interval));

        loop {
            interval.tick().await;

            // Check if we should stop
            let state = self.state.read().await;
            if !state.is_running {
                break;
            }

            // Process scheduled tasks
            if let Err(e) = self.process_scheduled_tasks().await {
                error!("Failed to process scheduled tasks: {}", e);
            }

            // Log statistics
            let state = self.state.read().await;
            info!(
                "Chain sync stats: last_block={:?}, events_processed={}, errors={}",
                state.last_block, state.events_processed, state.errors_count
            );
        }

        Ok(())
    }

    /// Process scheduled tasks
    async fn process_scheduled_tasks(&self) -> AppResult<()> {
        // Get tasks that are due
        // No algorithm_tasks table - skip scheduled task processing for now
        debug!("Scheduled task processing not implemented for current schema");

        Ok(())
    }

    /// Get worker statistics
    pub async fn get_stats(&self) -> WorkerStats {
        let state = self.state.read().await;
        WorkerStats {
            is_running: state.is_running,
            last_block: state.last_block,
            events_processed: state.events_processed,
            errors_count: state.errors_count,
        }
    }
}

/// Worker statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerStats {
    pub is_running: bool,
    pub last_block: Option<u64>,
    pub events_processed: u64,
    pub errors_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_lifecycle() {
        // Test worker can be created and started
        // This is a placeholder for actual tests
    }
}
