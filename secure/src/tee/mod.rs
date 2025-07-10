use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use async_trait::async_trait;
use sha2::{Sha256, Digest};
use hkdf::Hkdf;
use secp256k1::{SecretKey, PublicKey, Secp256k1};
use serde::{Serialize, Deserialize};
use tracing::{info, warn};
use common::ApiResult;

pub mod encryption;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Phala,
    Mock,
}

impl ClientKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientKind::Phala => "phala",
            ClientKind::Mock => "mock",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyContext {
    pub dataset_hash: String,
    pub author: String,
    pub purpose: String,
    pub salt: Vec<u8>,
}

impl KeyContext {
    pub fn new(dataset_hash: String, author: String, purpose: String) -> Self {
        let salt = format!("delong-{}-{}", author, purpose);
        Self {
            dataset_hash,
            author,
            purpose,
            salt: salt.into_bytes(),
        }
    }

    pub fn cache_key(&self) -> String {
        format!("{}:{}:{}", self.dataset_hash, self.author, self.purpose)
    }

    pub fn info(&self) -> &[u8] {
        b"delong-v1"
    }

    pub fn salt(&self) -> &[u8] {
        &self.salt
    }
}

#[derive(Debug, Clone)]
pub struct EthereumAccount {
    pub private_key: SecretKey,
    pub address: String,
}

impl EthereumAccount {
    pub fn from_secret_key(private_key: SecretKey) -> Self {
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &private_key);
        
        // Convert public key to Ethereum address
        let public_key_bytes = public_key.serialize_uncompressed();
        let hash = Sha256::digest(&public_key_bytes[1..]);
        let address = format!("0x{}", hex::encode(&hash[12..]));
        
        Self {
            private_key,
            address,
        }
    }
}

#[async_trait]
pub trait TeeClient: Send + Sync {
    async fn derive_key(&self, context: &KeyContext) -> ApiResult<Vec<u8>>;
    async fn verify_attestation(&self) -> ApiResult<bool>;
    fn client_kind(&self) -> ClientKind;
}

pub struct PhalaClient {
    attestation_verified: RwLock<bool>,
}

impl PhalaClient {
    pub fn new() -> Self {
        Self {
            attestation_verified: RwLock::new(false),
        }
    }
}

#[async_trait]
impl TeeClient for PhalaClient {
    async fn derive_key(&self, context: &KeyContext) -> ApiResult<Vec<u8>> {
        let verified = *self.attestation_verified.read().unwrap();
        if !verified {
            return Err(common::ApiError::Forbidden);
        }

        // In a real Phala implementation, this would call the Phala runtime
        // For now, use a deterministic key derivation
        let mut hasher = Sha256::new();
        hasher.update(context.dataset_hash.as_bytes());
        hasher.update(&context.salt);
        hasher.update(context.info());
        let result = hasher.finalize();
        
        info!(
            dataset_hash = %context.dataset_hash,
            author = %context.author,
            purpose = %context.purpose,
            "Derived key in TEE environment"
        );
        
        Ok(result.to_vec())
    }

    async fn verify_attestation(&self) -> ApiResult<bool> {
        // In a real implementation, this would verify hardware attestation
        info!("Verifying TEE attestation");
        *self.attestation_verified.write().unwrap() = true;
        Ok(true)
    }

    fn client_kind(&self) -> ClientKind {
        ClientKind::Phala
    }
}

pub struct MockClient;

impl MockClient {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TeeClient for MockClient {
    async fn derive_key(&self, context: &KeyContext) -> ApiResult<Vec<u8>> {
        // Mock implementation always succeeds
        let mut hasher = Sha256::new();
        hasher.update(b"mock_master_key");
        hasher.update(context.dataset_hash.as_bytes());
        hasher.update(&context.salt);
        hasher.update(context.info());
        let result = hasher.finalize();
        
        info!(
            dataset_hash = %context.dataset_hash,
            author = %context.author,
            purpose = %context.purpose,
            "Derived mock key"
        );
        
        Ok(result.to_vec())
    }

    async fn verify_attestation(&self) -> ApiResult<bool> {
        warn!("Using mock TEE client - attestation always succeeds");
        Ok(true)
    }

    fn client_kind(&self) -> ClientKind {
        ClientKind::Mock
    }
}

pub struct KeyVault {
    client: Arc<dyn TeeClient>,
    symmetric_key_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    ethereum_cache: Arc<RwLock<HashMap<String, EthereumAccount>>>,
    attestation_verified: Arc<RwLock<bool>>,
}

impl Clone for KeyVault {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            symmetric_key_cache: Arc::clone(&self.symmetric_key_cache),
            ethereum_cache: Arc::clone(&self.ethereum_cache),
            attestation_verified: Arc::clone(&self.attestation_verified),
        }
    }
}

impl KeyVault {
    pub fn new(client: Arc<dyn TeeClient>) -> Self {
        Self {
            client,
            symmetric_key_cache: Arc::new(RwLock::new(HashMap::new())),
            ethereum_cache: Arc::new(RwLock::new(HashMap::new())),
            attestation_verified: Arc::new(RwLock::new(false)),
        }
    }

    pub fn new_with_client_kind(client_kind: ClientKind) -> Self {
        let client: Arc<dyn TeeClient> = match client_kind {
            ClientKind::Phala => Arc::new(PhalaClient::new()),
            ClientKind::Mock => Arc::new(MockClient::new()),
        };
        Self::new(client)
    }

    pub async fn verify_attestation(&self) -> ApiResult<bool> {
        let verified = self.client.verify_attestation().await?;
        *self.attestation_verified.write().unwrap() = verified;
        Ok(verified)
    }

    pub fn is_attestation_verified(&self) -> bool {
        *self.attestation_verified.read().unwrap()
    }

    pub async fn derive_symmetric_key(&self, context: &KeyContext) -> ApiResult<Vec<u8>> {
        let cache_key = context.cache_key();
        
        // Check cache first
        {
            let cache = self.symmetric_key_cache.read().unwrap();
            if let Some(cached_key) = cache.get(&cache_key) {
                return Ok(cached_key.clone());
            }
        }

        // Derive new key
        let raw_key = self.client.derive_key(context).await?;
        
        // Use HKDF to derive final key
        let hkdf = Hkdf::<Sha256>::new(Some(context.salt()), &raw_key);
        let mut key = vec![0u8; 32];
        hkdf.expand(context.info(), &mut key)
            .map_err(|_e| common::ApiError::InternalError)?;
        
        // Cache the result
        {
            let mut cache = self.symmetric_key_cache.write().unwrap();
            cache.insert(cache_key, key.clone());
        }
        
        info!(
            dataset_hash = %context.dataset_hash,
            "Derived and cached symmetric key"
        );
        
        Ok(key)
    }

    pub async fn derive_ethereum_account(&self, context: &KeyContext) -> ApiResult<EthereumAccount> {
        let cache_key = context.cache_key();
        
        // Check cache first
        {
            let cache = self.ethereum_cache.read().unwrap();
            if let Some(cached_account) = cache.get(&cache_key) {
                return Ok(cached_account.clone());
            }
        }

        // Derive new key for Ethereum account
        let raw_key = self.client.derive_key(context).await?;

        let secret_key = SecretKey::from_slice(&raw_key)
            .map_err(|e| common::ApiError::InternalError)?;
        
        let account = EthereumAccount::from_secret_key(secret_key);

        // Cache the result
        {
            let mut cache = self.ethereum_cache.write().unwrap();
            cache.insert(cache_key, account.clone());
        }
        
        info!(
            dataset_hash = %context.dataset_hash,
            address = %account.address,
            "Derived and cached Ethereum account"
        );
        
        Ok(account)
    }

    pub fn clear_cache(&self) {
        let mut sym_cache = self.symmetric_key_cache.write().unwrap();
        let mut eth_cache = self.ethereum_cache.write().unwrap();
        sym_cache.clear();
        eth_cache.clear();
        info!("Cleared key caches");
    }

    /// Get the number of cached symmetric keys
    pub fn cached_symmetric_keys_count(&self) -> usize {
        self.symmetric_key_cache.read().unwrap().len()
    }

    /// Get the number of cached Ethereum accounts
    pub fn cached_ethereum_accounts_count(&self) -> usize {
        self.ethereum_cache.read().unwrap().len()
    }

    /// Encrypt data using TEE-derived keys
    pub async fn encrypt_data(&self, data: &[u8], dataset_id: &str) -> ApiResult<EncryptedData> {
        let context = KeyContext::new(
            dataset_id.to_string(),
            "system".to_string(), // Default author for system operations
            "encryption".to_string(),
        );

        let key = self.derive_symmetric_key(&context).await?;
        
        let (ciphertext, nonce) = crate::tee::encryption::MockEncryption::encrypt(data, &key)?;
        
        Ok(EncryptedData {
            encrypted_data: ciphertext,
            nonce,
            dataset_id: dataset_id.to_string(),
        })
    }

    /// Decrypt data using TEE-derived keys
    pub async fn decrypt_data(&self, encrypted: &EncryptedData) -> ApiResult<Vec<u8>> {
        let context = KeyContext::new(
            encrypted.dataset_id.clone(),
            "system".to_string(),
            "encryption".to_string(),
        );

        let key = self.derive_symmetric_key(&context).await?;
        
        let plaintext = crate::tee::encryption::MockEncryption::decrypt(
            &encrypted.encrypted_data,
            &encrypted.nonce,
            &key
        )?;
        
        Ok(plaintext)
    }
}

/// Encrypted data structure for KeyVault operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub encrypted_data: Vec<u8>,
    pub nonce: Vec<u8>,
    pub dataset_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_context() {
        let context = KeyContext::new(
            "test_hash".to_string(),
            "author123".to_string(),
            "testing".to_string(),
        );
        
        assert_eq!(context.dataset_hash, "test_hash");
        assert_eq!(context.author, "author123");
        assert_eq!(context.purpose, "testing");
        assert_eq!(context.cache_key(), "test_hash:author123:testing");
    }

    #[tokio::test]
    async fn test_mock_key_vault() {
        let vault = KeyVault::new_with_client_kind(ClientKind::Mock);
        
        let context = KeyContext::new(
            "test_dataset".to_string(),
            "test_author".to_string(),
            "encryption".to_string(),
        );
        
        // Test attestation
        let verified = vault.verify_attestation().await.unwrap();
        assert!(verified);
        
        // Test symmetric key derivation
        let key1 = vault.derive_symmetric_key(&context).await.unwrap();
        let key2 = vault.derive_symmetric_key(&context).await.unwrap();
        assert_eq!(key1, key2); // Should be cached
        assert_eq!(key1.len(), 32);
        
        // Test Ethereum account derivation
        let account1 = vault.derive_ethereum_account(&context).await.unwrap();
        let account2 = vault.derive_ethereum_account(&context).await.unwrap();
        assert_eq!(account1.address, account2.address); // Should be cached
        assert!(account1.address.starts_with("0x"));
    }

    #[tokio::test]
    async fn test_phala_key_vault_requires_attestation() {
        let vault = KeyVault::new_with_client_kind(ClientKind::Phala);
        
        let context = KeyContext::new(
            "test_dataset".to_string(),
            "test_author".to_string(),
            "encryption".to_string(),
        );
        
        // Should fail without attestation
        let result = vault.derive_symmetric_key(&context).await;
        assert!(result.is_err());
        
        // Should succeed after attestation
        vault.verify_attestation().await.unwrap();
        let key = vault.derive_symmetric_key(&context).await.unwrap();
        assert_eq!(key.len(), 32);
    }

    #[tokio::test]
    async fn test_different_contexts_different_keys() {
        let vault = KeyVault::new_with_client_kind(ClientKind::Mock);
        vault.verify_attestation().await.unwrap();
        
        let context1 = KeyContext::new(
            "dataset1".to_string(),
            "author1".to_string(),
            "encryption".to_string(),
        );
        
        let context2 = KeyContext::new(
            "dataset2".to_string(),
            "author1".to_string(),
            "encryption".to_string(),
        );
        
        let key1 = vault.derive_symmetric_key(&context1).await.unwrap();
        let key2 = vault.derive_symmetric_key(&context2).await.unwrap();
        
        // Different contexts should produce different keys
        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_cache_management() {
        let vault = KeyVault::new_with_client_kind(ClientKind::Mock);
        vault.verify_attestation().await.unwrap();
        
        let context = KeyContext::new(
            "test_dataset".to_string(),
            "test_author".to_string(),
            "encryption".to_string(),
        );
        
        // Initially no cached keys
        assert_eq!(vault.cached_symmetric_keys_count(), 0);
        assert_eq!(vault.cached_ethereum_accounts_count(), 0);
        
        // Derive keys
        vault.derive_symmetric_key(&context).await.unwrap();
        vault.derive_ethereum_account(&context).await.unwrap();
        
        // Should be cached
        assert_eq!(vault.cached_symmetric_keys_count(), 1);
        assert_eq!(vault.cached_ethereum_accounts_count(), 1);
        
        // Clear cache
        vault.clear_cache();
        assert_eq!(vault.cached_symmetric_keys_count(), 0);
        assert_eq!(vault.cached_ethereum_accounts_count(), 0);
    }
} 