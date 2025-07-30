//! Cryptographic utilities for secure data encryption and decryption
//!
//! This module provides AES-GCM encryption/decryption functionality
//! compatible with the TEE security requirements.

use aes_gcm::{
    Aes128Gcm, Aes256Gcm,
    aead::{Aead, AeadCore, KeyInit, OsRng, generic_array::GenericArray},
};
use hex;
use thiserror::Error;

/// Errors that can occur during cryptographic operations
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid key length: expected 16, 24, or 32 bytes, got {0}")]
    InvalidKeyLength(usize),

    #[error("Invalid hex key: {0}")]
    InvalidHexKey(#[from] hex::FromHexError),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Combined data too short: expected at least {expected} bytes, got {actual}")]
    DataTooShort { expected: usize, actual: usize },
}

/// Result type for cryptographic operations
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Encrypts plaintext using AES-GCM with a random nonce
///
/// The output format is [nonce | ciphertext] where the nonce is prepended
/// to the ciphertext. This matches the format used by the Go implementation.
///
/// # Arguments
/// * `plaintext` - The data to encrypt
/// * `key` - The encryption key (must be 16, 24, or 32 bytes)
///
/// # Returns
/// * `Ok(Vec<u8>)` - The encrypted data with prepended nonce
/// * `Err(CryptoError)` - If encryption fails
pub fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    match key.len() {
        16 => encrypt_with_cipher::<Aes128Gcm>(plaintext, key),
        32 => encrypt_with_cipher::<Aes256Gcm>(plaintext, key),
        len => Err(CryptoError::InvalidKeyLength(len)),
    }
}

/// Decrypts ciphertext that was encrypted with `encrypt`
///
/// Expects the input format to be [nonce | ciphertext] where the nonce
/// is prepended to the ciphertext.
///
/// # Arguments
/// * `combined` - The encrypted data with prepended nonce
/// * `key` - The decryption key (must be 16, 24, or 32 bytes)
///
/// # Returns
/// * `Ok(Vec<u8>)` - The decrypted plaintext
/// * `Err(CryptoError)` - If decryption fails
pub fn decrypt(combined: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    match key.len() {
        16 => decrypt_with_cipher::<Aes128Gcm>(combined, key),
        32 => decrypt_with_cipher::<Aes256Gcm>(combined, key),
        len => Err(CryptoError::InvalidKeyLength(len)),
    }
}

/// Encrypts plaintext using a hex-encoded key
///
/// # Arguments
/// * `plaintext` - The data to encrypt
/// * `hex_key` - The hex-encoded encryption key
///
/// # Returns
/// * `Ok(Vec<u8>)` - The encrypted data with prepended nonce
/// * `Err(CryptoError)` - If encryption fails
pub fn encrypt_hex_key(plaintext: &[u8], hex_key: &str) -> Result<Vec<u8>> {
    let key = hex::decode(hex_key)?;
    encrypt(plaintext, &key)
}

/// Decrypts ciphertext using a hex-encoded key
///
/// # Arguments
/// * `combined` - The encrypted data with prepended nonce
/// * `hex_key` - The hex-encoded decryption key
///
/// # Returns
/// * `Ok(Vec<u8>)` - The decrypted plaintext
/// * `Err(CryptoError)` - If decryption fails
pub fn decrypt_hex_key(combined: &[u8], hex_key: &str) -> Result<Vec<u8>> {
    let key = hex::decode(hex_key)?;
    decrypt(combined, &key)
}

// Internal helper functions

fn encrypt_with_cipher<C>(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>>
where
    C: KeyInit + Aead + AeadCore,
{
    let cipher = C::new(GenericArray::from_slice(key));
    let nonce = C::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    // Prepend nonce to ciphertext
    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

fn decrypt_with_cipher<C>(combined: &[u8], key: &[u8]) -> Result<Vec<u8>>
where
    C: KeyInit + Aead + AeadCore,
{
    // AES-GCM typically uses 12-byte nonces
    let nonce_size = 12;

    if combined.len() < nonce_size {
        return Err(CryptoError::DataTooShort {
            expected: nonce_size,
            actual: combined.len(),
        });
    }

    let (nonce_bytes, ciphertext) = combined.split_at(nonce_size);
    let nonce = GenericArray::from_slice(nonce_bytes);
    let cipher = C::new(GenericArray::from_slice(key));

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, TEE world!";
        let key = b"0123456789abcdef"; // 16 bytes for AES-128

        let encrypted = encrypt(plaintext, key).unwrap();
        assert!(encrypted.len() > plaintext.len());

        let decrypted = decrypt(&encrypted, key).unwrap();
        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_encrypt_decrypt_hex_key() {
        let plaintext = b"Secure data";
        let hex_key = "0123456789abcdef0123456789abcdef"; // 32 hex chars = 16 bytes

        let encrypted = encrypt_hex_key(plaintext, hex_key).unwrap();
        let decrypted = decrypt_hex_key(&encrypted, hex_key).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_different_key_sizes() {
        let plaintext = b"Test data";

        // Test AES-128 (16 bytes)
        let key_128 = vec![0u8; 16];
        let encrypted = encrypt(plaintext, &key_128).unwrap();
        let decrypted = decrypt(&encrypted, &key_128).unwrap();
        assert_eq!(plaintext, &decrypted[..]);

        // Test AES-256 (32 bytes)
        let key_256 = vec![0u8; 32];
        let encrypted = encrypt(plaintext, &key_256).unwrap();
        let decrypted = decrypt(&encrypted, &key_256).unwrap();
        assert_eq!(plaintext, &decrypted[..]);
    }

    #[test]
    fn test_invalid_key_length() {
        let plaintext = b"Test";
        let invalid_key = vec![0u8; 15]; // Invalid length

        let result = encrypt(plaintext, &invalid_key);
        assert!(matches!(result, Err(CryptoError::InvalidKeyLength(15))));
    }

    #[test]
    fn test_decrypt_too_short() {
        let key = vec![0u8; 16];
        let too_short = vec![0u8; 5]; // Less than nonce size

        let result = decrypt(&too_short, &key);
        assert!(matches!(result, Err(CryptoError::DataTooShort { .. })));
    }
}
