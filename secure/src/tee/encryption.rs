use sha2::{Sha256, Digest};
use rand::RngCore;
use serde::{Serialize, Deserialize};
use tracing::info;
use common::ApiResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionResult {
    /// The encrypted data
    pub ciphertext: Vec<u8>,
    /// The nonce used for encryption 
    pub nonce: Vec<u8>,
    /// Dataset hash for key derivation
    pub dataset_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptionRequest {
    /// The encrypted data to decrypt
    pub ciphertext: Vec<u8>,
    /// The nonce used during encryption
    pub nonce: Vec<u8>,
    /// Dataset hash for key derivation
    pub dataset_hash: String,
}

/// Simple XOR-based encryption for testing purposes
/// In production, this would use AES-GCM or other proven cryptography
pub struct MockEncryption;

impl MockEncryption {
    /// Encrypt data using XOR with key
    pub fn encrypt(plaintext: &[u8], key: &[u8]) -> ApiResult<(Vec<u8>, Vec<u8>)> {
        if key.is_empty() {
            return Err(common::ApiError::InternalError("Key cannot be empty".to_string()));
        }

        // Generate random nonce
        let mut nonce = vec![0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);

        // XOR encryption with key cycling
        let mut ciphertext = Vec::new();
        for (i, &byte) in plaintext.iter().enumerate() {
            let key_byte = key[i % key.len()];
            ciphertext.push(byte ^ key_byte);
        }

        info!(
            plaintext_len = plaintext.len(),
            ciphertext_len = ciphertext.len(),
            "Data encrypted with mock encryption"
        );

        Ok((ciphertext, nonce))
    }

    /// Decrypt data using XOR with key
    pub fn decrypt(ciphertext: &[u8], _nonce: &[u8], key: &[u8]) -> ApiResult<Vec<u8>> {
        if key.is_empty() {
            return Err(common::ApiError::InternalError("Key cannot be empty".to_string()));
        }

        // XOR decryption (same as encryption for XOR)
        let mut plaintext = Vec::new();
        for (i, &byte) in ciphertext.iter().enumerate() {
            let key_byte = key[i % key.len()];
            plaintext.push(byte ^ key_byte);
        }

        info!(
            ciphertext_len = ciphertext.len(),
            plaintext_len = plaintext.len(),
            "Data decrypted with mock encryption"
        );

        Ok(plaintext)
    }
}

/// Hash-based key derivation
pub struct KeyDerivation;

impl KeyDerivation {
    /// Derive a key from password and salt using SHA256
    pub fn derive_key(password: &[u8], salt: &[u8], key_length: usize) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(salt);
        hasher.update(b"delong-key-derivation");
        
        let mut key = hasher.finalize().to_vec();
        
        // Extend key if needed
        while key.len() < key_length {
            let mut hasher = Sha256::new();
            hasher.update(&key);
            hasher.update(salt);
            key.extend_from_slice(&hasher.finalize());
        }
        
        key.truncate(key_length);
        key
    }

    /// Create a secure key from dataset hash and author
    pub fn create_dataset_key(dataset_hash: &str, author: &str, purpose: &str) -> Vec<u8> {
        let salt = format!("delong-{}-{}", author, purpose);
        Self::derive_key(dataset_hash.as_bytes(), salt.as_bytes(), 32)
    }
}

/// Stream encryption for large data
pub struct StreamEncryption;

impl StreamEncryption {
    /// Encrypt data in chunks
    pub fn encrypt_stream(
        data: &[u8], 
        key: &[u8], 
        chunk_size: usize
    ) -> ApiResult<Vec<EncryptionResult>> {
        let mut results = Vec::new();
        let chunks = data.chunks(chunk_size);
        
        for (i, chunk) in chunks.enumerate() {
            let (ciphertext, nonce) = MockEncryption::encrypt(chunk, key)?;
            
            results.push(EncryptionResult {
                ciphertext,
                nonce,
                dataset_hash: format!("stream_chunk_{}", i),
            });
        }
        
        info!(
            total_chunks = results.len(),
            chunk_size = chunk_size,
            "Stream encryption completed"
        );
        
        Ok(results)
    }

    /// Decrypt stream chunks back to original data
    pub fn decrypt_stream(
        chunks: &[EncryptionResult],
        key: &[u8]
    ) -> ApiResult<Vec<u8>> {
        let mut plaintext = Vec::new();
        
        for chunk in chunks {
            let decrypted = MockEncryption::decrypt(
                &chunk.ciphertext,
                &chunk.nonce,
                key
            )?;
            plaintext.extend_from_slice(&decrypted);
        }
        
        info!(
            chunks_count = chunks.len(),
            total_size = plaintext.len(),
            "Stream decryption completed"
        );
        
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_encrypt_decrypt() {
        let plaintext = b"This is a test message";
        let key = b"test_key_32_bytes_long_for_testing";

        let (ciphertext, nonce) = MockEncryption::encrypt(plaintext, key).unwrap();
        assert_ne!(ciphertext, plaintext);
        assert_eq!(nonce.len(), 12);

        let decrypted = MockEncryption::decrypt(&ciphertext, &nonce, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_derivation() {
        let key1 = KeyDerivation::create_dataset_key("dataset1", "author1", "encryption");
        let key2 = KeyDerivation::create_dataset_key("dataset2", "author1", "encryption");
        let key3 = KeyDerivation::create_dataset_key("dataset1", "author2", "encryption");

        assert_eq!(key1.len(), 32);
        assert_ne!(key1, key2); // Different datasets
        assert_ne!(key1, key3); // Different authors
    }

    #[test]
    fn test_stream_encryption() {
        let data = b"This is a long message that will be encrypted in chunks";
        let key = b"stream_key_32_bytes_long_testing";
        let chunk_size = 10;

        let encrypted_chunks = StreamEncryption::encrypt_stream(data, key, chunk_size).unwrap();
        assert!(encrypted_chunks.len() > 1);

        let decrypted = StreamEncryption::decrypt_stream(&encrypted_chunks, key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_empty_key_fails() {
        let plaintext = b"test data";
        let empty_key = b"";

        let result = MockEncryption::encrypt(plaintext, empty_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_key_derivation() {
        let key1 = KeyDerivation::create_dataset_key("dataset1", "author1", "encryption");
        let key2 = KeyDerivation::create_dataset_key("dataset1", "author1", "encryption");
        
        // Same inputs should produce same key
        assert_eq!(key1, key2);
    }
} 