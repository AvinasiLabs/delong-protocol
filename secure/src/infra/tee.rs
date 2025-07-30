//! TEE (Trusted Execution Environment) key management and derivation
//!
//! This module provides secure key derivation and management functionality
//! specifically designed for TEE environments. It supports Ethereum key
//! generation and symmetric key derivation using HKDF.

use async_trait::async_trait;
use ethers::{core::k256::ecdsa::SigningKey, prelude::*};
use hkdf::Hkdf;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Errors that can occur during TEE operations
#[derive(Error, Debug)]
pub enum TeeError {
    #[error("TEE adapter error: {0}")]
    AdapterError(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Invalid key type: {0}")]
    InvalidKeyType(String),

    #[error("Ethereum key generation failed: {0}")]
    EthereumKeyError(String),

    #[error("Cache error: {0}")]
    CacheError(String),
}

/// Result type for TEE operations
pub type Result<T> = std::result::Result<T, TeeError>;

/// TEE client types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Dstack,
    Tappd,
}

/// Ethereum account representation
#[derive(Debug, Clone)]
pub struct EthereumAccount {
    pub private_key: SigningKey,
    pub address: Address,
}

impl EthereumAccount {
    /// Create a new Ethereum account from a private key
    pub fn from_private_key(private_key: SigningKey) -> Self {
        let wallet = LocalWallet::from(private_key.clone());
        Self {
            private_key,
            address: wallet.address(),
        }
    }

    /// Get the address as a hex string
    pub fn address_hex(&self) -> String {
        format!("{:?}", self.address)
    }
}

/// TEE adapter trait for different TEE implementations
#[async_trait]
pub trait TeeAdapter: Send + Sync {
    /// Derive a key from the TEE's root key
    async fn derive_key(&self, context: &str, key_type: &str) -> Result<Vec<u8>>;

    /// Get the TEE's attestation report
    async fn get_attestation(&self) -> Result<Vec<u8>>;

    /// Get the TEE type
    fn client_kind(&self) -> ClientKind;
}

/// Dstack TEE adapter implementation
pub struct DstackAdapter {
    // Implementation details would go here
}

impl DstackAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TeeAdapter for DstackAdapter {
    async fn derive_key(&self, context: &str, key_type: &str) -> Result<Vec<u8>> {
        // TODO: Implement actual Dstack key derivation
        // For now, return a dummy implementation
        let mut key = vec![0u8; 32];
        let input = format!("{}-{}", context, key_type);
        let hk = Hkdf::<Sha256>::new(None, input.as_bytes());
        hk.expand(b"dstack-key", &mut key)
            .map_err(|e| TeeError::KeyDerivationFailed(e.to_string()))?;
        Ok(key)
    }

    async fn get_attestation(&self) -> Result<Vec<u8>> {
        // TODO: Implement actual attestation
        Ok(vec![])
    }

    fn client_kind(&self) -> ClientKind {
        ClientKind::Dstack
    }
}

/// Tappd TEE adapter implementation
pub struct TappdAdapter {
    // Implementation details would go here
}

impl TappdAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TeeAdapter for TappdAdapter {
    async fn derive_key(&self, context: &str, key_type: &str) -> Result<Vec<u8>> {
        // TODO: Implement actual Tappd key derivation
        let mut key = vec![0u8; 32];
        let input = format!("{}-{}", context, key_type);
        let hk = Hkdf::<Sha256>::new(None, input.as_bytes());
        hk.expand(b"tappd-key", &mut key)
            .map_err(|e| TeeError::KeyDerivationFailed(e.to_string()))?;
        Ok(key)
    }

    async fn get_attestation(&self) -> Result<Vec<u8>> {
        // TODO: Implement actual attestation
        Ok(vec![])
    }

    fn client_kind(&self) -> ClientKind {
        ClientKind::Tappd
    }
}

/// Key vault for managing TEE-derived keys
pub struct KeyVault {
    adapter: Box<dyn TeeAdapter>,
    eth_cache: Arc<Mutex<HashMap<String, EthereumAccount>>>,
    symm_key_cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl KeyVault {
    /// Create a new key vault with the specified adapter
    pub fn new(adapter: Box<dyn TeeAdapter>) -> Self {
        Self {
            adapter,
            eth_cache: Arc::new(Mutex::new(HashMap::new())),
            symm_key_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a key vault from a client type
    pub fn from_config(client_type: ClientKind) -> Self {
        let adapter: Box<dyn TeeAdapter> = match client_type {
            ClientKind::Dstack => Box::new(DstackAdapter::new()),
            ClientKind::Tappd => Box::new(TappdAdapter::new()),
        };
        Self::new(adapter)
    }

    /// Get or derive an Ethereum account for the given context
    pub async fn get_ethereum_account(&self, context: &str) -> Result<EthereumAccount> {
        // Check cache first
        {
            let cache = self
                .eth_cache
                .lock()
                .map_err(|e| TeeError::CacheError(e.to_string()))?;
            if let Some(account) = cache.get(context) {
                return Ok(account.clone());
            }
        }

        // Derive new key
        let key_bytes = self.adapter.derive_key(context, "ethereum").await?;

        // Create signing key from bytes
        let signing_key = SigningKey::from_slice(&key_bytes)
            .map_err(|e| TeeError::EthereumKeyError(e.to_string()))?;

        let account = EthereumAccount::from_private_key(signing_key);

        // Cache the account
        {
            let mut cache = self
                .eth_cache
                .lock()
                .map_err(|e| TeeError::CacheError(e.to_string()))?;
            cache.insert(context.to_string(), account.clone());
        }

        Ok(account)
    }

    /// Get or derive a symmetric key for the given context
    pub async fn get_symmetric_key(&self, context: &str) -> Result<Vec<u8>> {
        // Check cache first
        {
            let cache = self
                .symm_key_cache
                .lock()
                .map_err(|e| TeeError::CacheError(e.to_string()))?;
            if let Some(key) = cache.get(context) {
                return Ok(key.clone());
            }
        }

        // Derive new key
        let key = self.adapter.derive_key(context, "symmetric").await?;

        // Cache the key
        {
            let mut cache = self
                .symm_key_cache
                .lock()
                .map_err(|e| TeeError::CacheError(e.to_string()))?;
            cache.insert(context.to_string(), key.clone());
        }

        Ok(key)
    }

    /// Derive a key using HKDF with custom salt and info
    pub fn derive_key_hkdf(
        &self,
        ikm: &[u8],
        salt: Option<&[u8]>,
        info: &[u8],
        length: usize,
    ) -> Result<Vec<u8>> {
        let mut okm = vec![0u8; length];
        let hk = Hkdf::<Sha256>::new(salt, ikm);
        hk.expand(info, &mut okm)
            .map_err(|e| TeeError::KeyDerivationFailed(e.to_string()))?;
        Ok(okm)
    }

    /// Get attestation report from TEE
    pub async fn get_attestation(&self) -> Result<Vec<u8>> {
        self.adapter.get_attestation().await
    }

    /// Clear all cached keys
    pub fn clear_cache(&self) -> Result<()> {
        {
            let mut cache = self
                .eth_cache
                .lock()
                .map_err(|e| TeeError::CacheError(e.to_string()))?;
            cache.clear();
        }
        {
            let mut cache = self
                .symm_key_cache
                .lock()
                .map_err(|e| TeeError::CacheError(e.to_string()))?;
            cache.clear();
        }
        Ok(())
    }
}

/// Key context builder for generating deterministic context strings
pub struct KeyContext {
    parts: Vec<String>,
}

impl KeyContext {
    /// Create a new key context builder
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    /// Add a component to the context
    pub fn with(mut self, component: &str) -> Self {
        self.parts.push(component.to_string());
        self
    }

    /// Add a typed component to the context
    pub fn with_type(self, typ: &str, value: &str) -> Self {
        self.with(&format!("{}:{}", typ, value))
    }

    /// Build the final context string
    pub fn build(self) -> String {
        self.parts.join("/")
    }
}

impl Default for KeyContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_vault_ethereum_account() {
        let vault = KeyVault::from_config(ClientKind::Dstack);

        let account1 = vault.get_ethereum_account("test-context").await.unwrap();
        let account2 = vault.get_ethereum_account("test-context").await.unwrap();

        // Should return the same account from cache
        assert_eq!(account1.address, account2.address);
    }

    #[tokio::test]
    async fn test_key_vault_symmetric_key() {
        let vault = KeyVault::from_config(ClientKind::Tappd);

        let key1 = vault.get_symmetric_key("test-key").await.unwrap();
        let key2 = vault.get_symmetric_key("test-key").await.unwrap();

        // Should return the same key from cache
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_key_context_builder() {
        let context = KeyContext::new()
            .with("delong")
            .with_type("algo", "123")
            .with_type("dataset", "456")
            .build();

        assert_eq!(context, "delong/algo:123/dataset:456");
    }

    #[test]
    fn test_hkdf_key_derivation() {
        let vault = KeyVault::from_config(ClientKind::Dstack);

        let ikm = b"initial key material";
        let salt = b"salt";
        let info = b"info";

        let key = vault.derive_key_hkdf(ikm, Some(salt), info, 32).unwrap();
        assert_eq!(key.len(), 32);
    }
}
