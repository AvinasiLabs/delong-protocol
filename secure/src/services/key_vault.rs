//! Service for managing and deriving cryptographic keys within the TEE.
use crate::services::key_ctx::KeyContext;
use anyhow::{Context, Result};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::{Signer, Wallet};
use hkdf::Hkdf;
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

pub type EthereumWallet = Wallet<SigningKey>;

/// A secure vault for deriving and caching cryptographic keys.
/// This is a simplified, file-backed simulation of a hardware-backed KeyVault.
#[derive(Debug)]
pub struct KeyVaultService {
    master_key: Vec<u8>,
    // In a real multi-threaded scenario, we'd use a concurrent hash map.
    // A Mutex is sufficient for this simulation.
    eth_cache: Mutex<HashMap<String, EthereumWallet>>,
    symm_key_cache: Mutex<HashMap<String, Vec<u8>>>,
}

impl KeyVaultService {
    /// Creates a new `KeyVaultService`.
    ///
    /// It loads the master key from the specified path. If the file does not exist,
    /// a new random master key is generated and saved to that path.
    pub fn new(master_key_path: &str) -> Result<Self> {
        let path = Path::new(master_key_path);
        let master_key = if path.exists() {
            info!("Loading master key from {}", master_key_path);
            fs::read(path).context("Failed to read master key from file")?
        } else {
            info!("Master key not found, generating a new one at {}", master_key_path);
            let new_key = rand::random::<[u8; 32]>().to_vec();
            fs::create_dir_all(path.parent().unwrap())
                .context("Failed to create directory for master key")?;
            fs::write(path, &new_key).context("Failed to write new master key to file")?;
            new_key
        };

        Ok(Self {
            master_key,
            eth_cache: Mutex::new(HashMap::new()),
            symm_key_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Derives a new Ethereum account (wallet) using HKDF based on the master key and a context.
    ///
    /// The derivation is deterministic. The same context will always produce the same account.
    /// Results are cached in memory to avoid re-derivation.
    pub fn derive_ethereum_account(&self, kc: &KeyContext) -> Result<EthereumWallet> {
        let cache_key = kc.cache_key();
        if let Some(wallet) = self.eth_cache.lock().unwrap().get(&cache_key) {
            return Ok(wallet.clone());
        }

        // The HKDF-SHA256 process for deriving the private key.
        let hkdf = Hkdf::<Sha256>::new(Some(&kc.salt()), &self.master_key);
        let mut okm = [0u8; 32]; // Output Key Material
        hkdf.expand(&kc.info(), &mut okm)
            .context("HKDF expansion failed")?;

        let signing_key = SigningKey::from_bytes(&okm)?;
        let wallet = Wallet::from(signing_key);
        
        info!("Derived new Ethereum account for purpose '{}' with address: {}", kc.purpose(), wallet.address());

        self.eth_cache
            .lock()
            .unwrap()
            .insert(cache_key, wallet.clone());

        Ok(wallet)
    }

    /// Derives a new 32-byte symmetric key using HKDF.
    pub fn derive_symmetric_key(&self, kc: &KeyContext) -> Result<Vec<u8>> {
        let cache_key = kc.cache_key();
        if let Some(key) = self.symm_key_cache.lock().unwrap().get(&cache_key) {
            return Ok(key.clone());
        }
        
        let hkdf = Hkdf::<Sha256>::new(Some(&kc.salt()), &self.master_key);
        let mut okm = vec![0u8; 32];
        hkdf.expand(&kc.info(), &mut okm)
            .context("HKDF expansion failed")?;

        info!("Derived new symmetric key for purpose '{}'", kc.purpose());
        
        self.symm_key_cache
            .lock()
            .unwrap()
            .insert(cache_key, okm.clone());
            
        Ok(okm)
    }
} 