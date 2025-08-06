use super::tee_error::{TeeError, TeeResult};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::tee::{TeeKey, TeeService};

/// Key context for upload report encryption
pub const KEY_CTX_UPLOAD_REPORT_ENCRYPT: &str = "upload-report-key/encrypt-user-test-report";

/// Key context prefix for task-specific encryption
pub const KEY_CTX_TASK_ENCRYPT_PREFIX: &str = "task-encrypt-key";

/// Key context prefix for data encryption
pub const KEY_CTX_DATA_ENCRYPT_PREFIX: &str = "data-encrypt-key";

/// Symmetric key with attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestatedSymmetricKey {
    /// The symmetric key bytes
    pub key: Vec<u8>,
    /// Associated TEE key with attestation
    pub tee_key: TeeKey,
    /// Key purpose/context
    pub context: String,
    /// Key length in bytes
    pub key_length: usize,
}

/// Key derivation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDerivationParams {
    /// Salt for HKDF (optional)
    pub salt: Option<Vec<u8>>,
    /// Info for HKDF
    pub info: Vec<u8>,
    /// Desired output key length
    pub key_length: usize,
}

impl Default for KeyDerivationParams {
    fn default() -> Self {
        Self {
            salt: None,
            info: b"delong-key-derivation".to_vec(),
            key_length: 32, // 256-bit key by default
        }
    }
}

/// TEE-backed key management service
pub struct TeeKeyManagementService {
    tee_service: Arc<TeeService>,
    key_cache: Arc<RwLock<HashMap<String, AttestatedSymmetricKey>>>,
}

impl TeeKeyManagementService {
    /// Create a new key management service
    pub fn new(tee_service: Arc<TeeService>) -> Self {
        Self {
            tee_service,
            key_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or derive a symmetric key for the given context
    pub async fn get_symmetric_key(
        &self,
        context: &str,
        params: Option<KeyDerivationParams>,
    ) -> TeeResult<AttestatedSymmetricKey> {
        // Check cache first
        {
            let cache = self.key_cache.read().await;
            if let Some(key) = cache.get(context) {
                debug!("Using cached symmetric key for context: {}", context);
                return Ok(key.clone());
            }
        }

        // Derive new key
        info!("Deriving new symmetric key for context: {}", context);
        let params = params.unwrap_or_default();

        // Get master key from TEE
        let tee_key = self
            .tee_service
            .derive_key(context, Some("symmetric"))
            .await
            .map_err(|e| {
                TeeError::key_derivation(
                    context,
                    Some(format!("Failed to derive master key from TEE: {}", e)),
                )
            })?;

        // Derive symmetric key using HKDF
        let master_key = hex::decode(&tee_key.key)?;
        let derived_key = self.derive_key_hkdf(&master_key, &params)?;

        let attested_key = AttestatedSymmetricKey {
            key: derived_key,
            tee_key,
            context: context.to_string(),
            key_length: params.key_length,
        };

        // Cache the key
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(context.to_string(), attested_key.clone());
        }

        info!(
            "Derived symmetric key for context: {} (length: {} bytes)",
            context, params.key_length
        );

        Ok(attested_key)
    }

    /// Get encryption key for upload reports
    pub async fn get_upload_report_key(&self) -> TeeResult<AttestatedSymmetricKey> {
        let params = KeyDerivationParams {
            salt: Some(b"upload-report-salt".to_vec()),
            info: b"encrypt-user-test-report".to_vec(),
            key_length: 32, // AES-256
        };

        self.get_symmetric_key(KEY_CTX_UPLOAD_REPORT_ENCRYPT, Some(params))
            .await
    }

    /// Get encryption key for a specific task
    pub async fn get_task_encryption_key(
        &self,
        task_id: &str,
    ) -> TeeResult<AttestatedSymmetricKey> {
        let context = format!("{}/{}", KEY_CTX_TASK_ENCRYPT_PREFIX, task_id);
        let params = KeyDerivationParams {
            salt: Some(format!("task-{}-salt", task_id).into_bytes()),
            info: format!("encrypt-task-{}", task_id).into_bytes(),
            key_length: 32,
        };

        self.get_symmetric_key(&context, Some(params)).await
    }

    /// Get encryption key for data encryption
    pub async fn get_data_encryption_key(
        &self,
        data_id: &str,
    ) -> TeeResult<AttestatedSymmetricKey> {
        let context = format!("{}/{}", KEY_CTX_DATA_ENCRYPT_PREFIX, data_id);
        let params = KeyDerivationParams {
            salt: Some(format!("data-{}-salt", data_id).into_bytes()),
            info: format!("encrypt-data-{}", data_id).into_bytes(),
            key_length: 32,
        };

        self.get_symmetric_key(&context, Some(params)).await
    }

    /// Derive a key using HKDF
    fn derive_key_hkdf(&self, ikm: &[u8], params: &KeyDerivationParams) -> TeeResult<Vec<u8>> {
        let mut okm = vec![0u8; params.key_length];
        let hk = Hkdf::<Sha256>::new(params.salt.as_deref(), ikm);
        hk.expand(&params.info, &mut okm)
            .map_err(|e| TeeError::HkdfFailed(format!("HKDF expansion failed: {}", e)))?;
        Ok(okm)
    }

    /// Derive a key with custom HKDF parameters
    pub async fn derive_custom_key(
        &self,
        context: &str,
        salt: Option<&[u8]>,
        info: &[u8],
        key_length: usize,
    ) -> TeeResult<Vec<u8>> {
        // Get master key from cache or derive it
        let attested_key = self.get_symmetric_key(context, None).await?;

        // Derive custom key using HKDF
        let params = KeyDerivationParams {
            salt: salt.map(|s| s.to_vec()),
            info: info.to_vec(),
            key_length,
        };

        self.derive_key_hkdf(&attested_key.key, &params)
    }

    /// Clear the key cache
    pub async fn clear_cache(&self) {
        let mut cache = self.key_cache.write().await;
        cache.clear();
        info!("Cleared symmetric key cache");
    }

    /// Get all cached key contexts
    pub async fn list_cached_keys(&self) -> Vec<String> {
        let cache = self.key_cache.read().await;
        cache.keys().cloned().collect()
    }

    /// Generate a random nonce using TEE
    pub async fn generate_nonce(&self, length: usize) -> TeeResult<Vec<u8>> {
        // Use TEE to derive a nonce
        let context = format!("nonce/{}", uuid::Uuid::new_v4());
        let params = KeyDerivationParams {
            salt: Some(b"nonce-salt".to_vec()),
            info: b"generate-nonce".to_vec(),
            key_length: length,
        };

        let key = self.get_symmetric_key(&context, Some(params)).await?;
        Ok(key.key)
    }
}

/// Builder for creating a key management service
pub struct TeeKeyManagementServiceBuilder {
    tee_service: Option<Arc<TeeService>>,
}

impl TeeKeyManagementServiceBuilder {
    pub fn new() -> Self {
        Self { tee_service: None }
    }

    pub fn tee_service(mut self, service: Arc<TeeService>) -> Self {
        self.tee_service = Some(service);
        self
    }

    pub fn build(self) -> TeeResult<TeeKeyManagementService> {
        let tee_service = self.tee_service.ok_or(TeeError::ServiceUnavailable)?;

        Ok(TeeKeyManagementService::new(tee_service))
    }
}

impl Default for TeeKeyManagementServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::tee::test_helpers;

    #[tokio::test]
    async fn test_symmetric_key_derivation() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let key_service = TeeKeyManagementService::new(tee_service);

        let key = key_service.get_symmetric_key("test-context", None).await;
        assert!(key.is_ok());

        if let Ok(attested_key) = key {
            assert_eq!(attested_key.context, "test-context");
            assert_eq!(attested_key.key_length, 32); // Default length
            assert_eq!(attested_key.key.len(), 32);
        }
    }

    #[tokio::test]
    async fn test_upload_report_key() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let key_service = TeeKeyManagementService::new(tee_service);

        let key = key_service.get_upload_report_key().await;
        assert!(key.is_ok());

        if let Ok(attested_key) = key {
            assert_eq!(attested_key.context, KEY_CTX_UPLOAD_REPORT_ENCRYPT);
            assert_eq!(attested_key.key.len(), 32);
        }
    }

    #[tokio::test]
    async fn test_task_encryption_key() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let key_service = TeeKeyManagementService::new(tee_service);

        let task_id = "task-123";
        let key = key_service.get_task_encryption_key(task_id).await;
        assert!(key.is_ok());

        if let Ok(attested_key) = key {
            assert!(attested_key.context.contains(KEY_CTX_TASK_ENCRYPT_PREFIX));
            assert!(attested_key.context.contains(task_id));
            assert_eq!(attested_key.key.len(), 32);
        }
    }

    #[tokio::test]
    async fn test_key_caching() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let key_service = TeeKeyManagementService::new(tee_service);

        let context = "cache-test";

        // First call should derive new key
        let key1 = key_service.get_symmetric_key(context, None).await;
        assert!(key1.is_ok());

        // Second call should use cached key
        let key2 = key_service.get_symmetric_key(context, None).await;
        assert!(key2.is_ok());

        if let (Ok(k1), Ok(k2)) = (key1, key2) {
            // Should return the same key
            assert_eq!(k1.key, k2.key);
        }
    }

    #[tokio::test]
    async fn test_custom_key_derivation() {
        let tee_service = Arc::new(test_helpers::create_test_tee_service());
        let key_service = TeeKeyManagementService::new(tee_service);

        let custom_key = key_service
            .derive_custom_key(
                "custom-context",
                Some(b"custom-salt"),
                b"custom-info",
                16, // 128-bit key
            )
            .await;

        assert!(custom_key.is_ok());
        if let Ok(key) = custom_key {
            assert_eq!(key.len(), 16);
        }
    }
}
