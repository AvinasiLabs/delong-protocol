//! Infrastructure modules for the secure service
//!
//! This module contains foundational components that provide core functionality
//! for cryptography, database access, blockchain interaction, TEE management,
//! and WebSocket communication.

pub mod contracts;
pub mod crypto;
pub mod db;
pub mod tee;
pub mod ws;

// Re-export commonly used items for convenience
pub use contracts::{ContractAddresses, ContractCaller, ContractConfig, ContractEvent};
pub use crypto::{CryptoError, decrypt, decrypt_hex_key, encrypt, encrypt_hex_key};
pub use db::{Database, DatabaseError};
pub use tee::{ClientKind, EthereumAccount, KeyContext, KeyVault, TeeError};
pub use ws::{TaskNotifier, WsHub, WsMessage};
