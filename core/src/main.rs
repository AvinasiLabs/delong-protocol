//! Delong Protocol Core Service
//!
//! The core business logic service for the Delong privacy-preserving computation platform.
//! This service handles API key management, user authentication, and business logic operations.

use axum::{
    Router,
    extract::{Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    serve,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tokio::net::TcpListener;
use tracing::{error, info, instrument, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// API key management service
#[derive(Debug, Clone)]
pub struct ApiKeyService {
    /// In-memory storage for API keys (replace with database in production)
    pub keys: Arc<Mutex<HashMap<String, StoredApiKey>>>,
    /// Storage for API key by hash lookup
    pub key_hashes: Arc<Mutex<HashMap<String, String>>>, // hash -> key_id
}

/// Stored API key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub key_hash: String,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
    pub is_active: bool,
    pub created_at: SystemTime,
    pub last_used_at: Option<SystemTime>,
    pub expires_at: Option<SystemTime>,
}

/// User permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    DataRead,
    DataWrite,
    DataDelete,
    AlgorithmSubmit,
    AlgorithmRead,
    ApiKeyManage,
    AdminAccess,
}

/// Rate limiting tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitTier {
    Basic,
    Premium,
    Enterprise,
    Internal,
}

/// Request to create a new API key
#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub expires_in_days: Option<u32>,
}

/// Response for API key creation
#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub api_key: String,
    pub user_id: String,
    pub name: String,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
    pub expires_at: Option<String>,
    pub created_at: String,
}

/// Request to validate an API key
#[derive(Debug, Deserialize)]
pub struct ValidateApiKeyRequest {
    pub api_key: String,
}

/// Response for API key validation
#[derive(Debug, Serialize)]
pub struct ValidateApiKeyResponse {
    pub is_valid: bool,
    pub key_id: Option<String>,
    pub user_id: Option<String>,
    pub permissions: Option<Vec<Permission>>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub last_used_at: Option<String>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub timestamp: String,
}

impl ApiKeyService {
    /// Create a new API key service
    pub fn new() -> Self {
        Self {
            keys: Arc::new(Mutex::new(HashMap::new())),
            key_hashes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new API key
    pub async fn create_api_key(
        &self,
        request: CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponse, String> {
        let key_id = format!("key_{}", uuid::Uuid::new_v4().simple());
        let api_key = self.generate_api_key();
        let key_hash = self.hash_api_key(&api_key);

        let expires_at = request
            .expires_in_days
            .map(|days| SystemTime::now() + Duration::from_secs(days as u64 * 24 * 3600));

        let stored_key = StoredApiKey {
            id: key_id.clone(),
            user_id: request.user_id.clone(),
            name: request.name.clone(),
            description: request.description.clone(),
            key_hash: key_hash.clone(),
            permissions: request.permissions.clone(),
            rate_limit_tier: request.rate_limit_tier.unwrap_or(RateLimitTier::Basic),
            is_active: true,
            created_at: SystemTime::now(),
            last_used_at: None,
            expires_at,
        };

        // Store the key
        {
            let mut keys = self
                .keys
                .lock()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;
            keys.insert(key_id.clone(), stored_key.clone());
        }

        // Store hash mapping
        {
            let mut hashes = self
                .key_hashes
                .lock()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;
            hashes.insert(key_hash, key_id.clone());
        }

        Ok(CreateApiKeyResponse {
            id: key_id,
            api_key,
            user_id: request.user_id,
            name: request.name,
            permissions: request.permissions,
            rate_limit_tier: stored_key.rate_limit_tier,
            expires_at: expires_at.map(|t| format_system_time(t)),
            created_at: format_system_time(stored_key.created_at),
        })
    }

    /// Validate an API key
    pub async fn validate_api_key(
        &self,
        request: ValidateApiKeyRequest,
    ) -> Result<ValidateApiKeyResponse, String> {
        let key_hash = self.hash_api_key(&request.api_key);

        // Find key by hash
        let key_id = {
            let hashes = self
                .key_hashes
                .lock()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;
            hashes.get(&key_hash).cloned()
        };

        let key_id = match key_id {
            Some(id) => id,
            None => {
                return Ok(ValidateApiKeyResponse {
                    is_valid: false,
                    key_id: None,
                    user_id: None,
                    permissions: None,
                    rate_limit_tier: None,
                    last_used_at: None,
                });
            }
        };

        // Get key details
        let stored_key = {
            let mut keys = self
                .keys
                .lock()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;
            match keys.get_mut(&key_id) {
                Some(key) => {
                    // Update last used time
                    key.last_used_at = Some(SystemTime::now());
                    key.clone()
                }
                None => {
                    return Ok(ValidateApiKeyResponse {
                        is_valid: false,
                        key_id: None,
                        user_id: None,
                        permissions: None,
                        rate_limit_tier: None,
                        last_used_at: None,
                    });
                }
            }
        };

        // Check if key is active
        if !stored_key.is_active {
            return Ok(ValidateApiKeyResponse {
                is_valid: false,
                key_id: Some(key_id),
                user_id: Some(stored_key.user_id),
                permissions: None,
                rate_limit_tier: None,
                last_used_at: stored_key.last_used_at.map(format_system_time),
            });
        }

        // Check if key is expired
        if let Some(expires_at) = stored_key.expires_at {
            if SystemTime::now() > expires_at {
                return Ok(ValidateApiKeyResponse {
                    is_valid: false,
                    key_id: Some(key_id),
                    user_id: Some(stored_key.user_id),
                    permissions: None,
                    rate_limit_tier: None,
                    last_used_at: stored_key.last_used_at.map(format_system_time),
                });
            }
        }

        Ok(ValidateApiKeyResponse {
            is_valid: true,
            key_id: Some(key_id),
            user_id: Some(stored_key.user_id),
            permissions: Some(stored_key.permissions),
            rate_limit_tier: Some(stored_key.rate_limit_tier),
            last_used_at: stored_key.last_used_at.map(format_system_time),
        })
    }

    /// Revoke an API key
    pub async fn revoke_api_key(&self, key_id: &str) -> Result<bool, String> {
        let mut keys = self
            .keys
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        match keys.get_mut(key_id) {
            Some(key) => {
                key.is_active = false;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// List API keys for a user
    pub async fn list_user_api_keys(&self, user_id: &str) -> Result<Vec<StoredApiKey>, String> {
        let keys = self
            .keys
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        let user_keys: Vec<StoredApiKey> = keys
            .values()
            .filter(|key| key.user_id == user_id)
            .cloned()
            .collect();

        Ok(user_keys)
    }

    /// Generate a new API key
    fn generate_api_key(&self) -> String {
        format!("dlk_{}", uuid::Uuid::new_v4().simple())
    }

    /// Hash an API key for storage
    fn hash_api_key(&self, api_key: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        api_key.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Format SystemTime as RFC3339 string
fn format_system_time(time: SystemTime) -> String {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let timestamp = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                .unwrap_or_else(|| chrono::Utc::now());
            timestamp.to_rfc3339()
        }
        Err(_) => chrono::Utc::now().to_rfc3339(),
    }
}

/// Create error response
fn error_response(error: &str, message: &str) -> Json<ErrorResponse> {
    Json(ErrorResponse {
        error: error.to_string(),
        message: message.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Health check endpoint
#[instrument]
async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "delong-core".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Create API key endpoint
#[instrument(skip(service))]
async fn create_api_key_handler(
    axum::extract::State(service): axum::extract::State<Arc<ApiKeyService>>,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        user_id = request.user_id,
        key_name = request.name,
        "Creating new API key"
    );

    match service.create_api_key(request).await {
        Ok(response) => {
            info!(key_id = response.id, "API key created successfully");
            Ok(Json(response))
        }
        Err(error) => {
            error!(error = error, "Failed to create API key");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                error_response("CREATION_FAILED", &error),
            ))
        }
    }
}

/// Validate API key endpoint
#[instrument(skip(service))]
async fn validate_api_key_handler(
    axum::extract::State(service): axum::extract::State<Arc<ApiKeyService>>,
    Json(request): Json<ValidateApiKeyRequest>,
) -> Result<Json<ValidateApiKeyResponse>, (StatusCode, Json<ErrorResponse>)> {
    match service.validate_api_key(request).await {
        Ok(response) => Ok(Json(response)),
        Err(error) => {
            error!(error = error, "Failed to validate API key");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                error_response("VALIDATION_FAILED", &error),
            ))
        }
    }
}

/// Revoke API key endpoint
#[instrument(skip(service))]
async fn revoke_api_key_handler(
    axum::extract::State(service): axum::extract::State<Arc<ApiKeyService>>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    info!(key_id = key_id, "Revoking API key");

    match service.revoke_api_key(&key_id).await {
        Ok(true) => {
            info!(key_id = key_id, "API key revoked successfully");
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => {
            warn!(key_id = key_id, "API key not found");
            Err((
                StatusCode::NOT_FOUND,
                error_response("KEY_NOT_FOUND", "API key not found"),
            ))
        }
        Err(error) => {
            error!(error = error, key_id = key_id, "Failed to revoke API key");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                error_response("REVOCATION_FAILED", &error),
            ))
        }
    }
}

/// List user API keys endpoint
#[derive(Debug, Deserialize)]
struct ListKeysQuery {
    user_id: String,
}

#[instrument(skip(service))]
async fn list_api_keys_handler(
    axum::extract::State(service): axum::extract::State<Arc<ApiKeyService>>,
    Query(query): Query<ListKeysQuery>,
) -> Result<Json<Vec<StoredApiKey>>, (StatusCode, Json<ErrorResponse>)> {
    info!(user_id = query.user_id, "Listing API keys for user");

    match service.list_user_api_keys(&query.user_id).await {
        Ok(keys) => {
            info!(
                user_id = query.user_id,
                key_count = keys.len(),
                "API keys retrieved"
            );
            Ok(Json(keys))
        }
        Err(error) => {
            error!(
                error = error,
                user_id = query.user_id,
                "Failed to list API keys"
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                error_response("LIST_FAILED", &error),
            ))
        }
    }
}

/// Create the application router
fn create_router(service: Arc<ApiKeyService>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health_handler))
        // API key management
        .route("/api-keys", post(create_api_key_handler))
        .route("/api-keys/validate", post(validate_api_key_handler))
        .route("/api-keys/:key_id", delete(revoke_api_key_handler))
        .route("/api-keys", get(list_api_keys_handler))
        .with_state(service)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "core=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Delong Core Service");

    // Create API key service
    let api_key_service = Arc::new(ApiKeyService::new());

    // Create router
    let app = create_router(api_key_service);

    // Start server
    let addr: SocketAddr = "0.0.0.0:8081".parse()?;
    info!("Core service listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_creation() {
        let service = ApiKeyService::new();

        let request = CreateApiKeyRequest {
            user_id: "test_user".to_string(),
            name: "Test Key".to_string(),
            description: Some("Test description".to_string()),
            permissions: vec![Permission::DataRead, Permission::DataWrite],
            rate_limit_tier: Some(RateLimitTier::Basic),
            expires_in_days: Some(30),
        };

        let response = service.create_api_key(request).await.unwrap();

        assert_eq!(response.user_id, "test_user");
        assert_eq!(response.name, "Test Key");
        assert!(response.api_key.starts_with("dlk_"));
        assert_eq!(response.permissions.len(), 2);
    }

    #[tokio::test]
    async fn test_api_key_validation() {
        let service = ApiKeyService::new();

        // Create a key first
        let create_request = CreateApiKeyRequest {
            user_id: "test_user".to_string(),
            name: "Test Key".to_string(),
            description: None,
            permissions: vec![Permission::DataRead],
            rate_limit_tier: None,
            expires_in_days: None,
        };

        let created = service.create_api_key(create_request).await.unwrap();

        // Validate the key
        let validate_request = ValidateApiKeyRequest {
            api_key: created.api_key,
        };

        let validation = service.validate_api_key(validate_request).await.unwrap();

        assert!(validation.is_valid);
        assert_eq!(validation.user_id, Some("test_user".to_string()));
        assert!(validation.permissions.is_some());
    }

    #[tokio::test]
    async fn test_api_key_revocation() {
        let service = ApiKeyService::new();

        // Create a key first
        let create_request = CreateApiKeyRequest {
            user_id: "test_user".to_string(),
            name: "Test Key".to_string(),
            description: None,
            permissions: vec![Permission::DataRead],
            rate_limit_tier: None,
            expires_in_days: None,
        };

        let created = service.create_api_key(create_request).await.unwrap();

        // Revoke the key
        let revoked = service.revoke_api_key(&created.id).await.unwrap();
        assert!(revoked);

        // Validate should now fail
        let validate_request = ValidateApiKeyRequest {
            api_key: created.api_key,
        };

        let validation = service.validate_api_key(validate_request).await.unwrap();
        assert!(!validation.is_valid);
    }
}
