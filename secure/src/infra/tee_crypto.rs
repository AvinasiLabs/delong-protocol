//! TEE cryptographic service for secure data encryption
//!
//! This module provides TEE-based encryption services for datasets,
//! ensuring all sensitive data is properly encrypted before storage.

use crate::infra::{
    crypto::{decrypt, encrypt, CryptoError},
    tee::{Client as TeeClient, KEY_CTX_DATA_ENCRYPT_PREFIX},
};
use avinapi::prelude::AppError;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Purpose identifier for static dataset encryption
pub const PURPOSE_ENC_STATIC_DATASET: &str = "enc_static_dataset";

/// TEE cryptographic service for secure operations
#[derive(Clone)]
pub struct TeeCryptoService {
    /// TEE client for key derivation
    tee_client: Arc<TeeClient>,
}

impl TeeCryptoService {
    /// Create a new TEE crypto service
    pub fn new(tee_client: Arc<TeeClient>) -> Self {
        Self { tee_client }
    }

    /// Derive a symmetric key for dataset encryption
    ///
    /// # Arguments
    /// * `author` - The dataset author identifier
    /// * `purpose` - The purpose of the key (defaults to PURPOSE_ENC_STATIC_DATASET)
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The derived symmetric key
    /// * `Err(AppError)` - If key derivation fails
    pub async fn derive_dataset_key(
        &self,
        author: &str,
        purpose: Option<&str>,
    ) -> Result<Vec<u8>, AppError> {
        let purpose = purpose.unwrap_or(PURPOSE_ENC_STATIC_DATASET);

        // Construct the key derivation path
        let key_path = format!("{}/{}", KEY_CTX_DATA_ENCRYPT_PREFIX, author);

        debug!(
            "Deriving dataset encryption key for author: {}, path: {}, purpose: {}",
            author, key_path, purpose
        );

        // Derive the key using TEE
        let derived_key = self
            .tee_client
            .derive_key(&key_path, Some(purpose))
            .await
            .map_err(|e| {
                error!("Failed to derive key for author {}: {}", author, e);
                AppError::Internal(format!("Key derivation failed: {}", e))
            })?;

        // Convert hex key to bytes
        let key_bytes = hex::decode(&derived_key.key).map_err(|e| {
            error!("Failed to decode derived key: {}", e);
            AppError::Internal(format!("Invalid key format: {}", e))
        })?;

        // Ensure key has correct length for AES (16, 24, or 32 bytes)
        if key_bytes.len() != 16 && key_bytes.len() != 24 && key_bytes.len() != 32 {
            error!("Invalid key length: {} bytes", key_bytes.len());
            return Err(AppError::Internal(format!(
                "Invalid key length: expected 16, 24, or 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        info!(
            "Successfully derived {} byte key for author: {}",
            key_bytes.len(),
            author
        );

        Ok(key_bytes)
    }

    /// Encrypt data using a TEE-derived key
    ///
    /// # Arguments
    /// * `data` - The data to encrypt
    /// * `author` - The dataset author identifier
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The encrypted data
    /// * `Err(AppError)` - If encryption fails
    pub async fn encrypt_dataset(&self, data: &[u8], author: &str) -> Result<Vec<u8>, AppError> {
        // Derive the encryption key
        let key = self.derive_dataset_key(author, None).await?;

        // Encrypt the data
        let encrypted = encrypt(data, &key).map_err(|e| {
            error!("Failed to encrypt data for author {}: {}", author, e);
            match e {
                CryptoError::InvalidKeyLength(len) => {
                    AppError::Internal(format!("Invalid key length: {}", len))
                }
                CryptoError::EncryptionFailed(msg) => {
                    AppError::Internal(format!("Encryption failed: {}", msg))
                }
                _ => AppError::Internal("Encryption failed".to_string()),
            }
        })?;

        debug!(
            "Successfully encrypted {} bytes for author: {}",
            data.len(),
            author
        );

        Ok(encrypted)
    }

    /// Decrypt data using a TEE-derived key
    ///
    /// # Arguments
    /// * `encrypted_data` - The encrypted data
    /// * `author` - The dataset author identifier
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The decrypted data
    /// * `Err(AppError)` - If decryption fails
    pub async fn decrypt_dataset(
        &self,
        encrypted_data: &[u8],
        author: &str,
    ) -> Result<Vec<u8>, AppError> {
        // Derive the decryption key
        let key = self.derive_dataset_key(author, None).await?;

        // Decrypt the data
        let decrypted = decrypt(encrypted_data, &key).map_err(|e| {
            error!("Failed to decrypt data for author {}: {}", author, e);
            match e {
                CryptoError::DecryptionFailed(msg) => {
                    AppError::Internal(format!("Decryption failed: {}", msg))
                }
                _ => AppError::Internal("Decryption failed".to_string()),
            }
        })?;

        debug!(
            "Successfully decrypted {} bytes for author: {}",
            encrypted_data.len(),
            author
        );

        Ok(decrypted)
    }

    /// Encrypt data with a provided key (for advanced use cases)
    ///
    /// # Arguments
    /// * `data` - The data to encrypt
    /// * `key` - The encryption key
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The encrypted data
    /// * `Err(AppError)` - If encryption fails
    pub fn encrypt_with_key(data: &[u8], key: &[u8]) -> Result<Vec<u8>, AppError> {
        encrypt(data, key).map_err(|e| {
            error!("Failed to encrypt data with provided key: {}", e);
            AppError::Internal(format!("Encryption failed: {}", e))
        })
    }

    /// Decrypt data with a provided key (for advanced use cases)
    ///
    /// # Arguments
    /// * `encrypted_data` - The encrypted data
    /// * `key` - The decryption key
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The decrypted data
    /// * `Err(AppError)` - If decryption fails
    pub fn decrypt_with_key(encrypted_data: &[u8], key: &[u8]) -> Result<Vec<u8>, AppError> {
        decrypt(encrypted_data, key).map_err(|e| {
            error!("Failed to decrypt data with provided key: {}", e);
            AppError::Internal(format!("Decryption failed: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_encrypt_decrypt_roundtrip() {
        // This test would require a mock TEE client
        // For now, we'll test the static encrypt/decrypt functions

        let data = b"Test dataset content";
        let key = vec![0u8; 32]; // 32-byte key for AES-256

        let encrypted =
            TeeCryptoService::encrypt_with_key(data, &key).expect("Encryption should succeed");

        assert_ne!(&encrypted[..], data);
        assert!(encrypted.len() > data.len()); // Should include nonce

        let decrypted = TeeCryptoService::decrypt_with_key(&encrypted, &key)
            .expect("Decryption should succeed");

        assert_eq!(&decrypted[..], data);
    }

    #[test]
    fn test_invalid_key_length() {
        let data = b"Test data";
        let invalid_key = vec![0u8; 15]; // Invalid key length

        let result = TeeCryptoService::encrypt_with_key(data, &invalid_key);
        assert!(result.is_err());
    }
}
