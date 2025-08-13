//! Smart contract interaction module
//!
//! This module provides functionality for interacting with blockchain smart contracts
//! using the alloy library.

use crate::config::ChainConfig;

use crate::infra::{db::Database, TeeEthereum, KEY_CTX_CONTRACT_OWNER};
use alloy::providers::Provider;
use alloy::sol_types::SolValue;
use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::ProviderBuilder,
    rpc::types::{BlockNumberOrTag, Filter, Log},
    signers::local::{LocalSignerError, PrivateKeySigner},
    sol,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

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
    SignerError(#[from] LocalSignerError),

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
    official_wallet: EthereumWallet, // Official wallet for funding operations
    tee_wallet: Option<EthereumWallet>, // TEE-derived wallet for contract operations
    tee_ethereum: Option<Arc<TeeEthereum>>, // TEE Ethereum manager
    addresses: ContractAddresses,
    db: Database, // Database connection for contract storage
}

impl ContractCaller {
    /// Create a new contract caller instance
    pub async fn new(config: ChainConfig, db: Database) -> Result<Self> {
        // Get official private key from config for funding operations
        let private_key = config.private_key.clone().ok_or_else(|| {
            ContractError::InvalidConfig("OFFICIAL_ACCOUNT_PRIVATE_KEY not set".to_string())
        })?;

        let signer = PrivateKeySigner::from_str(&private_key)?;
        let official_wallet = EthereumWallet::from(signer);

        // Load contract addresses from database
        let addresses = Self::load_contract_addresses(&db).await?;

        Ok(ContractCaller {
            config,
            official_wallet,
            tee_wallet: None,
            tee_ethereum: None,
            addresses,
            db,
        })
    }

    /// Load contract addresses from database
    async fn load_contract_addresses(db: &Database) -> Result<ContractAddresses> {
        let chain_id = std::env::var("CHAIN_ID").unwrap_or_else(|_| "31337".to_string());

        // Try to load addresses from database using runtime queries
        // This allows the code to compile even if the table doesn't exist yet
        let data_contribution: Option<String> = sqlx::query_scalar(
            "SELECT address FROM contract_meta WHERE name = $1 AND chain_id = $2",
        )
        .bind("DataContribution")
        .bind(&chain_id)
        .fetch_optional(db.pool())
        .await
        .unwrap_or(None); // If table doesn't exist, treat as None

        let algorithm_review: Option<String> = sqlx::query_scalar(
            "SELECT address FROM contract_meta WHERE name = $1 AND chain_id = $2",
        )
        .bind("AlgorithmReview")
        .bind(&chain_id)
        .fetch_optional(db.pool())
        .await
        .unwrap_or(None); // If table doesn't exist, treat as None

        // Use zero addresses if not found in database - will trigger deployment
        let data_contribution = data_contribution
            .and_then(|addr| Address::from_str(&addr).ok())
            .unwrap_or(Address::ZERO);

        let algorithm_review = algorithm_review
            .and_then(|addr| Address::from_str(&addr).ok())
            .unwrap_or(Address::ZERO);

        Ok(ContractAddresses {
            data_contribution,
            algorithm_review,
        })
    }

    /// Initialize TEE wallet for contract operations
    pub async fn with_tee(mut self, tee_ethereum: Arc<TeeEthereum>) -> Result<Self> {
        // Get TEE-derived contract owner account
        let tee_account = tee_ethereum.get_contract_owner().await.map_err(|e| {
            ContractError::InvalidConfig(format!("Failed to get TEE account: {}", e))
        })?;

        // Get the signer from TEE ethereum manager
        let tee_signer = tee_ethereum
            .get_signer(KEY_CTX_CONTRACT_OWNER)
            .await
            .map_err(|e| {
                ContractError::InvalidConfig(format!("Failed to get TEE signer: {}", e))
            })?;

        let tee_wallet = EthereumWallet::from(tee_signer);

        info!(
            "Initialized TEE wallet with address: {}",
            tee_account.address
        );

        self.tee_wallet = Some(tee_wallet);
        self.tee_ethereum = Some(tee_ethereum);

        // Ensure TEE wallet has sufficient balance for operations
        // Fund with a reasonable amount for testing/operations
        if let Err(e) = self.ensure_sufficient_balance(1.0).await {
            warn!("Failed to fund TEE wallet: {}. Continuing anyway.", e);
        } else {
            info!("TEE wallet funded successfully");
        }

        Ok(self)
    }

    /// Create a provider instance with the official wallet (for funding)
    async fn create_official_provider(&self) -> Result<impl Provider> {
        let provider_url = &self.config.rpc_url;

        let provider = ProviderBuilder::new()
            .wallet(self.official_wallet.clone())
            .connect(provider_url)
            .await
            .map_err(|e| ContractError::ProviderError(format!("Failed to connect: {}", e)))?;

        Ok(provider)
    }

    /// Create a provider instance with the TEE wallet (for contract operations)
    async fn create_tee_provider(&self) -> Result<impl Provider> {
        let provider_url = &self.config.rpc_url;

        let tee_wallet = self.tee_wallet.as_ref().ok_or_else(|| {
            ContractError::InvalidConfig("TEE wallet not initialized".to_string())
        })?;

        let provider = ProviderBuilder::new()
            .wallet(tee_wallet.clone())
            .connect(provider_url)
            .await
            .map_err(|e| ContractError::ProviderError(format!("Failed to connect: {}", e)))?;

        Ok(provider)
    }

    /// Ensure the TEE wallet has sufficient balance (funded by official wallet if needed)
    pub async fn ensure_sufficient_balance(&self, required_eth: f64) -> Result<()> {
        // First check if TEE wallet is initialized
        let tee_wallet = self.tee_wallet.as_ref().ok_or_else(|| {
            ContractError::InvalidConfig("TEE wallet not initialized".to_string())
        })?;

        let tee_address = tee_wallet.default_signer().address();

        // Check TEE wallet balance
        let provider = self.create_official_provider().await?;
        let balance = provider
            .get_balance(tee_address)
            .await
            .map_err(|e| ContractError::ProviderError(e.to_string()))?;
        let balance_eth = balance.to::<u128>() as f64 / 1e18;

        if balance_eth < required_eth {
            // Fund the TEE wallet from official wallet
            info!(
                "TEE wallet balance insufficient: {} ETH (required: {} ETH). Funding from official wallet...",
                balance_eth, required_eth
            );

            // Check official wallet balance first
            let official_address = self.official_wallet.default_signer().address();
            let official_balance = provider
                .get_balance(official_address)
                .await
                .map_err(|e| ContractError::ProviderError(e.to_string()))?;
            let official_balance_eth = official_balance.to::<u128>() as f64 / 1e18;

            let transfer_amount = required_eth - balance_eth + 0.01; // Add a small buffer

            if official_balance_eth < transfer_amount {
                return Err(ContractError::InsufficientBalance {
                    required: transfer_amount,
                    available: official_balance_eth,
                });
            }

            // Transfer ETH from official wallet to TEE wallet
            let transfer_amount_wei = U256::from((transfer_amount * 1e18) as u128);

            debug!(
                "Transferring {} ETH from official wallet {} to TEE wallet {}",
                transfer_amount, official_address, tee_address
            );

            // Build and send the transfer transaction
            let tx = alloy::rpc::types::TransactionRequest::default()
                .to(tee_address)
                .value(transfer_amount_wei)
                .from(official_address);

            let pending_tx = provider.send_transaction(tx).await.map_err(|e| {
                ContractError::TransactionFailed(format!(
                    "Failed to send funding transaction: {}",
                    e
                ))
            })?;

            // Wait for transaction confirmation
            let receipt = pending_tx
                .with_required_confirmations(self.config.confirmations)
                .get_receipt()
                .await
                .map_err(|e| {
                    ContractError::TransactionFailed(format!(
                        "Failed to confirm funding transaction: {}",
                        e
                    ))
                })?;

            info!(
                "Successfully funded TEE wallet with {} ETH. Tx hash: {:?}",
                transfer_amount, receipt.transaction_hash
            );
        } else {
            debug!(
                "TEE wallet balance: {} ETH (required: {} ETH)",
                balance_eth, required_eth
            );
        }

        Ok(())
    }

    /// Ensure contracts are deployed and accessible
    /// If contracts are not deployed, deploy them using the TEE wallet
    pub async fn ensure_contracts_deployed(&mut self) -> Result<()> {
        let provider = self.create_tee_provider().await?;

        // Check DataContribution contract
        let data_code = provider
            .get_code_at(self.addresses.data_contribution)
            .await?;

        let data_contribution_address = if data_code.is_empty() {
            info!("DataContribution contract not found, deploying...");

            // Deploy DataContribution contract
            let deploy_provider = self.create_tee_provider().await?;
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
            let deploy_provider = self.create_tee_provider().await?;
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

    /// Record data usage on the blockchain
    pub async fn record_data_usage(
        &self,
        scientist_wallet: Address,
        algo_cid: String,
        dataset_name: String,
    ) -> Result<String> {
        let provider = self.create_tee_provider().await?;
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

    /// Register a dataset on the blockchain
    pub async fn register_data(
        &self,
        author_wallet: Address,
        ipfs_cid: String,
        dataset_name: String,
    ) -> Result<String> {
        let provider = self.create_tee_provider().await?;
        let contract = DataContribution::new(self.addresses.data_contribution, provider);

        // Call the registerData function on the smart contract
        let call = contract.registerData(author_wallet, ipfs_cid.clone(), dataset_name.clone());

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!(
            "Dataset registered: name={}, ipfs_cid={}, tx={}",
            dataset_name, ipfs_cid, tx_hash
        );

        Ok(tx_hash)
    }

    /// Submit an algorithm for review
    pub async fn submit_algorithm(
        &self,
        scientist_wallet: Address,
        algorithm_cid: String,
        dataset_name: String,
    ) -> Result<String> {
        // Ensure TEE wallet has sufficient balance for the transaction
        self.ensure_sufficient_balance(0.1).await?;

        let provider = self.create_tee_provider().await?;
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
        // Ensure TEE wallet has sufficient balance for the transaction
        self.ensure_sufficient_balance(0.1).await?;

        let provider = self.create_tee_provider().await?;
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
        // Ensure TEE wallet has sufficient balance for the transaction
        self.ensure_sufficient_balance(0.1).await?;

        let provider = self.create_tee_provider().await?;
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
    pub async fn set_voting_duration(&self, duration: U256) -> Result<String> {
        // Ensure TEE wallet has sufficient balance for the transaction
        self.ensure_sufficient_balance(0.1).await?;

        let provider = self.create_tee_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let call = contract.setVotingDuration(duration);

        let pending_tx = call.send().await?;
        let receipt = pending_tx
            .with_required_confirmations(self.config.confirmations)
            .get_receipt()
            .await
            .map_err(|e| ContractError::TransactionFailed(e.to_string()))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        info!("Voting duration set: duration={}, tx={}", duration, tx_hash);

        Ok(tx_hash)
    }

    /// Vote on an algorithm
    pub async fn vote_on_algorithm(&self, algorithm_cid: String, vote: bool) -> Result<String> {
        let provider = self.create_tee_provider().await?;
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
        let provider = self.create_tee_provider().await?;
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
        let provider = self.create_tee_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let is_member = contract.isCommitteeMember(address).call().await?;
        Ok(is_member)
    }

    /// Get the contract owner
    pub async fn get_contract_owner(&self) -> Result<Address> {
        let provider = self.create_tee_provider().await?;
        let contract = AlgorithmReview::new(self.addresses.algorithm_review, provider);

        let owner = contract.owner().call().await?;
        Ok(owner)
    }

    /// Store contract address in database
    async fn store_contract_address(&self, name: &str, address: Address) -> Result<()> {
        info!("Storing contract {} at address {:?}", name, address);

        let chain_id = std::env::var("CHAIN_ID").unwrap_or_else(|_| "31337".to_string());
        let address_str = format!("{:#x}", address);

        // Use runtime query to avoid compile-time dependency on table existence
        sqlx::query(
            "INSERT INTO contract_meta (name, address, chain_id, deployed_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (name, chain_id) DO UPDATE
             SET address = $2, deployed_at = NOW()",
        )
        .bind(name)
        .bind(&address_str)
        .bind(&chain_id)
        .execute(self.db.pool())
        .await
        .map_err(|e| {
            // Log but don't fail if table doesn't exist yet
            info!(
                "Could not store contract address (table may not exist yet): {}",
                e
            );
            ContractError::ContractCallError(format!("Failed to store contract address: {}", e))
        })?;

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
        let provider = self.create_tee_provider().await?;
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

        let provider = self.create_tee_provider().await?;
        let logs = provider.get_logs(&filter).await?;
        Ok(logs)
    }

    /// Parse a log into a specific event type
    pub fn parse_event(&self, log: &Log) -> Result<ParsedEvent> {
        use alloy::sol_types::SolEvent;

        // Get the event signature (first topic)
        let event_sig = log
            .topic0()
            .ok_or_else(|| ContractError::ParseError("No event signature in log".to_string()))?;

        // DataContribution events
        if log.address() == self.addresses.data_contribution {
            // DataRegistered event
            if event_sig == &DataContribution::DataRegistered::SIGNATURE_HASH {
                // For indexed string (cid), we only have the hash in topics[2]
                let cid = if log.topics().len() > 2 {
                    format!("0x{}", hex::encode(&log.topics()[2]))
                } else {
                    return Err(ContractError::ParseError("Missing cid topic".to_string()));
                };

                // Decode the data field to get non-indexed parameters
                let dataset = String::abi_decode(&log.data().data).map_err(|e| {
                    ContractError::ParseError(format!("Failed to decode dataset: {}", e))
                })?;

                // Extract contributor address from topics[1]
                let contributor = Address::from_slice(&log.topics()[1][12..]);

                return Ok(ParsedEvent::DataRegistered {
                    contributor,
                    cid,
                    dataset,
                });
            }

            // DataUsed event
            if event_sig == &DataContribution::DataUsed::SIGNATURE_HASH {
                // For indexed string (cid), we only have the hash in topics[2]
                let cid = if log.topics().len() > 2 {
                    format!("0x{}", hex::encode(&log.topics()[2]))
                } else {
                    return Err(ContractError::ParseError("Missing cid topic".to_string()));
                };

                // Decode the data field to get non-indexed parameters (dataset, when)
                let (dataset, when): (String, U256) =
                    <(String, U256)>::abi_decode(&log.data().data).map_err(|e| {
                        ContractError::ParseError(format!("Failed to decode data: {}", e))
                    })?;

                let scientist = Address::from_slice(&log.topics()[1][12..]);

                return Ok(ParsedEvent::DataUsed {
                    scientist,
                    cid,
                    dataset,
                    when,
                });
            }
        }

        // AlgorithmReview events
        if log.address() == self.addresses.algorithm_review {
            // AlgorithmResolved event
            if event_sig == &AlgorithmReview::AlgorithmResolved::SIGNATURE_HASH {
                // Decode the data field (cid, approved)
                let (cid, approved): (String, bool) =
                    <(String, bool)>::abi_decode(&log.data().data).map_err(|e| {
                        ContractError::ParseError(format!("Failed to decode data: {}", e))
                    })?;

                // Extract execution_id from topics[1]
                let execution_id = U256::from_be_bytes(log.topics()[1].into());

                return Ok(ParsedEvent::AlgorithmResolved {
                    execution_id,
                    cid,
                    approved,
                });
            }

            // CommitteeMemberUpdated event
            if event_sig == &AlgorithmReview::CommitteeMemberUpdated::SIGNATURE_HASH {
                // Extract member address from topics[1]
                let member = Address::from_slice(&log.topics()[1][12..]);

                // Decode the approved flag from data
                let approved = bool::abi_decode(&log.data().data).map_err(|e| {
                    ContractError::ParseError(format!("Failed to decode approved: {}", e))
                })?;

                return Ok(ParsedEvent::CommitteeMemberUpdated { member, approved });
            }

            // ExecutionSubmitted event
            if event_sig == &AlgorithmReview::ExecutionSubmitted::SIGNATURE_HASH {
                // Decode the data field (cid, startTime, endTime)
                let (cid, start_time, end_time): (String, U256, U256) =
                    <(String, U256, U256)>::abi_decode(&log.data().data).map_err(|e| {
                        ContractError::ParseError(format!("Failed to decode data: {}", e))
                    })?;

                // Extract execution_id from topics[1]
                let execution_id = U256::from_be_bytes(log.topics()[1].into());

                return Ok(ParsedEvent::ExecutionSubmitted {
                    execution_id,
                    cid,
                    start_time,
                    end_time,
                });
            }

            // VoteCasted event
            if event_sig == &AlgorithmReview::VoteCasted::SIGNATURE_HASH {
                // Extract member address from topics[1]
                let member = Address::from_slice(&log.topics()[1][12..]);

                // Decode the data field (cid, approved, voteTime)
                let (cid, approved, vote_time): (String, bool, U256) =
                    <(String, bool, U256)>::abi_decode(&log.data().data).map_err(|e| {
                        ContractError::ParseError(format!("Failed to decode data: {}", e))
                    })?;

                return Ok(ParsedEvent::VoteCasted {
                    member,
                    cid,
                    approved,
                    vote_time,
                });
            }
        }

        Err(ContractError::ParseError(format!(
            "Unknown event signature: {:?} at address {:?}",
            event_sig,
            log.address()
        )))
    }
}

/// Parsed event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ParsedEvent {
    // DataContribution events
    DataRegistered {
        contributor: Address,
        cid: String, // Note: for indexed string, this will be the hash
        dataset: String,
    },
    DataUsed {
        scientist: Address,
        cid: String, // Note: for indexed string, this will be the hash
        dataset: String,
        when: U256,
    },
    // AlgorithmReview events
    AlgorithmResolved {
        execution_id: U256,
        cid: String,
        approved: bool,
    },
    CommitteeMemberUpdated {
        member: Address,
        approved: bool,
    },
    ExecutionSubmitted {
        execution_id: U256,
        cid: String,
        start_time: U256,
        end_time: U256,
    },
    VoteCasted {
        member: Address,
        cid: String,
        approved: bool,
        vote_time: U256,
    },
}

impl From<ContractError> for crate::AppError {
    fn from(err: ContractError) -> Self {
        match err {
            ContractError::InvalidConfig(msg) => crate::AppError::Validation(msg),
            ContractError::NotFound(msg) => crate::AppError::NotFound(msg),
            _ => crate::AppError::Internal(err.to_string()),
        }
    }
}
