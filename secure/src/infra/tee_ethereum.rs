use super::dstack_adapter::tee_key_to_ethereum_account;
use super::tee_error::{SigningOperation, TeeError, TeeResult};
use alloy::primitives::{Address, Bytes, B256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::tee::{TeeKey, TeeService};

/// Key context for TEE contract owner
pub const KEY_CTX_TEE_CONTRACT_OWNER: &str = "tee-eth-account/contract-owner";

/// Key context for algorithm owner accounts
pub const KEY_CTX_ALGORITHM_OWNER_PREFIX: &str = "tee-eth-account/algorithm-owner";

/// Key context for dataset owner accounts
pub const KEY_CTX_DATASET_OWNER_PREFIX: &str = "tee-eth-account/dataset-owner";

/// Ethereum account with attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestatedEthereumAccount {
    /// Ethereum address
    pub address: Address,
    /// Associated TEE key with attestation
    pub tee_key: TeeKey,
    /// Account purpose/context
    pub context: String,
}

/// TEE-backed Ethereum service
pub struct TeeEthereumService {
    tee_service: Arc<TeeService>,
    account_cache: Arc<RwLock<std::collections::HashMap<String, TeeKey>>>,
}

impl TeeEthereumService {
    /// Create a new TEE Ethereum service
    pub fn new(tee_service: Arc<TeeService>) -> Self {
        Self {
            tee_service,
            account_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get or derive an Ethereum account for the given context
    pub async fn get_account(&self, context: &str) -> TeeResult<AttestatedEthereumAccount> {
        // Check cache first
        {
            let cache = self.account_cache.read().await;
            if let Some(tee_key) = cache.get(context) {
                debug!("Using cached account for context: {}", context);
                // Recreate signer to get address
                let key_response = dstack_sdk::dstack_client::GetKeyResponse {
                    key: tee_key.key.clone(),
                    signature_chain: tee_key.signature_chain.clone(),
                };
                let signer = tee_key_to_ethereum_account(&key_response)?;
                return Ok(AttestatedEthereumAccount {
                    address: signer.address(),
                    tee_key: tee_key.clone(),
                    context: context.to_string(),
                });
            }
        }

        // Derive new account
        info!("Deriving new Ethereum account for context: {}", context);
        let tee_key = self
            .tee_service
            .derive_key(context, Some("ethereum"))
            .await
            .map_err(|e| {
                TeeError::key_derivation(
                    context,
                    Some(format!("Failed to derive key from TEE: {}", e)),
                )
            })?;

        // Convert to Ethereum account using dstack SDK
        let key_response = dstack_sdk::dstack_client::GetKeyResponse {
            key: tee_key.key.clone(),
            signature_chain: tee_key.signature_chain.clone(),
        };

        let signer = tee_key_to_ethereum_account(&key_response)?;
        let address = signer.address();

        // Cache the account
        {
            let mut cache = self.account_cache.write().await;
            cache.insert(context.to_string(), tee_key.clone());
        }

        info!(
            "Derived Ethereum account: {} for context: {}",
            address, context
        );

        Ok(AttestatedEthereumAccount {
            address,
            tee_key,
            context: context.to_string(),
        })
    }

    /// Get the contract owner account
    pub async fn get_contract_owner(&self) -> TeeResult<AttestatedEthereumAccount> {
        self.get_account(KEY_CTX_TEE_CONTRACT_OWNER).await
    }

    /// Get an algorithm owner account
    pub async fn get_algorithm_owner(&self, algo_id: &str) -> TeeResult<AttestatedEthereumAccount> {
        let context = format!("{}/{}", KEY_CTX_ALGORITHM_OWNER_PREFIX, algo_id);
        self.get_account(&context).await
    }

    /// Get a dataset owner account
    pub async fn get_dataset_owner(
        &self,
        dataset_id: &str,
    ) -> TeeResult<AttestatedEthereumAccount> {
        let context = format!("{}/{}", KEY_CTX_DATASET_OWNER_PREFIX, dataset_id);
        self.get_account(&context).await
    }

    /// Sign a message with the specified account
    pub async fn sign_message(&self, context: &str, message: &[u8]) -> TeeResult<Bytes> {
        let cache = self.account_cache.read().await;
        let tee_key = cache
            .get(context)
            .ok_or_else(|| TeeError::AccountNotFound(context.to_string()))?;

        // Recreate signer from TeeKey
        let key_response = dstack_sdk::dstack_client::GetKeyResponse {
            key: tee_key.key.clone(),
            signature_chain: tee_key.signature_chain.clone(),
        };
        let signer = tee_key_to_ethereum_account(&key_response)?;

        // Sign the message
        let signature =
            signer
                .sign_message(message)
                .await
                .map_err(|e| TeeError::SigningFailed {
                    operation: SigningOperation::Message,
                    reason: format!("{:?}", e),
                })?;

        Ok(signature.as_bytes().into())
    }

    /// Sign a transaction hash with the specified account
    pub async fn sign_hash(&self, context: &str, hash: &B256) -> TeeResult<Bytes> {
        let cache = self.account_cache.read().await;
        let tee_key = cache
            .get(context)
            .ok_or_else(|| TeeError::AccountNotFound(context.to_string()))?;

        // Recreate signer from TeeKey
        let key_response = dstack_sdk::dstack_client::GetKeyResponse {
            key: tee_key.key.clone(),
            signature_chain: tee_key.signature_chain.clone(),
        };
        let signer = tee_key_to_ethereum_account(&key_response)?;

        // Sign the hash
        let signature = signer
            .sign_hash(hash)
            .await
            .map_err(|e| TeeError::SigningFailed {
                operation: SigningOperation::Hash,
                reason: format!("{:?}", e),
            })?;

        Ok(signature.as_bytes().into())
    }

    /// Sign typed data (EIP-712) with the specified account
    pub async fn sign_typed_data<T: alloy::sol_types::SolStruct + Serialize>(
        &self,
        context: &str,
        payload: &T,
    ) -> TeeResult<Bytes> {
        let cache = self.account_cache.read().await;
        let tee_key = cache
            .get(context)
            .ok_or_else(|| TeeError::AccountNotFound(context.to_string()))?;

        // Recreate signer from TeeKey
        let key_response = dstack_sdk::dstack_client::GetKeyResponse {
            key: tee_key.key.clone(),
            signature_chain: tee_key.signature_chain.clone(),
        };
        let signer = tee_key_to_ethereum_account(&key_response)?;

        // Convert the payload to bytes and sign it as a message
        let payload_bytes = serde_json::to_vec(&payload).map_err(|e| TeeError::Serialization {
            operation: "typed data payload".to_string(),
            details: e.to_string(),
        })?;
        let signature =
            signer
                .sign_message(&payload_bytes)
                .await
                .map_err(|e| TeeError::SigningFailed {
                    operation: SigningOperation::TypedData,
                    reason: format!("{:?}", e),
                })?;

        Ok(signature.as_bytes().into())
    }

    /// Get the private key signer for advanced operations
    /// Note: Use with caution, prefer the sign_* methods when possible
    pub async fn get_signer(&self, context: &str) -> TeeResult<PrivateKeySigner> {
        // Ensure account is loaded
        let _ = self.get_account(context).await?;

        let cache = self.account_cache.read().await;
        let tee_key = cache
            .get(context)
            .ok_or_else(|| TeeError::AccountNotFound(context.to_string()))?;

        // Recreate signer from TeeKey
        let key_response = dstack_sdk::dstack_client::GetKeyResponse {
            key: tee_key.key.clone(),
            signature_chain: tee_key.signature_chain.clone(),
        };
        let signer = tee_key_to_ethereum_account(&key_response)?;

        Ok(signer)
    }

    /// Clear the account cache
    pub async fn clear_cache(&self) {
        let mut cache = self.account_cache.write().await;
        cache.clear();
        info!("Cleared Ethereum account cache");
    }

    /// Get all cached account contexts
    pub async fn list_cached_accounts(&self) -> Vec<String> {
        let cache = self.account_cache.read().await;
        cache.keys().cloned().collect()
    }
}

/// Builder for creating a TEE Ethereum service
pub struct TeeEthereumServiceBuilder {
    tee_service: Option<Arc<TeeService>>,
}

impl TeeEthereumServiceBuilder {
    pub fn new() -> Self {
        Self { tee_service: None }
    }

    pub fn tee_service(mut self, service: Arc<TeeService>) -> Self {
        self.tee_service = Some(service);
        self
    }

    pub fn build(self) -> TeeResult<TeeEthereumService> {
        let tee_service = self.tee_service.ok_or(TeeError::ServiceUnavailable)?;

        Ok(TeeEthereumService::new(tee_service))
    }
}

impl Default for TeeEthereumServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::tee::test_helpers;

    #[tokio::test]
    async fn test_account_derivation() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let eth_service = TeeEthereumService::new(tee_service);

        // Test deriving contract owner account
        let owner = eth_service.get_contract_owner().await;
        if let Err(ref e) = owner {
            eprintln!("Failed to get contract owner: {:?}", e);
        }
        assert!(owner.is_ok());

        if let Ok(account) = owner {
            assert_eq!(account.context, KEY_CTX_TEE_CONTRACT_OWNER);
            // Address should be valid (non-zero)
            assert_ne!(account.address, Address::ZERO);
        }
    }

    #[tokio::test]
    async fn test_account_caching() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let eth_service = TeeEthereumService::new(tee_service);

        let context = "test-context";

        // First call should derive new account
        let account1 = eth_service.get_account(context).await;
        assert!(account1.is_ok());

        // Second call should use cached account
        let account2 = eth_service.get_account(context).await;
        assert!(account2.is_ok());

        if let (Ok(acc1), Ok(acc2)) = (account1, account2) {
            // Should return the same address
            assert_eq!(acc1.address, acc2.address);
        }
    }

    #[tokio::test]
    async fn test_algorithm_owner_account() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let eth_service = TeeEthereumService::new(tee_service);

        let algo_id = "algo-123";
        let account = eth_service.get_algorithm_owner(algo_id).await;

        assert!(account.is_ok());
        if let Ok(acc) = account {
            assert!(acc.context.contains(KEY_CTX_ALGORITHM_OWNER_PREFIX));
            assert!(acc.context.contains(algo_id));
        }
    }

    #[tokio::test]
    async fn test_dataset_owner_account() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let eth_service = TeeEthereumService::new(tee_service);

        let dataset_id = "dataset-456";
        let account = eth_service.get_dataset_owner(dataset_id).await;

        assert!(account.is_ok());
        if let Ok(acc) = account {
            assert!(acc.context.contains(KEY_CTX_DATASET_OWNER_PREFIX));
            assert!(acc.context.contains(dataset_id));
        }
    }
}
