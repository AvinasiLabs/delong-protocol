//! TEE (Trusted Execution Environment) integration tests
//!
//! This module tests the cryptographic operations, key management, and secure data processing
//! capabilities of the TEE component within the Secure Service.

use common::ApiError;
use secure::tee::{KeyVault, ClientKind, KeyContext};

#[tokio::test]
async fn test_key_vault_initialization() {
    // Test mock key vault initialization
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    
    // Verify attestation
    let verified = key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    assert!(verified);
    
    // Should be verified after verification
    assert!(key_vault.is_attestation_verified());
    
    println!("✅ Key vault initialization test passed");
}

#[tokio::test]
async fn test_dataset_key_derivation() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    let context = KeyContext::new(
        "test_dataset_hash_123".to_string(),
        "test_author".to_string(),
        "encryption".to_string(),
    );
    
    // Derive key for dataset
    let key1 = key_vault.derive_symmetric_key(&context).await
        .expect("Failed to derive dataset key");
    
    // Key should not be empty
    assert!(!key1.is_empty());
    assert_eq!(key1.len(), 32);
    
    // Deriving the same dataset key again should return the same key
    let key2 = key_vault.derive_symmetric_key(&context).await
        .expect("Failed to derive dataset key again");
    
    assert_eq!(key1, key2);
    
    // Should have one cached key
    assert_eq!(key_vault.cached_symmetric_keys_count(), 1);
    
    println!("✅ Dataset key derivation test passed");
}

#[tokio::test]
async fn test_data_encryption_decryption() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    let test_data = b"This is sensitive biomedical data that needs encryption";
    let dataset_id = "biomedical_dataset_v1";
    
    // Encrypt the data
    let encrypted_result = key_vault.encrypt_data(test_data, dataset_id).await
        .expect("Failed to encrypt data");
    
    // Verify encryption result structure
    assert!(!encrypted_result.encrypted_data.is_empty());
    assert!(!encrypted_result.nonce.is_empty());
    assert_eq!(encrypted_result.dataset_id, dataset_id);
    
    // Encrypted data should be different from original
    assert_ne!(encrypted_result.encrypted_data, test_data);
    
    // Decrypt the data
    let decrypted_data = key_vault.decrypt_data(&encrypted_result).await
        .expect("Failed to decrypt data");
    
    // Decrypted data should match original
    assert_eq!(decrypted_data, test_data);
    
    println!("✅ Data encryption/decryption test passed");
}

#[tokio::test]
async fn test_multiple_dataset_keys() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    let datasets = vec![
        "dataset_genomics_001",
        "dataset_medical_imaging_002", 
        "dataset_clinical_trials_003",
    ];
    
    let mut keys = Vec::new();
    
    // Derive keys for multiple datasets
    for dataset in &datasets {
        let context = KeyContext::new(
            dataset.to_string(),
            "test_author".to_string(),
            "encryption".to_string(),
        );
        let key = key_vault.derive_symmetric_key(&context).await
            .expect("Failed to derive dataset key");
        keys.push(key);
    }
    
    // All keys should be unique
    for i in 0..keys.len() {
        for j in i+1..keys.len() {
            assert_ne!(keys[i], keys[j]);
        }
    }
    
    // Should have 3 cached keys
    assert_eq!(key_vault.cached_symmetric_keys_count(), 3);
    
    println!("✅ Multiple dataset keys test passed");
}

#[tokio::test]
async fn test_encryption_with_different_datasets() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    let test_data = b"Same data, different datasets";
    let dataset1 = "dataset_1";
    let dataset2 = "dataset_2";
    
    // Encrypt the same data with different dataset keys
    let result1 = key_vault.encrypt_data(test_data, dataset1).await
        .expect("Failed to encrypt with dataset1");
    
    let result2 = key_vault.encrypt_data(test_data, dataset2).await
        .expect("Failed to encrypt with dataset2");
    
    // Results should be different (different keys)
    assert_ne!(result1.encrypted_data, result2.encrypted_data);
    assert_ne!(result1.dataset_id, result2.dataset_id);
    
    // Both should decrypt to the same original data
    let decrypted1 = key_vault.decrypt_data(&result1).await
        .expect("Failed to decrypt data1");
    
    let decrypted2 = key_vault.decrypt_data(&result2).await
        .expect("Failed to decrypt data2");
    
    assert_eq!(decrypted1, test_data);
    assert_eq!(decrypted2, test_data);
    
    println!("✅ Encryption with different datasets test passed");
}

#[tokio::test]
async fn test_key_cache_management() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    // Derive some keys
    for i in 0..5 {
        let context = KeyContext::new(
            format!("dataset_{}", i),
            "test_author".to_string(),
            "encryption".to_string(),
        );
        key_vault.derive_symmetric_key(&context).await
            .expect("Failed to derive key");
    }
    
    // Should have 5 cached keys
    assert_eq!(key_vault.cached_symmetric_keys_count(), 5);
    
    // Clear all keys
    key_vault.clear_cache();
    
    // Should have no cached keys
    assert_eq!(key_vault.cached_symmetric_keys_count(), 0);
    
    println!("✅ Key cache management test passed");
}

#[tokio::test]
async fn test_encryption_without_attestation() {
    // Create a phala key vault which starts unverified
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Phala);
    // Don't call verify_attestation - should be unverified
    
    let test_data = b"Test data";
    let dataset_id = "test_dataset";
    
    // Should fail without attestation for Phala client
    let result = key_vault.encrypt_data(test_data, dataset_id).await;
    assert!(result.is_err());
    
    if let Err(ApiError::Forbidden(msg)) = result {
        assert!(msg.contains("Attestation not verified"));
    } else {
        panic!("Expected Forbidden error with attestation message");
    }
    
    println!("✅ Encryption without attestation test passed");
}

#[tokio::test]
async fn test_large_data_encryption() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    // Create large test data (1MB)
    let large_data = vec![0x42u8; 1024 * 1024];
    let dataset_id = "large_dataset";
    
    // Encrypt the large data
    let start_time = std::time::Instant::now();
    let encrypted_result = key_vault.encrypt_data(&large_data, dataset_id).await
        .expect("Failed to encrypt large data");
    let encrypt_duration = start_time.elapsed();
    
    // Decrypt the data
    let start_time = std::time::Instant::now();
    let decrypted_data = key_vault.decrypt_data(&encrypted_result).await
        .expect("Failed to decrypt large data");
    let decrypt_duration = start_time.elapsed();
    
    // Verify the data is correct
    assert_eq!(decrypted_data, large_data);
    
    println!("✅ Large data encryption test passed (encrypt: {:?}, decrypt: {:?})", 
             encrypt_duration, decrypt_duration);
}

#[tokio::test]
async fn test_concurrent_key_derivation() {
    let key_vault = KeyVault::new_with_client_kind(ClientKind::Mock);
    key_vault.verify_attestation().await.expect("Failed to verify TEE attestation");
    
    let datasets: Vec<String> = (0..10).map(|i| format!("concurrent_dataset_{}", i)).collect();
    
    // Derive keys concurrently
    let mut handles = Vec::new();
    for dataset in datasets.clone() {
        let vault = key_vault.clone();
        let handle = tokio::spawn(async move {
            let context = KeyContext::new(
                dataset,
                "test_author".to_string(),
                "encryption".to_string(),
            );
            vault.derive_symmetric_key(&context).await
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut keys = Vec::new();
    for handle in handles {
        let key = handle.await.expect("Task failed").expect("Key derivation failed");
        keys.push(key);
    }
    
    // All keys should be unique
    for i in 0..keys.len() {
        for j in i+1..keys.len() {
            assert_ne!(keys[i], keys[j]);
        }
    }
    
    // Should have all keys cached
    assert_eq!(key_vault.cached_symmetric_keys_count(), 10);
    
    println!("✅ Concurrent key derivation test passed");
} 