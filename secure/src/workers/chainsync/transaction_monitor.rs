//! Transaction Monitor Worker
//!
//! This worker monitors pending blockchain transactions via Redis queue
//! and replaces stuck transactions to prevent nonce blocking.
//!
//! ## Why this exists:
//! When blockchain transactions get stuck (due to low gas price or network congestion),
//! they block all subsequent transactions from the same wallet because of nonce ordering.
//! This worker detects stuck transactions and replaces them with empty high-gas transactions
//! to unblock the nonce sequence.
//!
//! ## How it works:
//! 1. Handlers add pending transactions to a Redis queue when submitting to blockchain
//! 2. This worker continuously monitors the queue (FIFO processing)
//! 3. For each transaction, it checks:
//!    - If timeout exceeded: Replace with empty transaction and mark as failed
//!    - If confirmed on chain: Update database status
//!    - If failed on chain: Mark as failed in database
//!    - If still pending: Re-queue for later checking
//!
//! ## Configuration:
//! - `timeout_seconds`: How long to wait before considering a transaction stuck (default: 120s)
//! - `gas_multiplier`: Gas price increase percentage for replacement tx (default: 120 = 20% increase)
//! - `idle_sleep_seconds`: How long to sleep when queue is empty (default: 5s)

use alloy::consensus::Transaction;
use alloy::network::TransactionBuilder;
use alloy::primitives::{FixedBytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use chrono::{DateTime, Duration, Utc};
use deadpool_redis::{redis::AsyncCommands, Pool};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::infra::{ContractCaller, Database};
use crate::models::blockchain_transaction::{BlockchainTransaction, EntityType, TransactionStatus};
use crate::{AppError, Result as AppResult};

/// Pending transaction information stored in Redis queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    pub tx_hash: String,
    pub entity_id: i64,
    pub entity_type: EntityType,
    pub nonce: u64,
    pub created_at: DateTime<Utc>,
}

/// Transaction status from chain check
#[derive(Debug)]
enum TxStatus {
    Confirmed(u64),
    Failed,
    StillPending,
}

/// Transaction monitor configuration
#[derive(Debug, Clone)]
pub struct TransactionMonitorConfig {
    /// Timeout before replacing transaction (in seconds)
    pub timeout_seconds: u64,
    /// Gas price multiplier for replacement (e.g., 120 = 20% increase)
    pub gas_multiplier: u64,
    /// Sleep duration when queue is empty (in seconds)
    pub idle_sleep_seconds: u64,
}

impl Default for TransactionMonitorConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 120,  // 2 minutes
            gas_multiplier: 120,   // 20% increase
            idle_sleep_seconds: 5, // 5 seconds
        }
    }
}

/// Transaction monitor that uses Redis queue
pub struct TransactionMonitor {
    /// Database connection
    db: Arc<Database>,
    /// Contract caller for blockchain operations
    contract_caller: Arc<ContractCaller>,
    /// Redis connection pool
    redis_pool: Pool,
    /// Configuration
    config: TransactionMonitorConfig,
}

impl TransactionMonitor {
    /// Redis queue key for pending transactions
    pub const PENDING_TX_QUEUE: &'static str = "delong:pending_txs";

    /// Create a new transaction monitor
    pub fn new(
        db: Arc<Database>,
        contract_caller: Arc<ContractCaller>,
        redis_pool: Pool,
        config: TransactionMonitorConfig,
    ) -> Self {
        Self {
            db,
            contract_caller,
            redis_pool,
            config,
        }
    }

    /// Start the transaction monitoring loop
    ///
    /// This is the main loop that continuously:
    /// 1. Pops pending transactions from Redis queue (FIFO - oldest first)
    /// 2. Processes each transaction to check status or handle timeouts
    /// 3. Re-queues transactions that are still pending
    /// 4. Sleeps when the queue is empty
    ///
    /// The loop runs indefinitely and is designed to be run as a background worker.
    /// It prevents blockchain nonce blocking by detecting and replacing stuck transactions.
    pub async fn monitor_loop(&self) -> AppResult<()> {
        info!(
            "Starting transaction monitor with {}s timeout",
            self.config.timeout_seconds
        );

        loop {
            // Get a connection from the pool
            let mut conn = self.redis_pool.get().await.map_err(|e| {
                AppError::Internal(format!("Failed to get redis connection: {}", e))
            })?;

            // Pop from the right (oldest transaction)
            let tx_data: Option<String> = conn
                .rpop(Self::PENDING_TX_QUEUE, None)
                .await
                .map_err(|e| AppError::Internal(format!("Redis RPOP failed: {}", e)))?;

            if let Some(data) = tx_data {
                // Parse pending transaction
                let pending_tx: PendingTransaction = serde_json::from_str(&data).map_err(|e| {
                    AppError::Internal(format!("Failed to parse pending tx: {}", e))
                })?;

                // Process the transaction
                if let Err(e) = self.process_pending_transaction(&pending_tx).await {
                    error!(
                        "Failed to process transaction {}: {}",
                        pending_tx.tx_hash, e
                    );
                    // Put it back to the queue for retry
                    let _: () = conn
                        .lpush(Self::PENDING_TX_QUEUE, &data)
                        .await
                        .map_err(|e| AppError::Internal(format!("Redis LPUSH failed: {}", e)))?;
                }
            } else {
                // Queue is empty, sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_secs(
                    self.config.idle_sleep_seconds,
                ))
                .await;
            }
        }
    }

    /// Process a single pending transaction from the monitoring queue
    ///
    /// This function performs the following tasks:
    /// 1. Checks if the transaction has exceeded the timeout threshold
    ///    - If yes: Replaces it with an empty transaction to unblock the nonce
    ///    - Marks the transaction as failed in the database
    ///
    /// 2. If not timed out, checks the transaction status on the blockchain:
    ///    - Confirmed: Updates database with block number and timestamp
    ///    - Failed: Marks as failed in the database
    ///    - Still Pending: Re-queues the transaction for later checking
    ///
    /// The main purpose is to prevent nonce blocking by detecting stuck transactions
    /// and replacing them with empty transactions that use the same nonce but higher gas.
    async fn process_pending_transaction(&self, pending_tx: &PendingTransaction) -> AppResult<()> {
        let age = Utc::now().signed_duration_since(pending_tx.created_at);

        if age > Duration::seconds(self.config.timeout_seconds as i64) {
            // Transaction is stuck, replace it
            info!(
                "Transaction {} stuck for {}s, replacing with empty tx",
                pending_tx.tx_hash,
                age.num_seconds()
            );

            // Replace the transaction
            if let Err(e) = self.replace_with_empty_tx(pending_tx).await {
                error!(
                    "Failed to replace transaction {}: {}",
                    pending_tx.tx_hash, e
                );
                // Even if replacement fails, mark as failed to unblock
            }

            // Mark as failed in database
            BlockchainTransaction::update_status(
                self.db.pool(),
                &pending_tx.tx_hash,
                TransactionStatus::Failed,
                None,
                None,
            )
            .await?;

            info!(
                "Transaction {} marked as failed due to timeout",
                pending_tx.tx_hash
            );
        } else {
            // Check current status on chain
            match self.check_transaction_status(&pending_tx.tx_hash).await? {
                TxStatus::Confirmed(block_number) => {
                    info!(
                        "Transaction {} confirmed in block {}",
                        pending_tx.tx_hash, block_number
                    );

                    // Update database
                    BlockchainTransaction::update_status(
                        self.db.pool(),
                        &pending_tx.tx_hash,
                        TransactionStatus::Confirmed,
                        Some(block_number),
                        Some(Utc::now()),
                    )
                    .await?;
                }
                TxStatus::Failed => {
                    warn!("Transaction {} failed on chain", pending_tx.tx_hash);

                    // Update database
                    BlockchainTransaction::update_status(
                        self.db.pool(),
                        &pending_tx.tx_hash,
                        TransactionStatus::Failed,
                        None,
                        None,
                    )
                    .await?;
                }
                TxStatus::StillPending => {
                    // Still pending, put back to queue (at the left/head for FIFO)
                    let data = serde_json::to_string(pending_tx)?;

                    let mut conn = self.redis_pool.get().await.map_err(|e| {
                        AppError::Internal(format!("Failed to get redis connection: {}", e))
                    })?;

                    let _: () = conn
                        .lpush(Self::PENDING_TX_QUEUE, data)
                        .await
                        .map_err(|e| AppError::Internal(format!("Redis LPUSH failed: {}", e)))?;

                    info!(
                        "Transaction {} still pending after {}s, re-queued",
                        pending_tx.tx_hash,
                        age.num_seconds()
                    );
                }
            }
        }

        Ok(())
    }

    /// Replace stuck transaction with empty high-gas transaction
    async fn replace_with_empty_tx(&self, pending_tx: &PendingTransaction) -> AppResult<()> {
        // Parse transaction hash
        let hash = FixedBytes::<32>::from_str(&pending_tx.tx_hash)
            .map_err(|e| AppError::Internal(format!("Invalid tx hash: {}", e)))?;

        // Get provider
        let provider = self.contract_caller.provider().await?;

        // Get original transaction
        let original = provider
            .get_transaction_by_hash(hash)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get transaction: {}", e)))?
            .ok_or_else(|| AppError::Internal("Transaction not found".to_string()))?;

        // Get nonce from original transaction
        let nonce = original.inner.nonce();

        // Get gas price and increase by configured multiplier
        let original_gas_price = original
            .inner
            .gas_price()
            .ok_or_else(|| AppError::Internal("No gas price in original tx".to_string()))?;
        let new_gas_price = original_gas_price * (self.config.gas_multiplier as u128) / 100;

        // Get the wallet address
        let wallet_address = self.contract_caller.wallet_address();

        info!(
            "Replacing tx with nonce {} - original gas: {}, new gas: {}",
            nonce, original_gas_price, new_gas_price
        );

        // Build replacement transaction: empty transaction to self with higher gas
        let tx = TransactionRequest::default()
            .with_to(wallet_address)
            .with_value(U256::ZERO)
            .with_nonce(nonce)
            .with_gas_price(new_gas_price)
            .with_gas_limit(21000); // Standard transfer gas limit

        // Send the replacement transaction
        let replacement_hash = self.contract_caller.send_raw_transaction(tx).await?;

        info!(
            "Successfully replaced stuck transaction {} with {} (nonce: {})",
            pending_tx.tx_hash, replacement_hash, nonce
        );

        Ok(())
    }

    /// Check transaction status on chain
    async fn check_transaction_status(&self, tx_hash: &str) -> AppResult<TxStatus> {
        // Parse transaction hash
        let hash = FixedBytes::<32>::from_str(tx_hash)
            .map_err(|e| AppError::Internal(format!("Invalid tx hash: {}", e)))?;

        // Get provider
        let provider = self.contract_caller.provider().await?;

        // Check for receipt first
        match provider.get_transaction_receipt(hash).await {
            Ok(Some(receipt)) => {
                // Transaction has a receipt
                if receipt.status() {
                    // Success
                    if let Some(block_number) = receipt.block_number {
                        Ok(TxStatus::Confirmed(block_number))
                    } else {
                        // Shouldn't happen but treat as pending
                        Ok(TxStatus::StillPending)
                    }
                } else {
                    // Transaction reverted
                    Ok(TxStatus::Failed)
                }
            }
            Ok(None) => {
                // No receipt yet, check if transaction exists
                match provider.get_transaction_by_hash(hash).await {
                    Ok(Some(_)) => {
                        // Transaction exists but not mined
                        Ok(TxStatus::StillPending)
                    }
                    Ok(None) => {
                        // Transaction not found
                        Ok(TxStatus::Failed)
                    }
                    Err(e) => {
                        warn!("Error checking transaction {}: {}", tx_hash, e);
                        // Assume still pending on error
                        Ok(TxStatus::StillPending)
                    }
                }
            }
            Err(e) => {
                warn!("Error getting receipt for {}: {}", tx_hash, e);
                // Assume still pending on error
                Ok(TxStatus::StillPending)
            }
        }
    }

    /// Add a transaction to the monitoring queue (static method for use in handlers)
    pub async fn add_to_queue(redis_pool: &Pool, pending_tx: PendingTransaction) -> AppResult<()> {
        let mut conn = redis_pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get redis connection: {}", e)))?;

        let data = serde_json::to_string(&pending_tx)?;

        let _: () = conn
            .lpush(Self::PENDING_TX_QUEUE, data)
            .await
            .map_err(|e| AppError::Internal(format!("Redis LPUSH failed: {}", e)))?;

        info!(
            "Transaction {} added to monitoring queue",
            pending_tx.tx_hash
        );
        Ok(())
    }

    /// Get queue length (for monitoring/debugging)
    pub async fn get_queue_length(&self) -> AppResult<usize> {
        let mut conn =
            self.redis_pool.get().await.map_err(|e| {
                AppError::Internal(format!("Failed to get redis connection: {}", e))
            })?;

        let len: usize = conn
            .llen(Self::PENDING_TX_QUEUE)
            .await
            .map_err(|e| AppError::Internal(format!("Redis LLEN failed: {}", e)))?;

        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadpool_redis::Config as RedisConfig;

    #[test]
    fn test_default_config() {
        let config = TransactionMonitorConfig::default();
        assert_eq!(config.timeout_seconds, 120);
        assert_eq!(config.gas_multiplier, 120);
        assert_eq!(config.idle_sleep_seconds, 5);
    }

    #[test]
    fn test_pending_transaction_serialization() {
        let tx = PendingTransaction {
            tx_hash: "0x123".to_string(),
            entity_id: 1,
            entity_type: EntityType::Dataset,
            nonce: 42,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&tx).unwrap();
        let deserialized: PendingTransaction = serde_json::from_str(&json).unwrap();

        assert_eq!(tx.tx_hash, deserialized.tx_hash);
        assert_eq!(tx.entity_id, deserialized.entity_id);
        assert_eq!(tx.nonce, deserialized.nonce);
    }

    #[tokio::test]
    #[ignore = "Requires Redis to be running"]
    async fn test_add_to_queue() {
        // This test requires Redis to be running
        // Run with: cargo test test_add_to_queue -- --ignored
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:11002".to_string());

        let redis_config = RedisConfig::from_url(redis_url);
        let pool = redis_config
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("Failed to create Redis pool");

        // Test adding a transaction to queue
        let tx = PendingTransaction {
            tx_hash: format!("0xtest_{}", Utc::now().timestamp()),
            entity_id: 999,
            entity_type: EntityType::Dataset,  // Using a valid EntityType
            nonce: 1,
            created_at: Utc::now(),
        };

        // Add to queue
        let result = TransactionMonitor::add_to_queue(&pool, tx.clone()).await;
        assert!(result.is_ok(), "Failed to add transaction to queue");

        // Verify it was added by popping it
        let mut conn = pool.get().await.unwrap();
        let data: Option<String> = conn
            .rpop(TransactionMonitor::PENDING_TX_QUEUE, None)
            .await
            .unwrap();

        assert!(data.is_some(), "Transaction not found in queue");
        let retrieved: PendingTransaction = serde_json::from_str(&data.unwrap()).unwrap();
        assert_eq!(retrieved.tx_hash, tx.tx_hash);
        assert_eq!(retrieved.entity_id, tx.entity_id);
    }

    #[tokio::test]
    async fn test_timeout_detection() {
        // Create a transaction that's older than timeout
        let old_tx = PendingTransaction {
            tx_hash: "0xold123".to_string(),
            entity_id: 1,
            entity_type: EntityType::Dataset,
            nonce: 1,
            created_at: Utc::now() - Duration::seconds(150), // 2.5 minutes old
        };

        let config = TransactionMonitorConfig::default();
        assert_eq!(config.timeout_seconds, 120);

        // Check if transaction is considered timed out
        let age = Utc::now().signed_duration_since(old_tx.created_at);
        assert!(age > Duration::seconds(config.timeout_seconds as i64));
    }

    #[test]
    fn test_gas_price_calculation() {
        let config = TransactionMonitorConfig::default();
        let original_gas: u128 = 1_000_000_000; // 1 gwei
        let new_gas = original_gas * (config.gas_multiplier as u128) / 100;
        assert_eq!(new_gas, 1_200_000_000); // 1.2 gwei (20% increase)
    }
}
