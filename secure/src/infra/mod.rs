//! Infrastructure modules for the secure service
//!
//! This module contains foundational components that provide core functionality
//! for cryptography, database access, blockchain interaction, TEE management,
//! and WebSocket communication.

pub mod contracts;
pub mod crypto;
pub mod db;
pub mod notifier;
pub mod sample_generator;
pub mod tee;
pub mod tee_crypto;

// Re-export commonly used items for convenience
pub use contracts::{ContractAddresses, ContractCaller, ContractError};
pub use crypto::{decrypt, decrypt_hex_key, encrypt, encrypt_hex_key, CryptoError};
pub use db::{Database, DatabaseError};
pub use notifier::Notifier;
pub use sample_generator::{SampleGenerator, DEFAULT_SAMPLE_SIZE};
pub use tee_crypto::{TeeCryptoService, PURPOSE_ENC_STATIC_DATASET};

// Re-export TEE components with cleaner names
pub use tee::{
    // Adapter functions
    tee_key_to_ethereum_account,
    // Core client
    Client as TeeClient,
    ClientBuilder as TeeClientBuilder,
    ClientConfig as TeeClientConfig,

    // Error handling
    Error as TeeError,
    // Ethereum management
    Ethereum as TeeEthereum,
    EthereumAccount as TeeEthereumAccount,
    EthereumBuilder as TeeEthereumBuilder,

    KeyDerivationParams,

    // Key management
    KeyManager as TeeKeyManager,
    KeyManagerBuilder as TeeKeyManagerBuilder,
    Result as TeeResult,

    SymmetricKey as TeeSymmetricKey,
    KEY_CTX_ALGORITHM_OWNER_PREFIX,
    // Key context constants
    KEY_CTX_CONTRACT_OWNER,
    KEY_CTX_DATASET_OWNER_PREFIX,
    KEY_CTX_DATA_ENCRYPT_PREFIX,

    KEY_CTX_TASK_ENCRYPT_PREFIX,
    KEY_CTX_UPLOAD_REPORT_ENCRYPT,
};
