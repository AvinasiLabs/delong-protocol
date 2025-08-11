//! Infrastructure modules for the secure service
//!
//! This module contains foundational components that provide core functionality
//! for cryptography, database access, blockchain interaction, TEE management,
//! and WebSocket communication.

pub mod contracts;
pub mod crypto;
pub mod db;
pub mod dstack_adapter;
pub mod notifier;

pub mod tee;
pub mod tee_error;
pub mod tee_ethereum;
pub mod tee_key_management;

// Re-export commonly used items for convenience
pub use contracts::{ContractAddresses, ContractCaller, ContractError};
pub use crypto::{decrypt, decrypt_hex_key, encrypt, encrypt_hex_key, CryptoError};
pub use db::{Database, DatabaseError};
pub use notifier::Notifier;

pub use tee::{TeeConfig, TeeKey, TeeService, TeeServiceBuilder};
pub use tee_error::{TeeError, TeeResult};
pub use tee_ethereum::{
    AttestatedEthereumAccount, TeeEthereumService, TeeEthereumServiceBuilder,
    KEY_CTX_ALGORITHM_OWNER_PREFIX, KEY_CTX_DATASET_OWNER_PREFIX, KEY_CTX_TEE_CONTRACT_OWNER,
};
pub use tee_key_management::{
    AttestatedSymmetricKey, KeyDerivationParams, TeeKeyManagementService,
    TeeKeyManagementServiceBuilder, KEY_CTX_DATA_ENCRYPT_PREFIX, KEY_CTX_TASK_ENCRYPT_PREFIX,
    KEY_CTX_UPLOAD_REPORT_ENCRYPT,
};
