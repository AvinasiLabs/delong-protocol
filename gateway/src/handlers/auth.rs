//! API key management handlers for authentication operations
//!
//! This module handles all API key related operations including creation,
//! validation, and revocation of API keys for the Delong platform.

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    middleware::auth::{Permission, RateLimitTier},
    utils::{generate_request_id, is_valid_api_key_format},
};

/// API key information returned to users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
    pub is_active: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    /// Only returned during creation
    pub api_key: Option<String>,
}

/// Request body for creating a new API key
#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: Option<RateLimitTier>,
    /// Expiration time in days (optional, default 30 days)
    pub expires_in_days: Option<u32>,
    /// Whether the key should be active immediately
    pub is_active: Option<bool>,
}

/// Response for successful API key creation
#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub api_key_info: ApiKeyInfo,
    pub warning: Option<String>,
}

/// Request body for API key validation
#[derive(Debug, Deserialize)]
pub struct ValidateApiKeyRequest {
    pub api_key: String,
    /// Optional context for validation logging
    pub context: Option<String>,
}

/// Response for API key validation
#[derive(Debug, Serialize)]
pub struct ValidateApiKeyResponse {
    pub is_valid: bool,
    pub user_id: Option<String>,
    pub permissions: Option<Vec<Permission>>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub expires_at: Option<String>,
    pub validation_message: String,
}

/// Request body for revoking an API key
#[derive(Debug, Deserialize)]
pub struct RevokeApiKeyRequest {
    pub reason: Option<String>,
    /// Whether to immediately revoke or schedule for later
    pub immediate: Option<bool>,
}

/// Response for API key revocation
#[derive(Debug, Serialize)]
pub struct RevokeApiKeyResponse {
    pub revoked: bool,
    pub revoked_at: String,
    pub message: String,
}

/// Query parameters for listing API keys
#[derive(Debug, Deserialize)]
pub struct ApiKeyListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub is_active: Option<bool>,
    pub rate_limit_tier: Option<RateLimitTier>,
    pub created_after: Option<String>,
    pub created_before: Option<String>,
}

/// Create a new API key
#[instrument(skip(payload), fields(request_id))]
pub async fn create_api_key_handler(
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiResponse<CreateApiKeyResponse>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        key_name = payload.name,
        permissions = ?payload.permissions,
        rate_limit_tier = ?payload.rate_limit_tier,
        "API key creation request received"
    );

    // Validate request
    if payload.name.trim().is_empty() {
        warn!(
            request_id = request_id,
            "API key creation failed: empty name"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    if payload.permissions.is_empty() {
        warn!(
            request_id = request_id,
            "API key creation failed: no permissions specified"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate expiration days
    let expires_in_days = payload.expires_in_days.unwrap_or(30);
    if expires_in_days > 365 {
        warn!(
            request_id = request_id,
            expires_in_days = expires_in_days,
            "API key creation failed: expiration too long"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Forward to core service for actual API key creation
    // This would include:
    // 1. Generate cryptographically secure API key
    // 2. Hash the API key for storage
    // 3. Store key metadata in database
    // 4. Set up rate limiting configuration
    // 5. Log creation event for audit

    // Generate mock API key
    let api_key = generate_api_key();
    let key_id = format!("key_{}", uuid::Uuid::new_v4().simple());
    let expires_at = chrono::Utc::now() + chrono::Duration::days(expires_in_days as i64);

    let api_key_info = ApiKeyInfo {
        id: key_id.clone(),
        name: payload.name.clone(),
        description: payload.description.clone(),
        permissions: payload.permissions.clone(),
        rate_limit_tier: payload.rate_limit_tier.unwrap_or(RateLimitTier::Basic),
        is_active: payload.is_active.unwrap_or(true),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_used_at: None,
        expires_at: Some(expires_at.to_rfc3339()),
        api_key: Some(api_key.clone()),
    };

    let response_data = CreateApiKeyResponse {
        api_key_info,
        warning: Some("Store this API key securely. It will not be shown again.".to_string()),
    };

    info!(
        request_id = request_id,
        key_id = key_id,
        key_name = payload.name,
        "API key created successfully"
    );

    Ok(Json(ApiResponse::success_with_id(
        response_data,
        request_id,
    )))
}

/// Validate an API key
#[instrument(skip(payload), fields(request_id))]
pub async fn validate_api_key_handler(
    Json(payload): Json<ValidateApiKeyRequest>,
) -> Result<Json<ApiResponse<ValidateApiKeyResponse>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        context = payload.context,
        "API key validation request received"
    );

    // Validate API key format
    if !is_valid_api_key_format(&payload.api_key) {
        warn!(
            request_id = request_id,
            "API key validation failed: invalid format"
        );

        let response_data = ValidateApiKeyResponse {
            is_valid: false,
            user_id: None,
            permissions: None,
            rate_limit_tier: None,
            expires_at: None,
            validation_message: "Invalid API key format".to_string(),
        };

        return Ok(Json(ApiResponse::success_with_id(
            response_data,
            request_id,
        )));
    }

    // TODO: Forward to core service for actual validation
    // This would include:
    // 1. Hash the provided API key
    // 2. Look up key in database
    // 3. Check if key is active and not expired
    // 4. Update last_used_at timestamp
    // 5. Return validation result with user context

    // Mock validation logic
    let is_valid = validate_mock_api_key(&payload.api_key);

    let response_data = if is_valid {
        ValidateApiKeyResponse {
            is_valid: true,
            user_id: Some("user_123".to_string()),
            permissions: Some(vec![
                Permission::DataRead,
                Permission::DataWrite,
                Permission::AlgorithmSubmit,
            ]),
            rate_limit_tier: Some(RateLimitTier::Basic),
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339()),
            validation_message: "API key is valid and active".to_string(),
        }
    } else {
        ValidateApiKeyResponse {
            is_valid: false,
            user_id: None,
            permissions: None,
            rate_limit_tier: None,
            expires_at: None,
            validation_message: "API key not found or inactive".to_string(),
        }
    };

    info!(
        request_id = request_id,
        is_valid = response_data.is_valid,
        user_id = response_data.user_id,
        "API key validation completed"
    );

    Ok(Json(ApiResponse::success_with_id(
        response_data,
        request_id,
    )))
}

/// Revoke an API key
#[instrument(skip( payload), fields(request_id, key_id = %key_id))]
pub async fn revoke_api_key_handler(
    Path(key_id): Path<String>,
    Json(payload): Json<RevokeApiKeyRequest>,
) -> Result<Json<ApiResponse<RevokeApiKeyResponse>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        key_id = key_id,
        reason = payload.reason,
        immediate = payload.immediate.unwrap_or(true),
        "API key revocation request received"
    );

    // Validate key ID format
    if !key_id.starts_with("key_") {
        warn!(
            request_id = request_id,
            key_id = key_id,
            "Invalid API key ID format"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Forward to core service for actual revocation
    // This would include:
    // 1. Verify the key exists and belongs to the requesting user
    // 2. Check if user has permission to revoke this key
    // 3. Mark key as revoked in database
    // 4. Invalidate any cached key information
    // 5. Log revocation event for audit
    // 6. Optionally notify other systems of revocation

    let revoked_at = chrono::Utc::now();
    let response_data = RevokeApiKeyResponse {
        revoked: true,
        revoked_at: revoked_at.to_rfc3339(),
        message: format!("API key {} has been successfully revoked", key_id),
    };

    info!(
        request_id = request_id,
        key_id = key_id,
        revoked_at = revoked_at.to_rfc3339(),
        "API key revoked successfully"
    );

    Ok(Json(ApiResponse::success_with_id(
        response_data,
        request_id,
    )))
}

/// List user's API keys (admin endpoint)
#[instrument(fields(request_id))]
pub async fn list_api_keys_handler(
    Query(query): Query<ApiKeyListQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<ApiKeyInfo>>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        page = query.pagination.page,
        limit = query.pagination.limit,
        "API key list request received"
    );

    // TODO: Forward to core service to get actual API key list
    // This would include:
    // 1. Verify user has permission to list keys
    // 2. Apply filters based on query parameters
    // 3. Paginate results
    // 4. Remove sensitive information (actual keys)
    // 5. Return filtered and paginated list

    let mock_keys = create_mock_api_keys();
    let filtered_keys = filter_api_keys(&mock_keys, &query);
    let total = filtered_keys.len() as u64;

    // Apply pagination
    let start = ((query.pagination.page - 1) * query.pagination.limit) as usize;
    let end = std::cmp::min(start + query.pagination.limit as usize, filtered_keys.len());
    let page_keys = filtered_keys[start..end].to_vec();

    let response_data = PaginatedResponse::new(
        page_keys,
        total,
        query.pagination.page,
        query.pagination.limit,
    );

    info!(
        request_id = request_id,
        total_keys = total,
        returned_count = response_data.items.len(),
        "API key list retrieved successfully"
    );

    Ok(Json(ApiResponse::success_with_id(
        response_data,
        request_id,
    )))
}

/// Generate a new API key
fn generate_api_key() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or(0)
        .hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);

    // In a real implementation, use a cryptographically secure random generator
    format!("dlk_{:x}", hasher.finish())
}

/// Validate mock API key (for testing)
fn validate_mock_api_key(api_key: &str) -> bool {
    // Mock validation - in reality this would hash and check against database
    api_key.len() >= 16 && (api_key.starts_with("dlk_") || api_key.starts_with("test-api-key"))
}

/// Create mock API keys for testing
fn create_mock_api_keys() -> Vec<ApiKeyInfo> {
    vec![
        ApiKeyInfo {
            id: "key_001".to_string(),
            name: "Development Key".to_string(),
            description: Some("Key for development and testing".to_string()),
            permissions: vec![
                Permission::DataRead,
                Permission::DataWrite,
                Permission::AlgorithmSubmit,
            ],
            rate_limit_tier: RateLimitTier::Basic,
            is_active: true,
            created_at: (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339(),
            last_used_at: Some((chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339()),
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(23)).to_rfc3339()),
            api_key: None, // Never return actual key in listings
        },
        ApiKeyInfo {
            id: "key_002".to_string(),
            name: "Production Key".to_string(),
            description: Some("Key for production workloads".to_string()),
            permissions: vec![
                Permission::DataRead,
                Permission::AlgorithmSubmit,
                Permission::AlgorithmRead,
            ],
            rate_limit_tier: RateLimitTier::Premium,
            is_active: true,
            created_at: (chrono::Utc::now() - chrono::Duration::days(15)).to_rfc3339(),
            last_used_at: Some((chrono::Utc::now() - chrono::Duration::minutes(30)).to_rfc3339()),
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(75)).to_rfc3339()),
            api_key: None,
        },
        ApiKeyInfo {
            id: "key_003".to_string(),
            name: "Legacy Key".to_string(),
            description: Some("Deprecated key for legacy systems".to_string()),
            permissions: vec![Permission::DataRead],
            rate_limit_tier: RateLimitTier::Basic,
            is_active: false,
            created_at: (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339(),
            last_used_at: Some((chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339()),
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(5)).to_rfc3339()),
            api_key: None,
        },
    ]
}

/// Filter API keys based on query parameters
fn filter_api_keys(keys: &[ApiKeyInfo], query: &ApiKeyListQuery) -> Vec<ApiKeyInfo> {
    let mut filtered = keys.to_vec();

    // Filter by active status
    if let Some(is_active) = query.is_active {
        filtered.retain(|k| k.is_active == is_active);
    }

    // Filter by rate limit tier
    if let Some(ref tier) = query.rate_limit_tier {
        filtered
            .retain(|k| std::mem::discriminant(&k.rate_limit_tier) == std::mem::discriminant(tier));
    }

    // Filter by creation date range
    if let Some(ref created_after) = query.created_after {
        if let Ok(after_date) = chrono::DateTime::parse_from_rfc3339(created_after) {
            filtered.retain(|k| {
                if let Ok(created_date) = chrono::DateTime::parse_from_rfc3339(&k.created_at) {
                    created_date > after_date
                } else {
                    false
                }
            });
        }
    }

    if let Some(ref created_before) = query.created_before {
        if let Ok(before_date) = chrono::DateTime::parse_from_rfc3339(created_before) {
            filtered.retain(|k| {
                if let Ok(created_date) = chrono::DateTime::parse_from_rfc3339(&k.created_at) {
                    created_date < before_date
                } else {
                    false
                }
            });
        }
    }

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();

        assert!(key1.starts_with("dlk_"));
        assert!(key2.starts_with("dlk_"));
        assert_ne!(key1, key2); // Should be unique
        assert!(key1.len() > 16);
    }

    #[test]
    fn test_validate_mock_api_key() {
        assert!(validate_mock_api_key("dlk_1234567890abcdef"));
        assert!(validate_mock_api_key("test-api-key-123456"));
        assert!(!validate_mock_api_key("invalid"));
        assert!(!validate_mock_api_key("short"));
    }

    #[test]
    fn test_filter_api_keys_by_active() {
        let keys = create_mock_api_keys();
        let query = ApiKeyListQuery {
            pagination: PaginationParams::default(),
            is_active: Some(true),
            rate_limit_tier: None,
            created_after: None,
            created_before: None,
        };

        let filtered = filter_api_keys(&keys, &query);
        assert_eq!(filtered.len(), 2); // Only active keys
        assert!(filtered.iter().all(|k| k.is_active));
    }

    #[test]
    fn test_filter_api_keys_by_tier() {
        let keys = create_mock_api_keys();
        let query = ApiKeyListQuery {
            pagination: PaginationParams::default(),
            is_active: None,
            rate_limit_tier: Some(RateLimitTier::Premium),
            created_after: None,
            created_before: None,
        };

        let filtered = filter_api_keys(&keys, &query);
        assert_eq!(filtered.len(), 1); // Only premium tier
        assert!(matches!(
            filtered[0].rate_limit_tier,
            RateLimitTier::Premium
        ));
    }

    #[test]
    fn test_create_api_key_request_validation() {
        let request = CreateApiKeyRequest {
            name: "Test Key".to_string(),
            description: Some("Test description".to_string()),
            permissions: vec![Permission::DataRead, Permission::DataWrite],
            rate_limit_tier: Some(RateLimitTier::Basic),
            expires_in_days: Some(30),
            is_active: Some(true),
        };

        assert!(!request.name.trim().is_empty());
        assert!(!request.permissions.is_empty());
        assert!(request.expires_in_days.unwrap_or(30) <= 365);
    }
}
