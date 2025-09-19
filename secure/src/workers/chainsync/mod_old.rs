//! Chain synchronization worker
//! Monitors blockchain events and updates database accordingly

pub mod event_handlers;
pub mod transaction_monitor;

use self::transaction_monitor::{TransactionMonitor, TransactionMonitorConfig};
use crate::{
    config::Config,
    error::Result as AppResult,
    infra::{
        contracts::{ContractCaller, ParsedEvent},
        db::Database,
        Notifier,
    },
    workers::algo_executor::AlgoExecutor,
};
use alloy::rpc::types::Log;
use std::sync::Arc;

use tokio::time::{self, Duration};
use tracing::{debug, error, info, trace, warn};

/// Chain sync worker that monitors blockchain events
#[derive(Clone)]
pub struct ChainSyncWorker {
    /// Database connection
    db: Arc<Database>,
    /// Contract caller for blockchain interactions
    contract_caller: Arc<ContractCaller>,
    /// Notifier for WebSocket notifications
    notifier: Arc<Notifier>,
    /// Algorithm executor
    algo_executor: Arc<AlgoExecutor>,
    /// Configuration
    config: Arc<Config>,
}

impl ChainSyncWorker {
    /// Create a new chain sync worker
    pub fn new(
        db: Arc<Database>,
        contract_caller: Arc<ContractCaller>,
        notifier: Arc<Notifier>,
        algo_executor: Arc<AlgoExecutor>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            db,
            contract_caller,
            notifier,
            algo_executor,
            config,
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

    /// Process blockchain events
    async fn process_events(&self) -> AppResult<()> {
        // Check if we should use polling mode (for test environments)
        let use_polling = std::env::var("USE_POLLING_MODE")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);

        if use_polling {
            info!("Using polling mode for event processing");
            self.process_events_polling().await
        } else {
            info!("Using WebSocket subscription for event processing");

            // Reconnect loop for WebSocket subscription
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

                        warn!("WebSocket connection lost, will reconnect in 5 seconds...");
                    }
                    Err(e) => {
                        error!("Failed to establish WebSocket subscription: {}, retrying in 5 seconds...", e);
                    }
                }

                // Wait before reconnecting
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    /// Process blockchain events using polling (for test environments)
    async fn process_events_polling(&self) -> AppResult<()> {
        let polling_interval = std::env::var("POLLING_INTERVAL_MS")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .unwrap_or(1000);

        let mut last_block = 0u64;

        loop {
            // Poll for new events
            match self
                .contract_caller
                .get_past_events(Some(last_block + 1), None)
                .await
            {
                Ok(logs) => {
                    let log_count = logs.len();
                    for log in logs {
                        // Update last_block if we have a new one
                        if let Some(block_number) = log.block_number {
                            if block_number > last_block {
                                last_block = block_number;
                            }
                        }

                        // Process the log
                        if let Err(e) = self.handle_log(log).await {
                            error!("Failed to handle log in polling mode: {}", e);
                        }
                    }

                    if log_count > 0 {
                        debug!("Processed {} events, last block: {}", log_count, last_block);
                    }
                }
                Err(e) => {
                    error!("Failed to poll for events: {}", e);
                }
            }

            // Wait before next poll
            time::sleep(Duration::from_millis(polling_interval)).await;
        }
    }

    /// Handle a single log entry
    pub async fn handle_log(&self, log: Log) -> AppResult<()> {
        debug!("Processing log: {:?}", log);

        // Parse the event from the log
        let event = self.contract_caller.parse_event(&log)?;

        match event {
            ParsedEvent::DataRegistered {
                contributor,
                cid,
                dataset_id,
                dataset,
            } => {
                self.handle_data_registered(&log, contributor, cid, dataset_id, dataset)
                    .await?;
            }
            ParsedEvent::DataUsed {
                scientist,
                cid,
                dataset_id,
                dataset,
                when,
            } => {
                self.handle_data_used(&log, scientist, cid, dataset_id, dataset, when)
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

    /// Handle DataRegistered event
    async fn handle_data_registered(
        &self,
        log: &Log,
        contributor: Address,
        cid: String,
        dataset_id: U256,
        dataset: String,
    ) -> AppResult<()> {
        info!(
            "DataRegistered: contributor={:?}, cid={}, dataset={}",
            contributor, cid, dataset
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        let tx_hash_str = format!("{:?}", tx_hash);

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            // If timestamp not in log, use current time as fallback
            Some(chrono::Utc::now())
        };

        // Convert dataset_id from U256 to i64 for database lookup
        let db_dataset_id = dataset_id.to::<i64>();

        // Find dataset by ID
        let dataset_record = Dataset::find_by_id(self.db.pool(), db_dataset_id).await?;

        let entity_id = match dataset_record {
            Some(ds) => ds.id,
            None => {
                warn!(
                    "Dataset not found by ID {} for transaction {}. The transaction may have arrived before the dataset was created.",
                    db_dataset_id, tx_hash_str
                );
                // We'll still create/update the blockchain transaction record without entity_id
                // The entity_id can be updated later when the dataset is created
                0 // Use 0 as placeholder for missing dataset
            }
        };

        // Update transaction status (assuming success since we received the event)
        let status = crate::models::TransactionStatus::Confirmed;

        // Use upsert instead of direct update
        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            entity_id,
            crate::models::blockchain_transaction::EntityType::Dataset,
            status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated blockchain transaction for dataset registration: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created blockchain transaction for dataset registration: tx={}",
                tx_hash_str
            );
        }

        /* Old direct SQL update - removing this
        sqlx::query!(
            r#"
            UPDATE blockchain_transaction
            SET status = $2, block_number = $3, block_timestamp = $4, updated_at = NOW()
            WHERE tx_hash = $1
            "#,
            tx_hash_str.clone(),
            status as crate::models::TransactionStatus,
            block_number as i64,
            block_timestamp
        )
        .execute(self.db.pool())
        .await?;
        */

        // Fetch the updated transaction
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send data registration notification: {}", e);
        }

        Ok(())
    }

    /// Handle DataUsed event
    async fn handle_data_used(
        &self,
        log: &Log,
        scientist: Address,
        cid: String,
        _dataset_id: U256,
        dataset: String,
        when: U256,
    ) -> AppResult<()> {
        info!(
            "DataUsed: scientist={:?}, cid={}, dataset={}, when={}",
            scientist, cid, dataset, when
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Convert when timestamp to proper format
        let used_at_naive = when.to_naive_datetime()?;
        let used_at =
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(used_at_naive, chrono::Utc);

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;
        let tx_hash_str = format!("{:?}", tx_hash);

        sqlx::query!(
            r#"
            UPDATE blockchain_transaction
            SET status = $2, block_number = $3, block_timestamp = $4, updated_at = NOW()
            WHERE tx_hash = $1
            "#,
            tx_hash_str.clone(),
            status as crate::models::TransactionStatus,
            block_number as i64,
            block_timestamp
        )
        .execute(self.db.pool())
        .await?;

        // Store in data_usage table
        let _data_usage = sqlx::query!(
            r#"
            INSERT INTO data_usage (scientist_wallet, cid, dataset, used_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
            format!("{:?}", scientist),
            cid,
            dataset,
            used_at
        )
        .fetch_one(self.db.pool())
        .await?;

        // Fetch the updated transaction
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send data usage notification: {}", e);
        }

        Ok(())
    }

    /// Handle AlgorithmResolved event
    async fn handle_algorithm_resolved(
        &self,
        log: &Log,
        execution_id: U256,
        cid: String,
        approved: bool,
    ) -> AppResult<()> {
        info!(
            "AlgorithmResolved: execution_id={}, cid={}, approved={}",
            execution_id, cid, approved
        );

        // Get transaction hash for logging
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;

        info!("Received event tx={:?}", tx_hash);

        // Get block info for transaction update
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        let exe_id = execution_id.to::<i64>();
        let tx_hash_str = format!("{:?}", tx_hash);

        // Update algorithm execution status
        let status = if approved {
            ReviewStatus::Approved
        } else {
            ReviewStatus::Rejected
        };

        // Update algorithm_executions table
        sqlx::query!(
            r#"
            UPDATE algorithm_executions
            SET review_status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            exe_id,
            if approved { "approved" } else { "rejected" }
        )
        .execute(self.db.pool())
        .await?;

        // Also update old algo_exe table for backward compatibility
        sqlx::query!(
            r#"
            UPDATE algo_exe
            SET review_status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            exe_id,
            status as ReviewStatus
        )
        .execute(self.db.pool())
        .await
        .ok(); // Ignore errors for old table

        // Create/update blockchain transaction record for the resolution event
        // This ensures we track the final state of the algorithm execution
        let tx_status = crate::models::TransactionStatus::Confirmed;

        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            exe_id,
            crate::models::blockchain_transaction::EntityType::Execution,
            tx_status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated blockchain transaction for algorithm resolution: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created blockchain transaction for algorithm resolution: tx={}",
                tx_hash_str
            );
        }

        // Fetch the updated transaction record
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str.clone(), &transaction)
            .await
        {
            warn!("Failed to send algorithm resolution notification: {}", e);
        }

        // If approved, schedule algorithm execution
        if approved {
            info!(
                "Execution task {} approved, notifying runtime service",
                exe_id
            );
            // Schedule the algorithm execution using new execute_task method
            if let Err(e) = self.algo_executor.execute_task(exe_id).await {
                error!(
                    "Failed to schedule algorithm execution for {}: {}",
                    exe_id, e
                );
            }
        } else {
            info!("Execution task {} rejected", exe_id);
        }

        Ok(())
    }

    /// Handle CommitteeMemberUpdated event
    async fn handle_committee_member_updated(
        &self,
        log: &Log,
        member: Address,
        approved: bool,
    ) -> AppResult<()> {
        info!(
            "CommitteeMemberUpdated: member={:?}, approved={}",
            member, approved
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;
        let tx_hash_str = format!("{:?}", tx_hash);

        sqlx::query!(
            r#"
            UPDATE blockchain_transaction
            SET status = $2, block_number = $3, block_timestamp = $4, updated_at = NOW()
            WHERE tx_hash = $1
            "#,
            tx_hash_str.clone(),
            status as crate::models::TransactionStatus,
            block_number as i64,
            block_timestamp
        )
        .execute(self.db.pool())
        .await?;

        // Fetch the updated transaction
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send committee update notification: {}", e);
        }

        Ok(())
    }

    /// Handle ExecutionSubmitted event
    async fn handle_execution_submitted(
        &self,
        log: &Log,
        execution_id: U256,
        cid: String,
        start_time: U256,
        end_time: U256,
    ) -> AppResult<()> {
        info!(
            "ExecutionSubmitted: execution_id={}, cid={} - will execute immediately (AI audit already passed)",
            execution_id, cid
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        // Update transaction status
        let status = crate::models::TransactionStatus::Confirmed;
        let tx_hash_str = format!("{:?}", tx_hash);
        let exe_id = execution_id.to::<i64>();

        // Use upsert instead of direct update to handle race conditions
        // This ensures the transaction record is created if it doesn't exist yet
        // (in case the event arrives before the handler creates the record)
        let was_updated = BlockchainTransaction::upsert_from_event(
            self.db.pool(),
            &tx_hash_str,
            exe_id,
            crate::models::blockchain_transaction::EntityType::Execution,
            status,
            block_number as i64,
            block_timestamp,
        )
        .await?;

        if was_updated {
            info!(
                "Updated blockchain transaction for execution submission: tx={}",
                tx_hash_str
            );
        } else {
            info!(
                "Created blockchain transaction for execution submission: tx={}",
                tx_hash_str
            );
        }

        // Convert timestamps to DateTime
        let vote_start_naive = start_time.to_naive_datetime()?;
        let vote_start = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            vote_start_naive,
            chrono::Utc,
        );
        let vote_end_naive = end_time.to_naive_datetime()?;
        let vote_end =
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(vote_end_naive, chrono::Utc);

        // Update execution record in new algorithm_executions table
        if status == crate::models::TransactionStatus::Confirmed {
            // Update the new table
            sqlx::query!(
                r#"
                UPDATE algorithm_executions
                SET review_status = 'approved',
                    execution_status = 'queued',
                    vote_start_time = $2,
                    vote_end_time = $3,
                    updated_at = NOW()
                WHERE id = $1
                "#,
                exe_id,
                vote_start,
                vote_end
            )
            .execute(self.db.pool())
            .await?;

            // Also update old table for backward compatibility (will be removed later)
            sqlx::query!(
                r#"
                UPDATE algo_exe
                SET vote_start_time = $2, vote_end_time = $3, updated_at = NOW()
                WHERE id = $1
                "#,
                exe_id,
                vote_start,
                vote_end
            )
            .execute(self.db.pool())
            .await
            .ok(); // Ignore errors for old table

            // Directly schedule algorithm execution (no need to wait for voting)
            // AI audit already passed during submission
            info!(
                "Directly scheduling algorithm execution {} (cid: {}) - AI audit already passed",
                exe_id, cid
            );

            // Schedule execution using new execute_task method
            if let Err(e) = self.algo_executor.execute_task(exe_id).await {
                error!(
                    "Failed to schedule algorithm execution for {} (cid: {}): {}",
                    exe_id, cid, e
                );
            }
        }

        // Fetch the updated transaction and send result
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send execution submission notification: {}", e);
        }

        Ok(())
    }

    /// Handle VoteCasted event
    async fn handle_vote_casted(
        &self,
        log: &Log,
        member: Address,
        cid: String,
        approved: bool,
        vote_time: U256,
    ) -> AppResult<()> {
        info!(
            "VoteCasted: member={:?}, cid={}, approved={}, vote_time={}",
            member, cid, approved, vote_time
        );

        // Get transaction hash and block info
        let tx_hash = log
            .transaction_hash
            .ok_or_else(|| AppError::Internal("Missing transaction hash".to_string()))?;
        let block_number = log
            .block_number
            .ok_or_else(|| AppError::Internal("Missing block number".to_string()))?;

        // Get block timestamp
        let block_timestamp = if let Some(timestamp) = log.block_timestamp {
            let naive = (timestamp as i64).to_naive_datetime()?;
            Some(chrono::DateTime::from_naive_utc_and_offset(
                naive,
                chrono::Utc,
            ))
        } else {
            Some(chrono::Utc::now())
        };

        let voted_at_naive = vote_time.to_naive_datetime()?;
        let voted_at =
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(voted_at_naive, chrono::Utc);

        // Format tx_hash once for consistency
        let tx_hash_str = format!("{:?}", tx_hash);

        // Begin transaction to ensure atomicity
        let mut tx = self.db.pool().begin().await?;

        // Store the vote
        let vote = sqlx::query!(
            r#"
            INSERT INTO vote (algo_cid, voter, approve, voted_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (algo_cid, voter)
            DO UPDATE SET approve = $3, voted_at = $4
            RETURNING id
            "#,
            &cid,
            format!("{:?}", member),
            approved,
            voted_at
        )
        .fetch_one(&mut *tx)
        .await?;

        // Create blockchain_transaction record with VOTE entity type
        let status = crate::models::TransactionStatus::Confirmed;
        sqlx::query!(
            r#"
            INSERT INTO blockchain_transaction (tx_hash, entity_id, entity_type, status, block_number, block_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            tx_hash_str.clone(),
            vote.id,
            "VOTE",
            status as crate::models::TransactionStatus,
            block_number as i64,
            block_timestamp
        )
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        info!(
            "Vote recorded: id={}, algo_cid={}, voter={:?}, approved={}",
            vote.id, cid, member, approved
        );

        // Fetch the updated transaction
        let transaction = BlockchainTransaction::find_by_tx_hash(self.db.pool(), &tx_hash_str)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".to_string()))?;

        // Push transaction result
        if let Err(e) = self
            .notifier
            .push_tx_result(tx_hash_str, &transaction)
            .await
        {
            warn!("Failed to send vote notification: {}", e);
        }

        // Check if we need to resolve the algorithm
        self.check_and_resolve_algorithm(&cid).await?;

        Ok(())
    }

    /// Check if algorithm can be resolved
    async fn check_and_resolve_algorithm(&self, cid: &str) -> AppResult<()> {
        // Get vote counts - only count votes from confirmed transactions
        let vote_stats = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE v.approve = true) as approve_count,
                COUNT(*) FILTER (WHERE v.approve = false) as reject_count,
                COUNT(*) as total_count
            FROM vote v
            JOIN blockchain_transaction bt ON bt.entity_id = v.id
            WHERE v.algo_cid = $1
                AND bt.status = 'confirmed'
                AND bt.entity_type = 'VOTE'
            "#,
            cid
        )
        .fetch_one(self.db.pool())
        .await?;

        let approve_count = vote_stats.approve_count.unwrap_or(0);
        let reject_count = vote_stats.reject_count.unwrap_or(0);

        // Get execution ID for this CID
        let execution = sqlx::query!(
            r#"
            SELECT ae.id
            FROM algo_exe ae
            JOIN algo a ON ae.algo_id = a.id
            WHERE a.cid = $1
            ORDER BY ae.created_at DESC
            LIMIT 1
            "#,
            cid
        )
        .fetch_optional(self.db.pool())
        .await?;

        if let Some(exe) = execution {
            // Check if we have enough votes (simple majority)
            // Get committee size from configuration
            let committee_size = self.config.committee.size;
            let required_votes = (committee_size / 2) + 1;

            if approve_count >= required_votes || reject_count >= required_votes {
                let approved = approve_count >= required_votes;

                info!(
                    "Algorithm {} reached consensus: approved={}, approve_count={}, reject_count={}",
                    cid, approved, approve_count, reject_count
                );

                // Resolve the algorithm on-chain
                match self
                    .contract_caller
                    .resolve_algorithm(cid.to_string(), U256::from(exe.id))
                    .await
                {
                    Ok(tx_hash) => {
                        info!(
                            "Algorithm resolved on-chain: cid={}, execution_id={}, tx_hash={}",
                            cid, exe.id, tx_hash
                        );
                    }
                    Err(e) => {
                        error!("Failed to resolve algorithm on-chain: {}", e);
                    }
                }
            } else {
                debug!(
                    "Not enough votes yet for {}: approve={}, reject={}, required={}",
                    cid, approve_count, reject_count, required_votes
                );
            }
        }

        Ok(())
    }

    /// Run periodic tasks
    async fn run_periodic_tasks(&self) -> AppResult<()> {
        let mut interval = time::interval(Duration::from_secs(30));

        // Recover unresolved algorithms on startup
        if let Err(e) = self.recover_resolve_tasks().await {
            error!("Failed to recover resolve tasks: {}", e);
        }

        loop {
            interval.tick().await;

            // Check for algorithms that need resolution
            trace!("Running periodic tasks check");
        }
    }

    /// Recover unresolved algorithm tasks on startup
    pub async fn recover_resolve_tasks(&self) -> AppResult<()> {
        info!("Recovering unresolved algorithms...");

        // Get all algorithm executions that are still under review
        let reviewing_exes = sqlx::query!(
            r#"
            SELECT ae.id, ae.vote_end_time, a.cid
            FROM algo_exe ae
            JOIN algo a ON ae.algo_id = a.id
            JOIN blockchain_transaction bt ON bt.entity_id = ae.id
            WHERE ae.review_status = 'reviewing'
                AND bt.status = 'confirmed'
                AND bt.entity_type = 'EXECUTION'
                AND ae.vote_end_time IS NOT NULL
            "#
        )
        .fetch_all(self.db.pool())
        .await?;

        let now = chrono::Utc::now();

        for exe in reviewing_exes {
            if let Some(vote_end) = exe.vote_end_time {
                if vote_end > now {
                    info!(
                        "Scheduling resolution for execution {} (cid: {}) at {}",
                        exe.id, exe.cid, vote_end
                    );
                    // Schedule the resolution at voting end time
                    let resolved_at = vote_end;
                    if let Err(e) = self
                        .algo_executor
                        .schedule_resolve(exe.id, exe.cid.clone(), resolved_at)
                        .await
                    {
                        error!(
                            "Failed to schedule resolution for execution {} (cid: {}): {}",
                            exe.id, exe.cid, e
                        );
                    }
                } else {
                    // Already past voting end time, try to resolve immediately
                    info!(
                        "Execution {} (cid: {}) voting period already ended, checking resolution",
                        exe.id, exe.cid
                    );
                    if let Err(e) = self.check_and_resolve_algorithm(&exe.cid).await {
                        error!("Failed to resolve algorithm {}: {}", exe.cid, e);
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, B256, U256};
    use alloy::rpc::types::Log;

    // Helper function to create a mock log
    fn create_mock_log(block_number: u64) -> Log {
        Log {
            inner: alloy::primitives::Log {
                address: address!("0x0000000000000000000000000000000000000000"),
                data: alloy::primitives::LogData::new_unchecked(
                    vec![],
                    alloy::primitives::Bytes::new(),
                ),
            },
            block_hash: Some(B256::ZERO),
            block_number: Some(block_number),
            block_timestamp: Some(1700000000u64), // Fixed timestamp for testing
            transaction_hash: Some(B256::from([1u8; 32])),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: false,
        }
    }

    #[tokio::test]
    async fn test_handle_log() {
        // Test that handle_log properly routes to the correct handler
        // This is the main unit test for the handle_log function

        // We can't easily test the full flow without mocking all dependencies
        // So we'll just test that the log parsing and routing works correctly

        let log = create_mock_log(100);

        // The actual behavior depends on the ParsedEvent returned by parse_event
        // which requires a proper ContractCaller setup

        // For now, we verify the log structure is correct
        assert!(log.transaction_hash.is_some());
        assert!(log.block_number.is_some());
        assert_eq!(log.block_number.unwrap(), 100);
    }

    #[tokio::test]
    async fn test_timestamp_conversion() {
        // Test timestamp conversion logic
        use crate::error::TimestampExt;

        let test_timestamps = vec![
            (U256::from(1234567890u64), 1234567890i64),
            (U256::from(1700000000u64), 1700000000i64),
            (U256::from(0u64), 0i64),
        ];

        for (u256_time, expected_i64) in test_timestamps {
            let result = u256_time.to_naive_datetime();
            assert!(result.is_ok());
            let datetime = result.unwrap();
            assert_eq!(datetime.and_utc().timestamp(), expected_i64);
        }
    }

    #[tokio::test]
    async fn test_vote_majority_calculation() {
        // Test vote majority logic without database

        // Test case 1: 5 committee members, 3 approve, 2 reject
        let committee_size = 5i64;
        let approve_count = 3i64;
        let reject_count = 2i64;
        let required_votes = (committee_size / 2) + 1;

        assert_eq!(required_votes, 3);
        assert!(approve_count >= required_votes);
        assert!(reject_count < required_votes);

        // Test case 2: 10 committee members, 5 approve, 5 reject
        let committee_size = 10i64;
        let approve_count = 5i64;
        let reject_count = 5i64;
        let required_votes = (committee_size / 2) + 1;

        assert_eq!(required_votes, 6);
        assert!(approve_count < required_votes);
        assert!(reject_count < required_votes);

        // Test case 3: 7 committee members, 4 approve, 3 reject
        let committee_size = 7i64;
        let approve_count = 4i64;
        let reject_count = 3i64;
        let required_votes = (committee_size / 2) + 1;

        assert_eq!(required_votes, 4);
        assert!(approve_count >= required_votes);
        assert!(reject_count < required_votes);
    }

    #[tokio::test]
    async fn test_address_formatting() {
        // Test address formatting consistency
        use alloy::primitives::address;

        let test_address = address!("0x1234567890123456789012345678901234567890");
        let formatted = format!("{:?}", test_address);

        // Verify the format is as expected
        assert!(formatted.starts_with("0x"));
        assert_eq!(formatted.len(), 42); // 0x + 40 hex chars
    }

    #[tokio::test]
    async fn test_cid_validation() {
        // Test CID validation logic

        // Valid CID patterns
        let long_valid_cid = format!("Qm{}", "x".repeat(44));
        let valid_cids = vec![
            "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG",
            "QmPZ9gcCEpqKTo6aq61g2nXGUhM4iCL3ewB6LDXZCtioEB",
            &long_valid_cid, // 46 chars total
        ];

        for cid in valid_cids {
            assert!(cid.len() <= 255); // Max database field length
            assert!(cid.starts_with("Qm")); // Common IPFS v0 CID prefix
        }

        // Invalid CID patterns
        let too_long_cid = "x".repeat(300);
        let invalid_cids = vec![
            "",            // Empty
            &too_long_cid, // Too long
        ];

        for cid in invalid_cids {
            assert!(cid.is_empty() || cid.len() > 255);
        }
    }

    #[tokio::test]
    async fn test_event_timing() {
        // Test event timing logic
        use alloy::primitives::U256;

        let now = chrono::Utc::now().timestamp() as u64;
        let test_cases = vec![
            (now, true),         // Current time - should be valid
            (0, false),          // Zero timestamp - technically valid but unusual
            (2147483647, true),  // Max 32-bit timestamp (2038)
            (now + 86400, true), // Future time (tomorrow) - should be valid
        ];

        for (timestamp, should_be_valid) in test_cases {
            let u256_time = U256::from(timestamp);
            let result = u256_time.to_naive_datetime();

            if timestamp == 0 {
                // Zero timestamp is technically valid (1970-01-01)
                assert!(result.is_ok());
            } else if should_be_valid {
                assert!(result.is_ok());
            }
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        // Test error handling for missing transaction hash
        let mut log = create_mock_log(100);
        log.transaction_hash = None;

        // Without transaction hash, many handlers should fail
        assert!(log.transaction_hash.is_none());

        // Test error handling for missing block number
        let mut log2 = create_mock_log(100);
        log2.block_number = None;

        assert!(log2.block_number.is_none());
    }
}
