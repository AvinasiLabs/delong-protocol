//! TEE-specific error types
//!
//! This module defines error types for TEE operations using thiserror
//! for better ergonomics and automatic trait implementations.

use thiserror::Error;

/// Errors that can occur during TEE operations
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to initialize TEE client
    #[error("Failed to initialize TEE client: {0}")]
    ClientInitialization(String),

    /// Failed to derive key from TEE
    #[error("Failed to derive key for context '{0}'")]
    KeyDerivation(String),

    /// Failed to derive key from TEE with additional context
    #[error("Failed to derive key for context '{context}': {message}")]
    KeyDerivationWithContext { context: String, message: String },

    /// Failed to convert TEE key to Ethereum account
    #[error("Failed to convert TEE key to Ethereum account: {0}")]
    KeyConversion(String),

    /// Failed to sign a message
    #[error("Failed to sign {operation}: {reason}")]
    SigningFailed {
        operation: SigningOperation,
        reason: String,
    },

    /// Account not found in cache
    #[error("Account not found in cache for context: {0}")]
    AccountNotFound(String),

    /// Failed to serialize/deserialize data
    #[error("Serialization error during {operation}: {details}")]
    Serialization { operation: String, details: String },

    /// HKDF key derivation failed
    #[error("HKDF key derivation failed: {0}")]
    HkdfFailed(String),

    /// Failed to emit event through TEE
    #[error("Failed to emit event '{event_name}': {reason}")]
    EventEmission { event_name: String, reason: String },

    /// TEE service is not available
    #[error("TEE service is not available")]
    ServiceUnavailable,

    /// Invalid configuration
    #[error("Invalid TEE configuration: {0}")]
    InvalidConfig(String),

    /// Hex decoding error
    #[error("Hex decoding error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Other TEE-related errors
    #[error("TEE error: {0}")]
    Other(String),
}

/// Types of signing operations
#[derive(Debug)]
pub enum SigningOperation {
    Message,
    Hash,
    TypedData,
}

impl std::fmt::Display for SigningOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SigningOperation::Message => write!(f, "message"),
            SigningOperation::Hash => write!(f, "hash"),
            SigningOperation::TypedData => write!(f, "typed data"),
        }
    }
}

/// Result type for TEE operations
pub type Result<T> = std::result::Result<T, Error>;

// Implement conversion to AppError for seamless error propagation
impl From<Error> for crate::AppError {
    fn from(err: Error) -> Self {
        match &err {
            Error::ClientInitialization(_)
            | Error::ServiceUnavailable
            | Error::InvalidConfig(_)
            | Error::KeyDerivation(_)
            | Error::KeyDerivationWithContext { .. } => crate::AppError::Internal(err.to_string()),
            Error::AccountNotFound(_) => crate::AppError::NotFound(err.to_string()),
            Error::Serialization { .. } | Error::HexDecode(_) | Error::Json(_) => {
                crate::AppError::Validation(err.to_string())
            }
            _ => crate::AppError::Internal(err.to_string()),
        }
    }
}

// Helper functions for creating errors with context
impl Error {
    /// Create a KeyDerivation error with optional source context
    pub fn key_derivation(context: &str, source: Option<String>) -> Self {
        match source {
            Some(msg) => Error::KeyDerivationWithContext {
                context: context.to_string(),
                message: msg,
            },
            None => Error::KeyDerivation(context.to_string()),
        }
    }
}
