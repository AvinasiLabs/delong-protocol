//! Smart contract interaction module
//!
//! This module provides functionality for interacting with blockchain smart contracts
//! using the alloy library.

use crate::config::ChainConfig;
use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{BlockNumberOrTag, Filter, Log},
    signers::local::PrivateKeySigner,
    sol,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info};

// Load contract ABIs
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    DataContribution,
    "abi/DataContribution.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    AlgorithmReview,
    "abi/AlgorithmReview.json"
);

/// Contract-related errors
#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    /// Provider error
    #[error("Provider error: {0}")]
    ProviderError(String),

    /// Contract call error
    #[error("Contract call error: {0}")]
    ContractCallError(String),

    /// Signer error
    #[error("Signer error: {0}")]
    SignerError(String),

    /// Insufficient balance
    #[error("Insufficient balance: required {required} ETH, available {available} ETH")]
    InsufficientBalance { required: f64, available: f64 },

    /// Transaction failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Alloy error
    #[error("Alloy error: {0}")]
    AlloyError(#[from] alloy::contract::Error),
}

impl From<alloy::transports::TransportError> for ContractError {
    fn from(err: alloy::transports::TransportError) -> Self {
        ContractError::ProviderError(err.to_string())
    }
}

/// Result type for contract operations
pub type Result<T> = std::result::Result<T, ContractError>;

/// Contract addresses holder
#[derive(Debug, Clone)]
pub struct ContractAddresses {
    pub data_contribution: Address,
    pub algorithm_review: Address,
}

/// Contract caller for blockchain interactions
#[derive(Clone)]
pub struct ContractCaller {
    config: ChainConfig,
    wallet: EthereumWallet,
    addresses: ContractAddresses,
}

impl ContractCaller {
    /// Create a new contract caller instance
    pub async fn new(config: ChainConfig) -> Result<Self> {
        // Get private key from config or use a default for development
        let private_key = config.private_key.clone().ok_or_else(|| {
            ContractError::InvalidConfig("OFFICIAL_ACCOUNT_PRIVATE_KEY not set".to_string())
        })?;

        let signer = PrivateKeySigner::from_str(&private_key)
            .map_err(|e| ContractError::SignerError(e.to_string()))?;
        let wallet = EthereumWallet::from(signer);

        // Parse contract addresses from config
        let addresses = ContractAddresses {
            data_contribution: Address::from_str(&config.data_contribution_address)
                .map_err(|e| ContractError::ParseError(e.to_string()))?,
            algorithm_review: Address::from_str(&config.algorithm_review_address)
                .map_err(|e| ContractError::ParseError(e.to_string()))?,
        };

        Ok(ContractCaller {
            config,
            wallet,
            addresses,
        })
    }

    /// Create a provider instance
    async fn create_provider(&self) -> Result<impl Provider> {
        let provider_url = &self.config.rpc_url;

        let provider = ProviderBuilder::new()
            .wallet(self.wallet.clone())
            .connect(provider_url)
            .await
            .map_err(|e| ContractError::ProviderError(format!("Failed to connect: {}", e)))?;

        Ok(provider)
    }

    /// Ensure the wallet has sufficient balance
    pub async fn ensure_sufficient_balance(&self, required_eth: f64) -> Result<()> {
        let provider = self.create_provider().await?;
        let signer = self.wallet.default_signer();
        let address = signer.address();
        let balance = provider
            .get_balance(address)
            .await
            .map_err(|e| ContractError::ProviderError(e.to_string()))?;
        let balance_eth = balance.to::<u128>() as f64 / 1e18;

        if balance_eth < required_eth {
            return Err(ContractError::InsufficientBalance {
                required: required_eth,
                available: balance_eth,
            });
        }

        debug!(
            "Wallet balance: {} ETH (required: {} ETH)",
            balance_eth, required_eth
        );
        Ok(())
    }

    /// Record data usage on the blockchain
    pub async fn record_data_usage(
        &self,
        scientist_wallet: Address,
        algo_cid: String,
        dataset_name: String,
    ) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = DataContribution::new(self.addresses.data_contribution, provider);

        let when = U256::from(chrono::Utc::now().timestamp() as u64);
        let call = contract.recordUsage(scientist_wallet, algo_cid.clone(), dataset_name, when);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!("Data usage recorded: algo_cid={}, tx={}", algo_cid, tx_hash);

        Ok(tx_hash)
    }

    /// Submit an algorithm for review
    pub async fn submit_algorithm(
        &self,
        scientist_wallet: Address,
        algorithm_cid: String,
        dataset_name: String,
    ) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        // TODO: Get proper execution_id from somewhere
        let execution_id = U256::from(1);
        let call = contract.submitAlgorithm(
            execution_id,
            scientist_wallet,
            algorithm_cid.clone(),
            dataset_name,
        );

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!("Algorithm submitted: cid={}, tx={}", algorithm_cid, tx_hash);

        Ok(tx_hash)
    }

    /// Add a committee member
    pub async fn add_committee_member(&self, member: Address) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let call = contract.setCommitteeMember(member, true);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!("Committee member added: member={}, tx={}", member, tx_hash);

        Ok(tx_hash)
    }

    /// Remove a committee member
    pub async fn remove_committee_member(&self, member: Address) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let call = contract.setCommitteeMember(member, false);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!(
            "Committee member removed: member={}, tx={}",
            member, tx_hash
        );

        Ok(tx_hash)
    }

    /// Set voting duration
    pub async fn set_voting_duration(&self, duration_seconds: U256) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let call = contract.setVotingDuration(duration_seconds);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!(
            "Voting duration set: duration={}, tx={}",
            duration_seconds, tx_hash
        );

        Ok(tx_hash)
    }

    /// Vote on an algorithm
    pub async fn vote_on_algorithm(&self, algorithm_cid: String, vote: bool) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let call = contract.vote(algorithm_cid.clone(), vote);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!(
            "Vote cast: algorithm_cid={}, vote={}, tx={}",
            algorithm_cid, vote, tx_hash
        );

        Ok(tx_hash)
    }

    /// Resolve an algorithm review
    pub async fn resolve_algorithm(
        &self,
        algorithm_cid: String,
        execution_id: U256,
    ) -> Result<String> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let call = contract.resolve(algorithm_cid.clone(), execution_id);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!("Algorithm resolved: cid={}, tx={}", algorithm_cid, tx_hash);

        Ok(tx_hash)
    }

    /// Check if an address is a committee member
    pub async fn is_committee_member(&self, address: Address) -> Result<bool> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let is_member = contract.isCommitteeMember(address).call().await?;
        Ok(is_member)
    }

    /// Get the contract owner
    pub async fn get_contract_owner(&self) -> Result<Address> {
        let provider = self.create_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let owner = contract.owner().call().await?;
        Ok(owner)
    }

    /// Ensure contracts are deployed and accessible
    /// If contracts are not deployed, deploy them using the configured wallet
    pub async fn ensure_contracts_deployed(&mut self) -> Result<()> {
        let provider = self.create_provider().await?;

        // Check DataContribution contract
        let data_code = provider
            .get_code_at(self.addresses.data_contribution)
            .await?;

        let data_contribution_address = if data_code.is_empty() {
            info!("DataContribution contract not found, deploying...");

            // Deploy DataContribution contract
            let deploy_provider = self.create_provider().await?;
            let deploy_tx = DataContribution::deploy(deploy_provider)
                .await
                .map_err(|e| {
                    ContractError::ContractCallError(format!(
                        "Failed to deploy DataContribution: {}",
                        e
                    ))
                })?;

            let deployed_address = deploy_tx.address().clone();
            info!(
                "DataContribution contract deployed at: {:?}",
                deployed_address
            );

            // Store contract address in database for future use
            self.store_contract_address("DataContribution", deployed_address)
                .await?;

            deployed_address
        } else {
            info!(
                "DataContribution contract already deployed at: {:?}",
                self.addresses.data_contribution
            );
            self.addresses.data_contribution
        };

        // Check AlgorithmReview contract
        let algo_code = provider
            .get_code_at(self.addresses.algorithm_review)
            .await?;

        let algorithm_review_address = if algo_code.is_empty() {
            info!("AlgorithmReview contract not found, deploying...");

            // Deploy AlgorithmReview contract
            let deploy_provider = self.create_provider().await?;
            let deploy_tx = AlgorithmReview::deploy(deploy_provider)
                .await
                .map_err(|e| {
                    ContractError::ContractCallError(format!(
                        "Failed to deploy AlgorithmReview: {}",
                        e
                    ))
                })?;

            let deployed_address = deploy_tx.address().clone();
            info!(
                "AlgorithmReview contract deployed at: {:?}",
                deployed_address
            );

            // Store contract address in database for future use
            self.store_contract_address("AlgorithmReview", deployed_address)
                .await?;

            deployed_address
        } else {
            info!(
                "AlgorithmReview contract already deployed at: {:?}",
                self.addresses.algorithm_review
            );
            self.addresses.algorithm_review
        };

        // Update addresses if they were deployed
        if data_code.is_empty() {
            self.addresses.data_contribution = data_contribution_address;
        }
        if algo_code.is_empty() {
            self.addresses.algorithm_review = algorithm_review_address;
        }

        info!("All contracts are deployed and accessible");
        Ok(())
    }

    /// Store contract address in database
    async fn store_contract_address(&self, name: &str, address: Address) -> Result<()> {
        // This would typically store the contract address in the database
        // using the contract_metas table
        info!("Storing contract {} at address {:?}", name, address);

        // TODO: Implement database storage when Database connection is available
        // sqlx::query!(
        //     "INSERT INTO contract_metas (name, address) VALUES ($1, $2)
        //      ON CONFLICT (name) DO UPDATE SET address = $2",
        //     name,
        //     format!("{:?}", address)
        // )
        // .execute(&self.db)
        // .await
        // .map_err(|e| ContractError::ContractCallError(format!("Failed to store contract address: {}", e)))?;

        Ok(())
    }

    /// Subscribe to contract events
    pub async fn subscribe_events<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(Log) + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);

        // Create a combined filter for all events
        let filter = Filter::new()
            .address(vec![
                self.addresses.data_contribution,
                self.addresses.algorithm_review,
            ])
            .from_block(BlockNumberOrTag::Latest);

        // Subscribe to logs
        let provider = self.create_provider().await?;
        let sub = provider
            .subscribe_logs(&filter)
            .await
            .map_err(|e| ContractError::ProviderError(e.to_string()))?;

        // Spawn a task to handle events
        tokio::spawn(async move {
            let mut stream = sub.into_stream();
            use futures::StreamExt;
            while let Some(log) = stream.next().await {
                callback(log);
            }
        });

        info!("Subscribed to contract events");
        Ok(())
    }

    /// Get past events from the contracts
    pub async fn get_past_events(
        &self,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> Result<Vec<Log>> {
        let from = from_block
            .map(BlockNumberOrTag::Number)
            .unwrap_or(BlockNumberOrTag::Earliest);
        let to = to_block
            .map(BlockNumberOrTag::Number)
            .unwrap_or(BlockNumberOrTag::Latest);

        // Create a combined filter for all events
        let filter = Filter::new().from_block(from).to_block(to).address(vec![
            self.addresses.data_contribution,
            self.addresses.algorithm_review,
        ]);

        let provider = self.create_provider().await?;
        let logs = provider.get_logs(&filter).await?;
        Ok(logs)
    }

    /// Parse a log into a specific event type
    pub fn parse_event(&self, log: &Log) -> Result<ParsedEvent> {
        // Check the first topic (event signature)
        let event_sig = log
            .topics()
            .get(0)
            .ok_or_else(|| ContractError::ParseError("No event signature in log".to_string()))?;

        // DataContribution events
        if log.address() == self.addresses.data_contribution {
            // DataRegistered(address indexed contributor, string indexed cid, string dataset)
            if event_sig == &alloy::primitives::keccak256("DataRegistered(address,string,string)") {
                let contributor = Address::from_slice(&log.topics()[1][12..]);
                // For now, return a simplified version
                return Ok(ParsedEvent::DataRegistered {
                    data_hash: format!("0x{}", hex::encode(&log.topics()[2])),
                    provider: contributor,
                    price: U256::ZERO, // Price not in this event
                });
            }
            // DataUsed(address indexed scientist, string indexed cid, string dataset, uint256 when)
            if event_sig == &alloy::primitives::keccak256("DataUsed(address,string,string,uint256)")
            {
                let scientist = Address::from_slice(&log.topics()[1][12..]);
                return Ok(ParsedEvent::DataUsed {
                    data_hash: format!("0x{}", hex::encode(&log.topics()[2])),
                    algorithm_id: format!("0x{}", hex::encode(&log.topics()[2])), // CID as algorithm_id
                    fee: U256::ZERO, // Fee not in this event
                });
            }
        }

        // AlgorithmReview events
        if log.address() == self.addresses.algorithm_review {
            // AlgorithmResolved(uint256 indexed executionId, string cid, bool approved)
            if event_sig == &alloy::primitives::keccak256("AlgorithmResolved(uint256,string,bool)")
            {
                // TODO: Properly decode the data field
                return Ok(ParsedEvent::AlgorithmResolved {
                    algorithm_id: "decoded_cid".to_string(), // Placeholder
                    approved: false,                         // Placeholder
                });
            }
            // CommitteeMemberUpdated(address indexed member, bool approved)
            if event_sig == &alloy::primitives::keccak256("CommitteeMemberUpdated(address,bool)") {
                let member = Address::from_slice(&log.topics()[1][12..]);
                // TODO: Decode bool from data
                return Ok(ParsedEvent::CommitteeMemberUpdated {
                    member,
                    is_member: true, // Placeholder
                });
            }
            // ExecutionSubmitted(uint256 indexed executionId, string cid, uint256 startTime, uint256 endTime)
            if event_sig
                == &alloy::primitives::keccak256(
                    "ExecutionSubmitted(uint256,string,uint256,uint256)",
                )
            {
                return Ok(ParsedEvent::ExecutionSubmitted {
                    algorithm_id: "decoded_cid".to_string(), // Placeholder
                    data_hash: "".to_string(),               // Not in this event
                    execution_result: "".to_string(),        // Not in this event
                });
            }
            // VoteCasted(address indexed member, string cid, bool approved, uint256 voteTime)
            if event_sig == &alloy::primitives::keccak256("VoteCasted(address,string,bool,uint256)")
            {
                let voter = Address::from_slice(&log.topics()[1][12..]);
                return Ok(ParsedEvent::VoteCasted {
                    algorithm_id: "decoded_cid".to_string(), // Placeholder
                    voter,
                    vote: true, // Placeholder
                });
            }
        }

        Err(ContractError::ParseError(format!(
            "Unknown event signature: {:?}",
            event_sig
        )))
    }
}

/// Parsed event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ParsedEvent {
    // DataContribution events
    DataRegistered {
        data_hash: String,
        provider: Address,
        price: U256,
    },
    DataUsed {
        data_hash: String,
        algorithm_id: String,
        fee: U256,
    },
    // AlgorithmReview events
    AlgorithmResolved {
        algorithm_id: String,
        approved: bool,
    },
    CommitteeMemberUpdated {
        member: Address,
        is_member: bool,
    },
    ExecutionSubmitted {
        algorithm_id: String,
        data_hash: String,
        execution_result: String,
    },
    VoteCasted {
        algorithm_id: String,
        voter: Address,
        vote: bool,
    },
}

impl From<ContractError> for crate::error::AppError {
    fn from(err: ContractError) -> Self {
        use crate::error::AppError;
        match err {
            ContractError::InvalidConfig(msg) => AppError::Config(msg),
            ContractError::NotFound(msg) => AppError::NotFound(msg),
            _ => AppError::Internal(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_contract_initialization() {
        // Test with a minimal config
        let config = ChainConfig {
            rpc_url: "http://localhost:8545".to_string(),
            ws_url: None,
            chain_id: 1337,
            contract_address: "0x0000000000000000000000000000000000000000".to_string(),
            data_contribution_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
            algorithm_review_address: "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512".to_string(),
            private_key: Some(
                "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
            ),
            confirmations: 1,
            gas_price_multiplier: 1.1,
            max_gas_limit: 10_000_000,
            funding_threshold_eth: 0.01,
            funding_amount_eth: 0.1,
            sync_interval: 10,
            sync_batch_size: 1000,
            block_batch_size: 100,
        };

        // Should create without errors (won't connect in test environment)
        let _caller = ContractCaller::new(config).await;
    }
}
