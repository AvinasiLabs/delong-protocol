//! Adapter layer for dstack-sdk to isolate version differences
//!
//! This module provides an abstraction layer between our code using alloy 1.0.1
//! and dstack-sdk which uses alloy 0.15. It handles type conversions and
//! prevents version conflicts from propagating through our codebase.

use super::tee_error::{SigningOperation, TeeError, TeeResult};
use alloy::primitives::{Address, B256};
use alloy::signers::local::PrivateKeySigner;
use dstack_sdk::dstack_client::GetKeyResponse;

/// Convert a hex string to our Address type
#[allow(dead_code)]
fn hex_to_address(hex: &str) -> TeeResult<Address> {
    hex.parse::<Address>()
        .map_err(|e| TeeError::Other(format!("Failed to parse address from hex: {}", e)))
}

/// Convert a hex string to our B256 type
#[allow(dead_code)]
fn hex_to_b256(hex: &str) -> TeeResult<B256> {
    hex.parse::<B256>()
        .map_err(|e| TeeError::Other(format!("Failed to parse B256 from hex: {}", e)))
}

/// Convert a TEE key response to an Ethereum account using our alloy version
///
/// This function wraps dstack_sdk's to_account function and handles the type
/// conversion between different alloy versions.
pub fn tee_key_to_ethereum_account(key_response: &GetKeyResponse) -> TeeResult<PrivateKeySigner> {
    // Call dstack-sdk's to_account function
    let dstack_signer = dstack_sdk::ethereum::to_account(key_response).map_err(|e| {
        TeeError::KeyConversion(format!(
            "Failed to convert TEE key to Ethereum account: {}",
            e
        ))
    })?;

    // Extract the private key from dstack's signer
    // Since both versions use the same underlying secp256k1 library,
    // we can extract the raw key bytes and recreate the signer

    // Get the address as a string from dstack's signer
    let _address_str = format!("{:?}", dstack_signer.address());

    // For the private key, we need to extract it from the dstack signer
    // This is a bit tricky since we can't directly access the private key
    // We'll need to use the raw key bytes from the TEE response

    // Parse the key bytes from the TEE response
    let key_bytes = hex::decode(&key_response.key.trim_start_matches("0x"))?;

    // Create a new signer with our alloy version
    let signer = PrivateKeySigner::from_slice(&key_bytes).map_err(|e| {
        TeeError::KeyConversion(format!("Failed to create signer from key bytes: {}", e))
    })?;

    Ok(signer)
}

/// Sign a message hash using dstack's signer and return our signature type
///
/// This is a placeholder for future implementation if we need to handle
/// signature type conversions between alloy versions.
pub async fn sign_with_dstack_signer(
    key_response: &GetKeyResponse,
    message_hash: &B256,
) -> TeeResult<alloy::signers::Signature> {
    // Get the signer using our conversion function
    let signer = tee_key_to_ethereum_account(key_response)?;

    // Use our signer to sign (this avoids version conflicts)
    // In alloy 0.15, we need to use the Signer trait
    use alloy::signers::Signer;
    let signature = signer
        .sign_hash(message_hash)
        .await
        .map_err(|e| TeeError::SigningFailed {
            operation: SigningOperation::Hash,
            reason: e.to_string(),
        })?;

    Ok(signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_address() {
        let hex = "0x742d35Cc6634C0532925a3b844Bc9e7595f7F1eD";
        let result = hex_to_address(hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hex_to_b256() {
        let hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = hex_to_b256(hex);
        assert!(result.is_ok());
    }
}
