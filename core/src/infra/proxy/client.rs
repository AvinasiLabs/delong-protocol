//! Proxy client for forwarding requests to the Secure service
//!
//! This module handles:
//! - Request forwarding to TEE-deployed Secure service
//! - Internal JWT generation for service-to-service authentication
//! - Request/response transformation and error handling

use axum::{
    body::Body,
    http::{HeaderMap, Method},
    response::Response,
};
use bytes::Bytes;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

use crate::config::ProxyConfig;
use avinapi::prelude::*;

/// Authentication context for forwarded requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    /// User ID from JWT or API key
    pub user_id: String,
    /// User email
    pub email: String,
    /// Authentication method (jwt or api_key)
    pub auth_method: String,
    /// Tenant/organization ID if applicable
    pub tenant_id: Option<String>,
    /// Permission scopes
    pub scopes: Vec<String>,
    /// Original client IP
    pub client_ip: Option<String>,
    /// Request ID for tracing
    pub request_id: String,
}

/// Internal JWT claims for service-to-service authentication
#[derive(Debug, Serialize, Deserialize)]
struct InternalJwtClaims {
    /// Subject (user ID)
    sub: String,
    /// Issued at timestamp
    iat: u64,
    /// Expiration timestamp
    exp: u64,
    /// Issuer (always "delong-core")
    iss: String,
    /// Audience (always "delong-secure")
    aud: String,
    /// Authentication context
    context: AuthContext,
    /// Request signature for integrity
    request_signature: RequestSignature,
}

/// Request signature for integrity verification
#[derive(Debug, Serialize, Deserialize)]
struct RequestSignature {
    /// HTTP method
    method: String,
    /// Request path
    path: String,
    /// SHA256 hash of request body (if present)
    body_digest: Option<String>,
    /// Timestamp
    timestamp: u64,
}

/// Proxy client for forwarding requests to Secure service
pub struct ProxyClient {
    client: Client,
    pub config: ProxyConfig,
    jwt_key: EncodingKey,
}

impl ProxyClient {
    /// Create a new proxy client
    pub fn new(config: ProxyConfig, jwt_secret: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()?;

        let jwt_key = EncodingKey::from_secret(jwt_secret.as_bytes());

        Ok(Self {
            client,
            config,
            jwt_key,
        })
    }

    /// Forward a request to the Secure service
    pub async fn forward_request(
        &self,
        method: Method,
        path: &str,
        headers: HeaderMap,
        body: Option<Bytes>,
        auth_context: AuthContext,
    ) -> AppResult<Response<Body>> {
        // Debug: Log the path we're about to use
        info!("ProxyClient: forward_request called with path: '{}'", path);

        // Generate internal JWT
        let internal_jwt = self.generate_internal_jwt(&method, path, &body, &auth_context)?;

        // Build the target URL
        let target_url = format!("{}{}", self.config.secure_service_url, path);

        if self.config.enable_debug_logging {
            debug!(
                "Forwarding {} request to: {} with auth_method: {}",
                method, target_url, auth_context.auth_method
            );
        }

        // Build the request
        let mut request = self.client.request(method.clone(), &target_url);

        // Add internal JWT header
        request = request.header("X-Internal-JWT", internal_jwt);

        // Forward relevant headers
        for (key, value) in headers.iter() {
            let key_str = key.as_str();
            // Skip host and connection headers
            if !matches!(key_str, "host" | "connection" | "content-length") {
                request = request.header(key.clone(), value.clone());
            }
        }

        // Add body if present
        if let Some(body_bytes) = body {
            request = request.body(body_bytes);
        }

        // Execute request with retries
        let mut retries = 0;
        loop {
            match request.try_clone().unwrap().send().await {
                Ok(response) => {
                    return self.transform_response(response).await;
                }
                Err(e) => {
                    if retries >= self.config.max_retries {
                        error!("Failed to forward request after {} retries: {}", retries, e);
                        return Err(AppError::Internal(format!(
                            "Secure service unavailable: {}",
                            e
                        )));
                    }
                    warn!("Request failed (attempt {}): {}", retries + 1, e);
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(100 * (1 << retries))).await;
                }
            }
        }
    }

    /// Generate internal JWT for service-to-service authentication
    fn generate_internal_jwt(
        &self,
        method: &Method,
        path: &str,
        body: &Option<Bytes>,
        auth_context: &AuthContext,
    ) -> AppResult<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::Internal(format!("Time error: {}", e)))?
            .as_secs();

        // Calculate body digest if present
        let body_digest = body.as_ref().map(|b| {
            let mut hasher = Sha256::new();
            hasher.update(b);
            format!("{:x}", hasher.finalize())
        });

        let request_signature = RequestSignature {
            method: method.to_string(),
            path: path.to_string(),
            body_digest,
            timestamp: now,
        };

        info!(
            "ProxyClient: Generating internal JWT with signature - method: {}, path: '{}'",
            method, path
        );

        let claims = InternalJwtClaims {
            sub: auth_context.user_id.clone(),
            iat: now,
            exp: now + self.config.jwt_expiration_seconds,
            iss: "delong-core".to_string(),
            aud: "delong-secure".to_string(),
            context: auth_context.clone(),
            request_signature,
        };

        encode(&Header::new(Algorithm::HS256), &claims, &self.jwt_key)
            .map_err(|e| AppError::Internal(format!("JWT generation failed: {}", e)))
    }

    /// Transform the response from Secure service
    async fn transform_response(&self, response: reqwest::Response) -> AppResult<Response<Body>> {
        let status = response.status();
        let headers = response.headers().clone();

        // Log response if debug enabled
        if self.config.enable_debug_logging {
            debug!("Received response with status: {}", status);
        }

        // Get response body
        let body_bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read response body: {}", e)))?;

        // Build Axum response
        let mut builder = Response::builder().status(status);

        // Forward relevant headers
        for (key, value) in headers.iter() {
            let key_str = key.as_str();
            // Skip connection-related headers
            if !matches!(
                key_str,
                "connection" | "transfer-encoding" | "content-encoding"
            ) {
                builder = builder.header(key.clone(), value.clone());
            }
        }

        // Set content-length
        builder = builder.header("content-length", body_bytes.len());

        builder
            .body(Body::from(body_bytes))
            .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))
    }

    /// Health check for Secure service
    pub async fn health_check(&self) -> AppResult<bool> {
        let url = format!("{}/health", self.config.secure_service_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                warn!("Secure service health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

/// Helper function to create a proxy client from config
pub async fn create_proxy_client(config: ProxyConfig, jwt_secret: &str) -> AppResult<ProxyClient> {
    ProxyClient::new(config, jwt_secret)
        .map_err(|e| AppError::Config(format!("Failed to create proxy client: {}", e)))
}
