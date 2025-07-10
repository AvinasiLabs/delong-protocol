//! Service for interacting with smart contracts on the blockchain.
use crate::{
    contracts::{AlgorithmReview, DataContribution},
    models::Contract,
    services::{
        key_ctx::{KeyContext, KeyKind, KEY_CTX_TEE_CONTRACT_OWNER},
        key_vault::{EthereumWallet, KeyVaultService},
    },
};
use anyhow::Result;
use common::ApiResult;
use ethers::{
    prelude::{Address, LocalWallet, Middleware, Provider, Signer, SignerMiddleware, TxnBuilder},
    types::{H160, U256},
    utils::parse_ether,
};
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};
use tracing::{info, instrument};

type EthProvider = Provider<ethers::providers::Http>;
type EthSigner = SignerMiddleware<EthProvider, EthereumWallet>;

#[derive(Clone)]
pub struct ContractCallerService {
    provider: Arc<EthProvider>,
    funding_wallet: LocalWallet, // For topping up other wallets
    key_vault: Arc<KeyVaultService>,
    data_contribution_address: Address,
    algorithm_review_address: Address,
}

impl ContractCallerService {
    pub async fn new(
        rpc_url: String,
        chain_id: u64,
        funding_private_key: String, // Plaintext key for the account that funds others
        data_contribution_address: String,
        algorithm_review_address: String,
        key_vault: Arc<KeyVaultService>,
    ) -> Result<Self> {
        let provider = Arc::new(Provider::try_from(rpc_url)?);
        let funding_wallet = funding_private_key.parse::<LocalWallet>()?.with_chain_id(chain_id);

        let dc_address = Address::from_str(&data_contribution_address)?;
        let ar_address = Address::from_str(&algorithm_review_address)?;

        info!("ContractCallerService initialized successfully");
        Ok(Self {
            provider,
            funding_wallet,
            key_vault,
            data_contribution_address: dc_address,
            algorithm_review_address: ar_address,
        })
    }

    /// Ensures the TEE contract owner wallet is funded. If not, it tops it up.
    pub async fn ensure_wallet_funded(
        &self,
        wallet_to_fund: &EthereumWallet,
        threshold_eth: f64,
        top_up_eth: f64,
    ) -> Result<()> {
        let to_addr = wallet_to_fund.address();
        let balance = self.provider.get_balance(to_addr, None).await?;
        let threshold = parse_ether(threshold_eth)?;

        info!(
            "Checking balance for {:#?}. Current: {}, Threshold: {}",
            to_addr, balance, threshold
        );

        if balance.cmp(&threshold) >= std::cmp::Ordering::Greater {
            info!("Balance is sufficient.");
            return Ok(());
        }

        info!("Balance below threshold. Funding wallet...");
        let top_up_value = parse_ether(top_up_eth)?;
        let tx = (&self.funding_wallet)
            .send_transaction(
                ethers::types::TransactionRequest::new().to(to_addr).value(top_up_value),
                None,
            )
            .await?
            .await?;

        info!("Successfully funded wallet. Tx: {:?}", tx);
        Ok(())
    }

    /// Ensures contracts are deployed, deploying them if necessary.
    pub async fn ensure_contracts_deployed(&self, db_pool: &PgPool) -> Result<()> {
        info!("Ensuring all smart contracts are deployed...");
        let key_context = KeyContext::new(
            KeyKind::EthAccount,
            KEY_CTX_TEE_CONTRACT_OWNER,
            "Contract deployment",
        );
        let owner_wallet = self.key_vault.derive_ethereum_account(&key_context)?;

        // Ensure the owner wallet has enough funds to deploy contracts.
        // Using hardcoded values from Go reference for now.
        self.ensure_wallet_funded(&owner_wallet, 0.1, 0.5).await?;

        let owner_signer = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            owner_wallet.with_chain_id(self.funding_wallet.chain_id()),
        ));

        // Deploy DataContribution if needed
        if Contract::get_by_key(db_pool, "data-contribution")
            .await?
            .is_none()
        {
            info!("DataContribution contract not found in DB, deploying...");
            let contract = DataContribution::deploy(owner_signer.clone(), ())?
                .send()
                .await?;
            let addr = contract.address();
            info!("Deployed DataContribution at address: {:?}", addr);
            Contract::create(
                db_pool,
                "data-contribution",
                &format!("{:?}", addr),
                &format!("{:?}", owner_signer.address()),
            )
            .await?;
        }

        // Deploy AlgorithmReview if needed
        if Contract::get_by_key(db_pool, "algorithm-review")
            .await?
            .is_none()
        {
            info!("AlgorithmReview contract not found in DB, deploying...");
            let contract = AlgorithmReview::deploy(owner_signer.clone(), ())?.send().await?;
            let addr = contract.address();
            info!("Deployed AlgorithmReview at address: {:?}", addr);
            Contract::create(
                db_pool,
                "algorithm-review",
                &format!("{:?}", addr),
                &format!("{:?}", owner_signer.address()),
            )
            .await?;
        }

        info!("Contract deployment check complete.");
        Ok(())
    }

    /// Creates a SignerMiddleware instance with a derived wallet for a specific context.
    fn get_signer_for_ctx(&self, kc: &crate::services::key_ctx::KeyContext) -> Result<Arc<EthSigner>> {
        let derived_wallet = self.key_vault.derive_ethereum_account(kc)?;
        let signer = SignerMiddleware::new(
            self.provider.clone(),
            derived_wallet.with_chain_id(self.funding_wallet.chain_id()),
        );
        Ok(Arc::new(signer))
    }

    #[instrument(skip(self), fields(execution_id, scientist_wallet, algo_cid))]
    pub async fn submit_algorithm_execution(
        &self,
        execution_id: i64,
        scientist_wallet: &str,
        algo_cid: &str,
        dataset: &str,
        kc: &crate::services::key_ctx::KeyContext, // The context for the signing wallet
    ) -> ApiResult<String> {
        let signer = self.get_signer_for_ctx(kc)?;
        let contract = AlgorithmReview::new(self.algorithm_review_address, signer);

        let scientist_addr = H160::from_str(scientist_wallet)?;
        let call = contract.submit_algorithm(
            U256::from(execution_id),
            scientist_addr,
            algo_cid.to_string(),
            dataset.to_string(),
        );
        let pending_tx = call.send().await?;
        let tx_hash = *pending_tx;
        info!("Submitted algorithm execution, tx_hash: {:?}", tx_hash);
        Ok(format!("{:?}", tx_hash))
    }

    #[instrument(skip(self), fields(member_wallet, is_approved))]
    pub async fn upsert_committee_member(
        &self,
        member_wallet: &str,
        is_approved: bool,
        kc: &crate::services::key_ctx::KeyContext, // The context for the signing wallet
    ) -> ApiResult<String> {
        let signer = self.get_signer_for_ctx(kc)?;
        let contract = AlgorithmReview::new(self.algorithm_review_address, signer);

        let member_addr = H160::from_str(member_wallet)?;
        let call = contract.set_committee_member(member_addr, is_approved);
        let pending_tx = call.send().await?;
        let tx_hash = *pending_tx;
        info!("Upserted committee member, tx_hash: {:?}", tx_hash);
        Ok(format!("{:?}", tx_hash))
    }

    #[instrument(skip(self), fields(execution_id, voter_wallet, decision))]
    pub async fn cast_vote(
        &self,
        execution_id: i64,
        voter_wallet: &str,
        decision: &str,
        kc: &crate::services::key_ctx::KeyContext, // The context for the signing wallet
    ) -> ApiResult<String> {
        let signer = self.get_signer_for_ctx(kc)?;
        let contract = AlgorithmReview::new(self.algorithm_review_address, signer.clone());
        
        // The contract's vote function uses the algorithm's CID, not the voter's wallet directly
        let execution = contract.executions(U256::from(execution_id)).call().await?;
        let algo_cid = execution.2; 
        
        let approved = decision.to_uppercase() == "APPROVE";
        let call = contract.vote(algo_cid, approved);
        let pending_tx = call.send().await?;
        let tx_hash = *pending_tx;
        info!("Cast vote, tx_hash: {:?}", tx_hash);
        Ok(format!("{:?}", tx_hash))
    }

    #[instrument(skip(self), fields(file_hash, ipfs_cid))]
    pub async fn register_static_dataset(
        &self,
        contributor_wallet: &str,
        ipfs_cid: &str,
        dataset_name: &str,
        kc: &crate::services::key_ctx::KeyContext, // The context for the signing wallet
    ) -> ApiResult<String> {
        let signer = self.get_signer_for_ctx(kc)?;
        let contract = DataContribution::new(self.data_contribution_address, signer);

        let contributor_addr = H160::from_str(contributor_wallet)?;
        let call = contract.register_data(
            contributor_addr,
            ipfs_cid.to_string(),
            dataset_name.to_string(),
        );
        let pending_tx = call.send().await?;
        let tx_hash = *pending_tx;
        info!("Registered static dataset, tx_hash: {:?}", tx_hash);
        Ok(format!("{:?}", tx_hash))
    }

    #[instrument(skip(self), fields(raw_report_cid))]
    pub async fn register_test_report(
        &self,
        contributor_wallet: &str,
        file_hash_as_dataset_name: &str,
        raw_report_cid: &str,
        kc: &crate::services::key_ctx::KeyContext, // The context for the signing wallet
    ) -> ApiResult<String> {
        let signer = self.get_signer_for_ctx(kc)?;
        let contract = DataContribution::new(self.data_contribution_address, signer);
        
        let contributor_addr = H160::from_str(contributor_wallet)?;
        let call = contract.register_data(
            contributor_addr,
            raw_report_cid.to_string(),
            file_hash_as_dataset_name.to_string(),
        );
        let pending_tx = call.send().await?;
        let tx_hash = *pending_tx;
        info!("Registered test report, tx_hash: {:?}", tx_hash);
        Ok(format!("{:?}", tx_hash))
    }
} 