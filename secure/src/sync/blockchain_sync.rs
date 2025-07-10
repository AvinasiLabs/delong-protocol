use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use common::ApiError;
use crate::config::SecureConfig;
use crate::services::{CommitteeService, VoteService};

/// Blockchain event types that we monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockchainEvent {
    /// Algorithm submitted to the blockchain
    AlgorithmSubmitted {
        execution_id: u64,
        scientist_wallet: String,
        algorithm_cid: String,
        dataset: String,
    },
    /// Static dataset registered on blockchain
    DatasetRegistered {
        dataset_id: u64,
        author_wallet: String,
        ipfs_cid: String,
        file_hash: String,
    },
    /// Vote cast by committee member
    VoteCast {
        execution_id: u64,
        voter_wallet: String,
        decision: String, // "APPROVE" or "REJECT"
    },
    /// Committee member added or removed
    CommitteeUpdated {
        wallet_address: String,
        is_active: bool,
    },
    /// Transaction confirmed on blockchain
    TransactionConfirmed {
        tx_hash: String,
        block_number: u64,
        block_timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Blockchain synchronization service
#[derive(Clone)]
pub struct BlockchainSyncService {
    config: SecureConfig,
    current_block: u64,
    is_running: Arc<tokio::sync::RwLock<bool>>,
    db_pool: PgPool,
}

impl BlockchainSyncService {
    /// Create a new blockchain sync service
    pub fn new(
        config: SecureConfig,
        db_pool: PgPool,
    ) -> Self {
        Self {
            config,
            current_block: 0,
            is_running: Arc::new(tokio::sync::RwLock::new(false)),
            db_pool,
        }
    }

    /// Start the blockchain synchronization service
    pub async fn start(&mut self) -> Result<(), ApiError> {
        if !self.config.blockchain.sync_enabled {
            info!("Blockchain synchronization is disabled in configuration");
            return Ok(());
        }
        
        info!("Starting blockchain synchronization service");
        
        // Set running flag
        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }

        // Initialize starting block number
        self.current_block = self.get_last_processed_block().await?;
        info!(block = %self.current_block, "Starting sync from block");

        // Start the main sync loop
        let sync_interval = Duration::from_secs(self.config.blockchain.sync_interval_seconds);
        let mut timer = interval(sync_interval);

        loop {
            // Check if we should stop
            {
                let is_running = self.is_running.read().await;
                if !*is_running {
                    info!("Blockchain sync service stopping");
                    break;
                }
            }

            timer.tick().await;

            // Sync new blocks
            if let Err(e) = self.sync_new_blocks().await {
                error!(error = %e, "Failed to sync new blocks, retrying in next cycle");
                // Continue on error - don't stop the service
            }
        }

        Ok(())
    }

    /// Stop the blockchain synchronization service
    pub async fn stop(&self) {
        info!("Stopping blockchain synchronization service");
        let mut is_running = self.is_running.write().await;
        *is_running = false;
    }

    /// Check if the service is currently running
    pub async fn is_running(&self) -> bool {
        let is_running = self.is_running.read().await;
        *is_running
    }

    /// Sync new blocks from the blockchain
    async fn sync_new_blocks(&mut self) -> Result<(), ApiError> {
        let latest_block = self.get_latest_block_number().await?;
        
        if latest_block <= self.current_block {
            // No new blocks to process
            return Ok(());
        }

        info!(
            from_block = %self.current_block,
            to_block = %latest_block,
            "Syncing new blocks"
        );

        // Process blocks in batches to avoid overwhelming the system
        let batch_size = 100;
        let mut current = self.current_block + 1;
        
        while current <= latest_block {
            let end_block = std::cmp::min(current + batch_size - 1, latest_block);
            
            if let Err(e) = self.process_block_range(current, end_block).await {
                error!(
                    error = %e,
                    from_block = %current,
                    to_block = %end_block,
                    "Failed to process block range"
                );
                // Don't advance current_block on error
                return Err(e);
            }

            current = end_block + 1;
            self.current_block = end_block;
            
            // Update last processed block
            self.update_last_processed_block(end_block).await?;
            
            // Small delay between batches to avoid overwhelming the RPC
            sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// Process a range of blocks for blockchain events
    async fn process_block_range(&self, from_block: u64, to_block: u64) -> Result<(), ApiError> {
        info!(from_block = %from_block, to_block = %to_block, "Processing block range");

        // Mock implementation - in production this would query actual blockchain
        match self.config.blockchain.client_type.as_str() {
            "mock" => {
                // Mock blockchain events for testing
                self.process_mock_events(from_block, to_block).await?;
            }
            "ethereum" => {
                // Real Ethereum blockchain integration
                self.process_ethereum_events(from_block, to_block).await?;
            }
            _ => {
                return Err(ApiError::ConfigurationError(
                    format!("Unsupported blockchain client type: {}", self.config.blockchain.client_type)
                ));
            }
        }

        Ok(())
    }

    /// Process mock blockchain events for testing
    async fn process_mock_events(&self, from_block: u64, to_block: u64) -> Result<(), ApiError> {
        info!(from_block = %from_block, to_block = %to_block, "Processing mock blockchain events");
        
        // Generate limited mock events for demonstration (much less frequent)
        for block_num in from_block..=to_block {
            if block_num % 100 == 0 {
                // Mock algorithm submission every 100 blocks (less frequent)
                let event = BlockchainEvent::AlgorithmSubmitted {
                    execution_id: block_num,
                    scientist_wallet: format!("0x{:040x}", block_num),
                    algorithm_cid: format!("QmMock{}", block_num),
                    dataset: "mock_dataset".to_string(),
                };
                
                self.handle_blockchain_event(event, block_num).await?;
            }
            
            if block_num % 200 == 0 {
                // Mock transaction confirmation every 200 blocks (less frequent)
                let event = BlockchainEvent::TransactionConfirmed {
                    tx_hash: format!("0x{:064x}", block_num),
                    block_number: block_num,
                    block_timestamp: chrono::Utc::now(),
                };
                
                self.handle_blockchain_event(event, block_num).await?;
            }
        }

        Ok(())
    }

    /// Process real Ethereum blockchain events
    async fn process_ethereum_events(&self, from_block: u64, to_block: u64) -> Result<(), ApiError> {
        // TODO: Implement actual Ethereum event listening
        warn!(
            from_block = %from_block,
            to_block = %to_block,
            "Ethereum blockchain integration not yet implemented"
        );
        
        Ok(())
    }

    /// Handle a blockchain event
    async fn handle_blockchain_event(&self, event: BlockchainEvent, block_number: u64) -> Result<(), ApiError> {
        info!(block = %block_number, event = ?event, "Handling blockchain event");

        match event {
            BlockchainEvent::AlgorithmSubmitted { execution_id, scientist_wallet, algorithm_cid, dataset } => {
                self.handle_algorithm_submitted(execution_id, scientist_wallet, algorithm_cid, dataset).await?;
            }
            BlockchainEvent::DatasetRegistered { dataset_id, author_wallet, ipfs_cid, file_hash } => {
                self.handle_dataset_registered(dataset_id, author_wallet, ipfs_cid, file_hash).await?;
            }
            BlockchainEvent::VoteCast { execution_id, voter_wallet, decision } => {
                self.handle_vote_cast(execution_id, voter_wallet, decision).await?;
            }
            BlockchainEvent::CommitteeUpdated { wallet_address, is_active } => {
                self.handle_committee_updated(wallet_address, is_active).await?;
            }
            BlockchainEvent::TransactionConfirmed { tx_hash, block_number, block_timestamp } => {
                self.handle_transaction_confirmed(tx_hash, block_number, block_timestamp).await?;
            }
        }

        Ok(())
    }

    /// Handle algorithm submission event
    async fn handle_algorithm_submitted(
        &self,
        execution_id: u64,
        scientist_wallet: String,
        algorithm_cid: String,
        dataset: String,
    ) -> Result<(), ApiError> {
        info!(
            execution_id = %execution_id,
            scientist_wallet = %scientist_wallet,
            algorithm_cid = %algorithm_cid,
            dataset = %dataset,
            "Handling algorithm submission"
        );

        // Update database with confirmed algorithm submission
        // Record the blockchain-confirmed algorithm submission
        let query = r#"
            INSERT INTO blockchain_events (event_type, entity_type, entity_id, tx_hash, block_number, scientist_wallet, algorithm_cid, dataset, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            ON CONFLICT (entity_type, entity_id, event_type) DO UPDATE SET
                scientist_wallet = EXCLUDED.scientist_wallet,
                algorithm_cid = EXCLUDED.algorithm_cid,
                dataset = EXCLUDED.dataset,
                updated_at = NOW()
        "#;
        
        match sqlx::query(query)
            .bind("algorithm_submitted")
            .bind("algorithm")
            .bind(execution_id as i64)
            .bind(format!("algo_submit_{}", execution_id)) // tx_hash placeholder
            .bind(0i64) // block_number placeholder  
            .bind(scientist_wallet)
            .bind(algorithm_cid)
            .bind(dataset)
            .execute(&self.db_pool)
            .await
        {
            Ok(_) => {
                info!(execution_id = %execution_id, "Algorithm submission recorded in database");
            }
            Err(e) => {
                error!(error = %e, execution_id = %execution_id, "Failed to record algorithm submission");
                // Don't fail the blockchain sync for database errors - log and continue
                warn!("Continuing blockchain sync despite database error");
            }
        }

        Ok(())
    }

    /// Handle dataset registration event
    async fn handle_dataset_registered(
        &self,
        dataset_id: u64,
        author_wallet: String,
        ipfs_cid: String,
        file_hash: String,
    ) -> Result<(), ApiError> {
        info!(
            dataset_id = %dataset_id,
            author_wallet = %author_wallet,
            ipfs_cid = %ipfs_cid,
            file_hash = %file_hash,
            "Handling dataset registration"
        );

        // Update database with confirmed dataset registration
        let query = r#"
            INSERT INTO blockchain_events (event_type, entity_type, entity_id, tx_hash, block_number, author_wallet, ipfs_cid, file_hash, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            ON CONFLICT (entity_type, entity_id, event_type) DO UPDATE SET
                author_wallet = EXCLUDED.author_wallet,
                ipfs_cid = EXCLUDED.ipfs_cid,
                file_hash = EXCLUDED.file_hash,
                updated_at = NOW()
        "#;
        
        match sqlx::query(query)
            .bind("dataset_registered")
            .bind("dataset")
            .bind(dataset_id as i64)
            .bind(format!("dataset_reg_{}", dataset_id)) // tx_hash placeholder
            .bind(0i64) // block_number placeholder
            .bind(author_wallet)
            .bind(ipfs_cid)
            .bind(file_hash)
            .execute(&self.db_pool)
            .await
        {
            Ok(_) => {
                info!(dataset_id = %dataset_id, "Dataset registration recorded in database");
            }
            Err(e) => {
                error!(error = %e, dataset_id = %dataset_id, "Failed to record dataset registration");
                // Don't fail blockchain sync for database errors
                warn!("Continuing blockchain sync despite database error");
            }
        }

        Ok(())
    }

    /// Handle vote cast event
    async fn handle_vote_cast(
        &self,
        execution_id: u64,
        voter_wallet: String,
        decision: String,
    ) -> Result<(), ApiError> {
        info!(
            execution_id = %execution_id,
            voter_wallet = %voter_wallet,
            decision = %decision,
            "Handling vote cast"
        );

        // Record vote in database using the vote service
        let vote_request = crate::services::vote::CastVoteRequest {
            execution_id: execution_id as i64,
            voter_wallet: voter_wallet.clone(),
            decision: decision.clone(),
        };
        
        match VoteService::cast_vote(&self.db_pool, vote_request).await {
            Ok(_) => {
                info!(execution_id = %execution_id, voter = %voter_wallet, "Vote recorded in database");
                
                // Check if voting is complete
                match VoteService::is_voting_complete(&self.db_pool, execution_id as i64).await {
                    Ok(true) => {
                        info!(execution_id = %execution_id, "Voting completed for execution");
                        
                        // Trigger next phase of algorithm review process
                        match self.trigger_algorithm_review_completion(execution_id as i32).await {
                            Ok(_) => {
                                info!(execution_id = %execution_id, "Algorithm review completion triggered successfully");
                            }
                            Err(e) => {
                                error!(error = %e, execution_id = %execution_id, "Failed to trigger algorithm review completion");
                            }
                        }
                    }
                    Ok(false) => {
                        debug!(execution_id = %execution_id, "Voting still in progress");
                    }
                    Err(e) => {
                        warn!(error = %e, execution_id = %execution_id, "Failed to check voting completion");
                    }
                }
            }
            Err(e) => {
                error!(error = %e, execution_id = %execution_id, voter = %voter_wallet, "Failed to record vote");
                // Don't fail blockchain sync for database errors
                warn!("Continuing blockchain sync despite vote recording error");
            }
        }

        Ok(())
    }

    /// Handle committee update event
    async fn handle_committee_updated(
        &self,
        wallet_address: String,
        is_active: bool,
    ) -> Result<(), ApiError> {
        info!(
            wallet_address = %wallet_address,
            is_active = %is_active,
            "Handling committee update"
        );

        // Update committee member status in database using the committee service
        match CommitteeService::update_member_status(
            &self.db_pool,
            &wallet_address,
            is_active
        ).await {
            Ok(_) => {
                info!(
                    wallet = %wallet_address,
                    active = %is_active,
                    "Committee member status updated"
                );
            }
            Err(e) => {
                error!(
                    error = %e,
                    wallet = %wallet_address,
                    "Failed to update committee member status"
                );
                // Don't fail blockchain sync for database errors
                warn!("Continuing blockchain sync despite committee update error");
            }
        }

        Ok(())
    }

    /// Handle transaction confirmation event
    async fn handle_transaction_confirmed(
        &self,
        tx_hash: String,
        block_number: u64,
        block_timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), ApiError> {
        info!(
            tx_hash = %tx_hash,
            block_number = %block_number,
            block_timestamp = %block_timestamp,
            "Handling transaction confirmation"
        );

        // Update blockchain transaction status in database
        let query = r#"
            UPDATE blockchain_transactions 
            SET status = 'CONFIRMED', 
                block_number = $1, 
                block_timestamp = $2,
                updated_at = NOW()
            WHERE tx_hash = $3
        "#;
        
        match sqlx::query(query)
            .bind(block_number as i64)
            .bind(block_timestamp)
            .bind(&tx_hash)
            .execute(&self.db_pool)
            .await
        {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    info!(tx_hash = %tx_hash, block = %block_number, "Transaction confirmed in database");
                } else {
                    warn!(tx_hash = %tx_hash, "Transaction not found in database for confirmation");
                }
            }
            Err(e) => {
                error!(error = %e, tx_hash = %tx_hash, "Failed to confirm transaction in database");
                // Don't fail blockchain sync for database errors
                warn!("Continuing blockchain sync despite database error");
            }
        }

        // Also record this confirmation as an event for audit trail
        let event_query = r#"
            INSERT INTO blockchain_events (event_type, entity_type, entity_id, tx_hash, block_number, block_timestamp, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            ON CONFLICT (tx_hash, event_type) DO UPDATE SET
                block_number = EXCLUDED.block_number,
                block_timestamp = EXCLUDED.block_timestamp,
                updated_at = NOW()
        "#;
        
        match sqlx::query(event_query)
            .bind("transaction_confirmed")
            .bind("transaction")
            .bind(0i64) // entity_id for transaction confirmations
            .bind(&tx_hash)
            .bind(block_number as i64)
            .bind(block_timestamp)
            .execute(&self.db_pool)
            .await
        {
            Ok(_) => {
                debug!(tx_hash = %tx_hash, "Transaction confirmation event recorded");
            }
            Err(e) => {
                warn!(error = %e, tx_hash = %tx_hash, "Failed to record transaction confirmation event");
            }
        }

        Ok(())
    }

    /// Get the latest block number from the blockchain
    async fn get_latest_block_number(&self) -> Result<u64, ApiError> {
        // In production, this would make an RPC call to an Ethereum node
        // For mock, we'll just return a slightly advanced block number
        match self.config.blockchain.client_type.as_str() {
            "mock" => Ok(self.current_block + 10), // Simulate new blocks
            "ethereum" => {
                // TODO: Implement actual RPC call
                warn!("Ethereum get_latest_block_number not yet implemented, returning mock data");
                Ok(self.current_block + 10)
            }
            _ => Err(ApiError::ConfigurationError(
                format!("Unsupported blockchain client type: {}", self.config.blockchain.client_type)
            )),
        }
    }

    /// Get the last processed block number from the database
    async fn get_last_processed_block(&self) -> Result<u64, ApiError> {
        // Query from database to get the last processed block
        let query = r#"
            SELECT block_number 
            FROM blockchain_sync_state 
            WHERE sync_key = 'last_processed_block'
        "#;
        
        match sqlx::query_as::<_, (i64,)>(query)
            .fetch_optional(&self.db_pool)
            .await
        {
            Ok(Some((block_number,))) => {
                debug!(block = %block_number, "Retrieved last processed block from database");
                Ok(block_number as u64)
            }
            Ok(None) => {
                info!("No previous sync state found, starting from block 0");
                Ok(0)
            }
            Err(e) => {
                warn!(error = %e, "Failed to query last processed block, starting from 0");
                // Don't fail startup for database errors
                Ok(0)
            }
        }
    }

    /// Update the last processed block number in the database
    async fn update_last_processed_block(&self, block_number: u64) -> Result<(), ApiError> {
        // Update database with the last processed block
        let query = r#"
            INSERT INTO blockchain_sync_state (sync_key, block_number, updated_at)
            VALUES ('last_processed_block', $1, NOW())
            ON CONFLICT (sync_key) DO UPDATE SET
                block_number = EXCLUDED.block_number,
                updated_at = NOW()
        "#;
        
        match sqlx::query(query)
            .bind(block_number as i64)
            .execute(&self.db_pool)
            .await
        {
            Ok(_) => {
                debug!(block = %block_number, "Updated last processed block in database");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, block = %block_number, "Failed to update last processed block in database");
                Err(ApiError::DatabaseError(format!("Database error: {}", e)))
            }
        }
    }

    /// Trigger the next phase of algorithm review process when voting is complete
    async fn trigger_algorithm_review_completion(&self, execution_id: i32) -> Result<(), ApiError> {
        info!(execution_id = %execution_id, "Triggering algorithm review completion process");
        
        // Check if the execution was approved
        let is_approved = VoteService::is_execution_approved(&self.db_pool, execution_id as i64).await?;
        
        if is_approved {
            info!(execution_id = %execution_id, "Algorithm execution approved by committee");
            
            // Update execution status to approved and ready for execution
            let update_query = r#"
                UPDATE algorithm_executions 
                SET status = 'APPROVED', 
                    approved_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
            "#;
            
            match sqlx::query(update_query)
                .bind(execution_id)
                .execute(&self.db_pool)
                .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        info!(execution_id = %execution_id, "Algorithm execution marked as approved");
                        
                        // Record approval event for audit trail
                        let event_query = r#"
                            INSERT INTO blockchain_events (event_type, entity_type, entity_id, created_at)
                            VALUES ('algorithm_approved', 'algorithm', $1, NOW())
                        "#;
                        
                        if let Err(e) = sqlx::query(event_query)
                            .bind(execution_id as i64)
                            .execute(&self.db_pool)
                            .await
                        {
                            warn!(error = %e, execution_id = %execution_id, "Failed to record approval event");
                        }
                    } else {
                        warn!(execution_id = %execution_id, "No algorithm execution found to approve");
                    }
                }
                Err(e) => {
                    error!(error = %e, execution_id = %execution_id, "Failed to update algorithm execution status");
                    return Err(ApiError::DatabaseError(format!("Database error: {}", e)));
                }
            }
        } else {
            info!(execution_id = %execution_id, "Algorithm execution rejected by committee");
            
            // Update execution status to rejected
            let update_query = r#"
                UPDATE algorithm_executions 
                SET status = 'REJECTED',
                    rejected_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
            "#;
            
            match sqlx::query(update_query)
                .bind(execution_id)
                .execute(&self.db_pool)
                .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        info!(execution_id = %execution_id, "Algorithm execution marked as rejected");
                        
                        // Record rejection event for audit trail
                        let event_query = r#"
                            INSERT INTO blockchain_events (event_type, entity_type, entity_id, created_at)
                            VALUES ('algorithm_rejected', 'algorithm', $1, NOW())
                        "#;
                        
                        if let Err(e) = sqlx::query(event_query)
                            .bind(execution_id as i64)
                            .execute(&self.db_pool)
                            .await
                        {
                            warn!(error = %e, execution_id = %execution_id, "Failed to record rejection event");
                        }
                    } else {
                        warn!(execution_id = %execution_id, "No algorithm execution found to reject");
                    }
                }
                Err(e) => {
                    error!(error = %e, execution_id = %execution_id, "Failed to update algorithm execution status");
                    return Err(ApiError::DatabaseError(format!("Database error: {}", e)));
                }
            }
        }
        
        Ok(())
    }
} 