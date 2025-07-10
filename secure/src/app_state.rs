use crate::{
    auth::AuthService,
    config::SecureConfig,
    services::{
        ctr_caller::ContractCallerService, ipfs::IpfsService, key_vault::KeyVaultService,
    },
};
use sqlx::PgPool;
use std::sync::Arc;

/// The application state, holding shared resources for the secure service.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: SecureConfig,
    pub ipfs_service: IpfsService,
    pub ctr_caller_service: Arc<ContractCallerService>,
    pub auth_service: AuthService,
    pub key_vault_service: Arc<KeyVaultService>,
}

impl AppState {
    /// Create a new `AppState`.
    pub async fn new(config: SecureConfig, pool: PgPool) -> anyhow::Result<Self> {
        let key_vault_service =
            Arc::new(KeyVaultService::new(&config.tee.key_vault_master_key_path)?);

        let ipfs_service = IpfsService::new(config.ipfs.api_url.clone());

        let ctr_caller_service = Arc::new(
            ContractCallerService::new(
                config.blockchain.http_url.clone(),
                config.blockchain.chain_id,
                config.blockchain.private_key.clone(),
                config.blockchain.data_contribution_address.clone(),
                config.blockchain.algorithm_review_address.clone(),
                key_vault_service.clone(),
            )
            .await?,
        );

        let auth_service = AuthService::new();

        Ok(Self {
            pool,
            config,
            ipfs_service,
            ctr_caller_service,
            auth_service,
            key_vault_service,
        })
    }
} 