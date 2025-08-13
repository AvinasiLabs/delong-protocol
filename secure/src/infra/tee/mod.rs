//! TEE (Trusted Execution Environment) module
//!
//! This module provides secure operations through TEE/dstack integration,
//! including key derivation, Ethereum account management, and cryptographic operations.

mod adapter;
mod client;
mod error;
mod ethereum;
mod keys;

// Re-export main types with cleaner names
pub use client::{Client, ClientBuilder, ClientConfig};
pub use error::{Error, Result};
pub use ethereum::{
    Ethereum,
    EthereumAccount,
    EthereumBuilder,
    KEY_CTX_ALGORITHM_OWNER_PREFIX,
    // Key contexts for different account types
    KEY_CTX_CONTRACT_OWNER,
    KEY_CTX_DATASET_OWNER_PREFIX,
};
pub use keys::{
    KeyDerivationParams,
    KeyManager,
    KeyManagerBuilder,
    SymmetricKey,
    KEY_CTX_DATA_ENCRYPT_PREFIX,
    KEY_CTX_TASK_ENCRYPT_PREFIX,
    // Key contexts for encryption
    KEY_CTX_UPLOAD_REPORT_ENCRYPT,
};

// Re-export the adapter for advanced use cases
pub use adapter::tee_key_to_ethereum_account;

// Test helpers for development
#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    /// Create a TEE client configured for testing
    pub fn create_test_client() -> Client {
        let endpoint = std::env::var("DSTACK_SIMULATOR_ENDPOINT").unwrap_or_else(|_| {
            // Use HTTP proxy to connect to dstack-simulator
            "http://localhost:11010".to_string()
        });

        ClientBuilder::new().endpoint(endpoint).build()
    }
}

/// Prelude module for common imports
pub mod prelude {
    pub use super::{Client, ClientBuilder, ClientConfig};
    pub use super::{Error, Result};
    pub use super::{Ethereum, EthereumAccount};
    pub use super::{KeyManager, SymmetricKey};
}
