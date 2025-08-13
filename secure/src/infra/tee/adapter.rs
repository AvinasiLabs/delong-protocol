//! Adapter module for converting dstack keys to Ethereum accounts

use super::error::{Error, Result};
use alloy::signers::local::PrivateKeySigner;
use dstack_sdk::dstack_client::GetKeyResponse;
use tracing::debug;

/// Convert a TEE key response to an Ethereum account
pub fn tee_key_to_ethereum_account(key_response: &GetKeyResponse) -> Result<PrivateKeySigner> {
    debug!("Converting TEE key to Ethereum account");

    // Parse the hex-encoded private key
    let key_bytes = hex::decode(&key_response.key)
        .map_err(|e| Error::KeyConversion(format!("Failed to decode hex key: {}", e)))?;

    // Ensure the key is exactly 32 bytes
    if key_bytes.len() != 32 {
        return Err(Error::KeyConversion(format!(
            "Invalid key length: expected 32 bytes, got {}",
            key_bytes.len()
        )));
    }

    // Convert Vec<u8> to [u8; 32]
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| Error::KeyConversion("Failed to convert key to fixed array".to_string()))?;

    // Create a PrivateKeySigner from the key bytes
    let signer = PrivateKeySigner::from_bytes(&key_array.into())
        .map_err(|e| Error::KeyConversion(format!("Failed to create signer: {}", e)))?;

    debug!("Successfully converted TEE key to Ethereum account");
    Ok(signer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_conversion() {
        // Test with a valid 32-byte key
        let key_response = GetKeyResponse {
            key: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            signature_chain: vec!["test_cert".to_string()],
        };

        let result = tee_key_to_ethereum_account(&key_response);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_hex() {
        let key_response = GetKeyResponse {
            key: "invalid_hex".to_string(),
            signature_chain: vec![],
        };

        let result = tee_key_to_ethereum_account(&key_response);
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("decode hex"));
        }
    }

    #[test]
    fn test_invalid_key_length() {
        // Test with a key that's not 32 bytes
        let key_response = GetKeyResponse {
            key: "0123456789abcdef".to_string(),
            signature_chain: vec![],
        };

        let result = tee_key_to_ethereum_account(&key_response);
        assert!(result.is_err());
    }
}
