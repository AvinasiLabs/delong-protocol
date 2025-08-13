use super::error::{Error, Result};
use dstack_sdk::dstack_client::DstackClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// TEE client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Endpoint for dstack service (Unix socket path or HTTP URL)
    pub endpoint: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self { endpoint: None }
    }
}

/// TEE derived key with attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Key {
    /// Private key in hex format
    pub key: String,
    /// Signature chain (certificate chain)
    pub signature_chain: Vec<String>,
    /// Key derivation path
    pub path: String,
    /// Key purpose
    pub purpose: Option<String>,
}

/// TEE client for secure operations
pub struct Client {
    inner: Arc<DstackClient>,
}

impl Client {
    /// Create a new TEE client instance
    pub fn new(config: ClientConfig) -> Self {
        let inner = Arc::new(DstackClient::new(config.endpoint.as_deref()));
        Self { inner }
    }

    /// Derive a key with attestation
    pub async fn derive_key(&self, path: &str, purpose: Option<&str>) -> Result<Key> {
        debug!("Deriving key for path: {}, purpose: {:?}", path, purpose);

        let response = self
            .inner
            .get_key(Some(path.to_string()), purpose.map(|p| p.to_string()))
            .await
            .map_err(|e| Error::key_derivation(path, Some(e.to_string())))?;

        Ok(Key {
            key: response.key,
            signature_chain: response.signature_chain,
            path: path.to_string(),
            purpose: purpose.map(|p| p.to_string()),
        })
    }

    /// Emit an event to the TEE event log
    pub async fn emit_event(&self, event_name: &str, payload: &[u8]) -> Result<()> {
        debug!("Emitting event: {}", event_name);

        self.inner
            .emit_event(event_name.to_string(), payload.to_vec())
            .await
            .map_err(|e| Error::EventEmission {
                event_name: event_name.to_string(),
                reason: e.to_string(),
            })?;

        info!("Event '{}' emitted successfully", event_name);
        Ok(())
    }

    /// Get the underlying dstack client for advanced operations
    pub fn inner(&self) -> &Arc<DstackClient> {
        &self.inner
    }
}

/// Builder for creating a TEE client with custom configuration
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = Some(endpoint.into());
        self
    }

    pub fn build(self) -> Client {
        Client::new(self.config)
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = ClientBuilder::new()
            .endpoint("http://localhost:11010")
            .build();

        // Test that we can create a client
        assert!(client.inner.as_ref() as *const _ != std::ptr::null());
    }

    #[tokio::test]
    async fn test_key_derivation() {
        let client = super::super::test_helpers::create_test_client();

        let result = client.derive_key("test-path", Some("test-purpose")).await;
        assert!(result.is_ok());

        if let Ok(key) = result {
            assert_eq!(key.path, "test-path");
            assert_eq!(key.purpose, Some("test-purpose".to_string()));
            assert!(!key.key.is_empty());
        }
    }
}
