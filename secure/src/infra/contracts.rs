//! Blockchain contract interaction utilities
//!
//! This module provides functionality for interacting with DeLong protocol
//! smart contracts, including data contribution and algorithm review contracts.

use ethers::{
    abi::{Abi, Address},
    core::{
        k256::ecdsa::SigningKey,
        types::{
            BlockNumber, Filter, H256, Log, TransactionReceipt, TransactionRequest, U64, U256,
        },
    },
    middleware::SignerMiddleware,
    prelude::*,
    providers::{Http, Provider, Ws},
    signers::LocalWallet,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

use crate::infra::tee::{EthereumAccount, KeyVault};

/// Contract interaction errors
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Provider error: {0}")]
    ProviderError(#[from] ethers::providers::ProviderError),

    #[error("Contract error: {0}")]
    ContractCallError(String),

    #[error("ABI error: {0}")]
    AbiError(#[from] ethers::abi::Error),

    #[error("Signer error: {0}")]
    SignerError(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: U256, available: U256 },

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Result type for contract operations
pub type Result<T> = std::result::Result<T, ContractError>;

/// Contract addresses configuration
#[derive(Debug, Clone)]
pub struct ContractAddresses {
    pub data_contribution: Address,
    pub algorithm_review: Address,
}

/// Contract caller configuration
#[derive(Debug, Clone)]
pub struct ContractConfig {
    pub http_url: String,
    pub ws_url: String,
    pub chain_id: u64,
    pub addresses: ContractAddresses,
    pub funding_threshold_eth: f64,
    pub top_up_amount_eth: f64,
}

/// Contract event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractEvent {
    DataContribution {
        contributor: Address,
        data_id: U256,
        amount: U256,
    },
    AlgorithmSubmitted {
        submitter: Address,
        algo_id: U256,
        ipfs_hash: String,
    },
    AlgorithmReviewed {
        reviewer: Address,
        algo_id: U256,
        approved: bool,
    },
}

/// Contract caller for interacting with DeLong protocol contracts
#[derive(Clone)]
pub struct ContractCaller {
    config: ContractConfig,
    http_provider: Arc<Provider<Http>>,
    ws_provider: Option<Arc<Provider<Ws>>>,
    key_vault: Arc<KeyVault>,
    funding_account: Option<EthereumAccount>,
}

impl ContractCaller {
    /// Create a new contract caller
    pub async fn new(
        config: ContractConfig,
        key_vault: Arc<KeyVault>,
        funding_private_key: Option<String>,
    ) -> Result<Self> {
        // Create providers
        let http_provider = Provider::<Http>::try_from(&config.http_url)
            .map_err(|e| ContractError::InvalidConfig(format!("Invalid HTTP URL: {}", e)))?;
        let http_provider = Arc::new(http_provider);

        // Try to connect to WebSocket, but don't fail if it's not available (for testing)
        let ws_provider = match Provider::<Ws>::connect(&config.ws_url).await {
            Ok(provider) => {
                tracing::info!("WebSocket provider connected successfully");
                Some(Arc::new(provider))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to connect WebSocket provider: {}. Some features may be unavailable.",
                    e
                );
                None
            }
        };

        // Create funding account if private key provided
        let funding_account = if let Some(key_hex) = funding_private_key {
            let key_bytes = hex::decode(key_hex.trim_start_matches("0x"))
                .map_err(|e| ContractError::SignerError(format!("Invalid private key: {}", e)))?;
            let signing_key = SigningKey::from_slice(&key_bytes)
                .map_err(|e| ContractError::SignerError(e.to_string()))?;
            Some(EthereumAccount::from_private_key(signing_key))
        } else {
            None
        };

        Ok(Self {
            config,
            http_provider,
            ws_provider,
            key_vault,
            funding_account,
        })
    }

    /// Create a contract instance with signer
    async fn create_contract_with_signer(
        &self,
        address: Address,
        abi: Abi,
        signer_context: &str,
    ) -> Result<H256> {
        // Get signer account
        let account = self
            .key_vault
            .get_ethereum_account(signer_context)
            .await
            .map_err(|e| ContractError::SignerError(e.to_string()))?;

        // Create wallet with chain ID
        let wallet =
            LocalWallet::from(account.private_key.clone()).with_chain_id(self.config.chain_id);

        // For now, just return a dummy transaction hash
        // TODO: Implement actual contract interaction when ABI is available
        Ok(H256::zero())
    }

    /// Check and top up account balance if needed
    pub async fn ensure_sufficient_balance(&self, account_address: Address) -> Result<()> {
        if self.funding_account.is_none() {
            return Ok(()); // No funding account configured
        }

        let balance = self
            .http_provider
            .get_balance(account_address, None)
            .await?;
        let threshold = ethers::utils::parse_ether(self.config.funding_threshold_eth)
            .map_err(|e| ContractError::InvalidConfig(format!("Invalid threshold: {}", e)))?;

        if balance < threshold {
            info!(
                "Account {} balance {} below threshold {}, topping up",
                account_address, balance, threshold
            );

            let top_up_amount =
                ethers::utils::parse_ether(self.config.top_up_amount_eth).map_err(|e| {
                    ContractError::InvalidConfig(format!("Invalid top-up amount: {}", e))
                })?;

            self.transfer_funds(account_address, top_up_amount).await?;
        }

        Ok(())
    }

    /// Transfer funds from funding account
    async fn transfer_funds(&self, to: Address, amount: U256) -> Result<H256> {
        let funding_account = self.funding_account.as_ref().ok_or_else(|| {
            ContractError::InvalidConfig("No funding account configured".to_string())
        })?;

        // Check funding account balance
        let balance = self
            .http_provider
            .get_balance(funding_account.address, None)
            .await?;

        if balance < amount {
            return Err(ContractError::InsufficientBalance {
                required: amount,
                available: balance,
            });
        }

        // Create transaction
        let tx = TransactionRequest::new()
            .to(to)
            .value(amount)
            .from(funding_account.address);

        // Create signer
        let wallet = LocalWallet::from(funding_account.private_key.clone())
            .with_chain_id(self.config.chain_id);
        let client = SignerMiddleware::new(self.http_provider.clone(), wallet);

        // Send transaction
        let pending_tx = client
            .send_transaction(tx, None)
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;
        let tx_hash = pending_tx.tx_hash();

        info!("Sent {} ETH to {}, tx: {:?}", amount, to, tx_hash);

        // Wait for confirmation
        let receipt = pending_tx
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?
            .ok_or_else(|| ContractError::TransactionFailed("No receipt".to_string()))?;

        if receipt.status == Some(U64::from(0)) {
            return Err(ContractError::TransactionFailed(
                "Transaction reverted".to_string(),
            ));
        }

        Ok(tx_hash)
    }

    /// Submit data contribution
    pub async fn submit_data_contribution(
        &self,
        data_id: U256,
        ipfs_hash: &str,
        signer_context: &str,
    ) -> Result<TransactionReceipt> {
        // TODO: Implement actual contract call when ABI is available
        // For now, return a dummy receipt
        let dummy_receipt = TransactionReceipt {
            transaction_hash: H256::zero(),
            transaction_index: U64::zero(),
            block_hash: Some(H256::zero()),
            block_number: Some(U64::zero()),
            from: Address::zero(),
            to: Some(self.config.addresses.data_contribution),
            cumulative_gas_used: U256::zero(),
            gas_used: Some(U256::zero()),
            contract_address: None,
            logs: vec![],
            status: Some(U64::from(1)),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: None,
            effective_gas_price: None,
            other: Default::default(),
        };

        Ok(dummy_receipt)
    }

    /// Submit algorithm for review
    pub async fn submit_algorithm(
        &self,
        algo_id: u64,
        scientist_address: Address,
        cid: &str,
        dataset: &str,
    ) -> Result<TransactionReceipt> {
        // TODO: Implement actual contract call when ABI is available
        // For now, return a dummy receipt
        let dummy_receipt = TransactionReceipt {
            transaction_hash: H256::zero(),
            transaction_index: U64::zero(),
            block_hash: Some(H256::zero()),
            block_number: Some(U64::zero()),
            from: Address::zero(),
            to: Some(scientist_address),
            cumulative_gas_used: U256::zero(),
            gas_used: Some(U256::zero()),
            contract_address: None,
            logs: vec![],
            status: Some(U64::from(1)),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: None,
            effective_gas_price: None,
            other: Default::default(),
        };

        Ok(dummy_receipt)
    }

    /// Add a committee member
    pub async fn add_committee_member(
        &self,
        member_address: Address,
    ) -> Result<TransactionReceipt> {
        // TODO: Implement actual contract call when ABI is available

        // For now, return a dummy transaction receipt
        let dummy_receipt = TransactionReceipt {
            transaction_hash: H256::random(),
            transaction_index: U64::from(0),
            block_hash: Some(H256::zero()),
            block_number: Some(U64::zero()),
            from: Address::zero(),
            to: Some(member_address),
            cumulative_gas_used: U256::zero(),
            gas_used: Some(U256::zero()),
            contract_address: None,
            logs: vec![],
            status: Some(U64::from(1)),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: None,
            effective_gas_price: None,
            other: Default::default(),
        };

        Ok(dummy_receipt)
    }

    /// Remove a committee member
    pub async fn remove_committee_member(
        &self,
        member_address: Address,
    ) -> Result<TransactionReceipt> {
        // TODO: Implement actual contract call when ABI is available

        // For now, return a dummy transaction receipt
        let dummy_receipt = TransactionReceipt {
            transaction_hash: H256::random(),
            transaction_index: U64::from(0),
            block_hash: Some(H256::zero()),
            block_number: Some(U64::zero()),
            from: Address::zero(),
            to: Some(member_address),
            cumulative_gas_used: U256::zero(),
            gas_used: Some(U256::zero()),
            contract_address: None,
            logs: vec![],
            status: Some(U64::from(1)),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: None,
            effective_gas_price: None,
            other: Default::default(),
        };

        Ok(dummy_receipt)
    }

    /// Submit a vote for algorithm review
    pub async fn submit_vote(
        &self,
        algo_exe_id: u64,
        vote: bool,
        voter_address: Address,
    ) -> Result<TransactionReceipt> {
        // TODO: Implement actual contract call when ABI is available

        // For now, return a dummy transaction receipt
        let dummy_receipt = TransactionReceipt {
            transaction_hash: H256::random(),
            transaction_index: U64::from(0),
            block_hash: Some(H256::zero()),
            block_number: Some(U64::zero()),
            from: voter_address,
            to: Some(self.config.addresses.algorithm_review),
            cumulative_gas_used: U256::zero(),
            gas_used: Some(U256::zero()),
            contract_address: None,
            logs: vec![],
            status: Some(U64::from(1)),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: None,
            effective_gas_price: None,
            other: Default::default(),
        };

        Ok(dummy_receipt)
    }

    /// Set voting duration
    pub async fn set_voting_duration(&self, duration: u64) -> Result<TransactionReceipt> {
        // TODO: Implement actual contract call when ABI is available

        // For now, return a dummy transaction receipt
        let dummy_receipt = TransactionReceipt {
            transaction_hash: H256::random(),
            transaction_index: U64::from(0),
            block_hash: Some(H256::zero()),
            block_number: Some(U64::zero()),
            from: Address::zero(),
            to: Some(self.config.addresses.algorithm_review),
            cumulative_gas_used: U256::zero(),
            gas_used: Some(U256::zero()),
            contract_address: None,
            logs: vec![],
            status: Some(U64::from(1)),
            root: None,
            logs_bloom: Default::default(),
            transaction_type: None,
            effective_gas_price: None,
            other: Default::default(),
        };

        Ok(dummy_receipt)
    }

    /// Watch for contract events
    pub async fn watch_events<F>(&self, from_block: Option<BlockNumber>, callback: F) -> Result<()>
    where
        F: Fn(ContractEvent) + Send + Sync + 'static,
    {
        // Check if WebSocket provider is available
        let ws_provider = self.ws_provider.as_ref().ok_or_else(|| {
            ContractError::InvalidConfig("WebSocket provider not available".to_string())
        })?;

        // Create event filters
        let data_contribution_filter = Filter::new()
            .address(self.config.addresses.data_contribution)
            .from_block(from_block.unwrap_or(BlockNumber::Latest));

        let algorithm_review_filter = Filter::new()
            .address(self.config.addresses.algorithm_review)
            .from_block(from_block.unwrap_or(BlockNumber::Latest));

        // Subscribe to events
        let mut data_stream = ws_provider
            .subscribe_logs(&data_contribution_filter)
            .await
            .map_err(|e| ContractError::ProviderError(e))?;
        let mut algo_stream = ws_provider
            .subscribe_logs(&algorithm_review_filter)
            .await
            .map_err(|e| ContractError::ProviderError(e))?;

        // Process events
        loop {
            tokio::select! {
                Some(log) = data_stream.next() => {
                    if let Err(e) = self.process_log(log, &callback) {
                        error!("Error processing data contribution log: {}", e);
                    }
                }
                Some(log) = algo_stream.next() => {
                    if let Err(e) = self.process_log(log, &callback) {
                        error!("Error processing algorithm review log: {}", e);
                    }
                }
                else => break,
            }
        }

        Ok(())
    }

    /// Process a log entry
    fn process_log<F>(&self, log: Log, _callback: &F) -> Result<()>
    where
        F: Fn(ContractEvent),
    {
        // TODO: Decode log based on event signature
        // This is a placeholder implementation
        info!("Processing log: {:?}", log);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_addresses() {
        let addresses = ContractAddresses {
            data_contribution: "0x0000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
            algorithm_review: "0x0000000000000000000000000000000000000002"
                .parse()
                .unwrap(),
        };

        assert_eq!(
            format!("{:?}", addresses.data_contribution),
            "0x0000000000000000000000000000000000000001"
        );
    }
}
