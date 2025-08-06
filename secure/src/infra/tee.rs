use super::tee_error::{TeeError, TeeResult};
use dstack_sdk::dstack_client::DstackClient;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// TEE service configuration
#[derive(Debug, Clone)]
pub struct TeeConfig {
    /// Endpoint for dstack service (Unix socket path or HTTP URL)
    pub endpoint: Option<String>,
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self { endpoint: None }
    }
}

/// TEE derived key with attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeKey {
    /// Private key in hex format
    pub key: String,
    /// Signature chain (certificate chain)
    pub signature_chain: Vec<String>,
    /// Key derivation path
    pub path: String,
    /// Key purpose
    pub purpose: Option<String>,
}

/// TEE service for secure operations
pub struct TeeService {
    client: Arc<DstackClient>,
}

impl TeeService {
    /// Create a new TEE service instance
    pub fn new(config: TeeConfig) -> Self {
        let client = Arc::new(DstackClient::new(config.endpoint.as_deref()));
        Self { client }
    }

    /// Derive a key with attestation
    pub async fn derive_key(&self, path: &str, purpose: Option<&str>) -> TeeResult<TeeKey> {
        debug!("Deriving key for path: {}, purpose: {:?}", path, purpose);

        let response = self
            .client
            .get_key(Some(path.to_string()), purpose.map(|p| p.to_string()))
            .await
            .map_err(|e| TeeError::key_derivation(path, Some(e.to_string())))?;

        Ok(TeeKey {
            key: response.key,
            signature_chain: response.signature_chain,
            path: path.to_string(),
            purpose: purpose.map(|p| p.to_string()),
        })
    }

    /// Emit an event to the TEE event log
    pub async fn emit_event(&self, event_name: &str, payload: &[u8]) -> TeeResult<()> {
        debug!("Emitting event: {}", event_name);

        self.client
            .emit_event(event_name.to_string(), payload.to_vec())
            .await
            .map_err(|e| TeeError::EventEmission {
                event_name: event_name.to_string(),
                reason: e.to_string(),
            })?;

        info!("Event '{}' emitted successfully", event_name);
        Ok(())
    }
}

/// Builder for creating a TEE service with custom configuration
pub struct TeeServiceBuilder {
    config: TeeConfig,
}

impl TeeServiceBuilder {
    pub fn new() -> Self {
        Self {
            config: TeeConfig::default(),
        }
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = Some(endpoint.into());
        self
    }

    pub fn build(self) -> TeeService {
        TeeService::new(self.config)
    }
}

impl Default for TeeServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    /// Create a TeeService configured for testing with dstack simulator
    pub fn create_test_tee_service() -> TeeService {
        let endpoint = std::env::var("DSTACK_SIMULATOR_ENDPOINT").unwrap_or_else(|_| {
            "/var/lib/docker/volumes/secure_simulator_sockets_dev/_data/dstack.sock".to_string()
        });

        TeeServiceBuilder::new().endpoint(endpoint).build()
    }
}
