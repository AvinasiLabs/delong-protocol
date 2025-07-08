//! API key management handlers for authentication operations
//!
//! This module handles all API key related operations including creation,
//! validation, and revocation of API keys for the Delong platform.

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::Json,
};
use core::ResponseCode;
use tracing::{info, instrument, warn};
use utoipa;

use crate::{
    handlers::{ApiResponse, PaginatedResponse},
    services::{generate_request_id, generate_unique_id},
};

use core::{
    ApiKeyInfo, ApiKeyListQuery, CreateApiKeyRequest, CreateApiKeyResponse, Permission,
    RateLimitTier, RevokeApiKeyRequest, RevokeApiKeyResponse, ValidateApiKeyRequest,
    ValidateApiKeyResponse, is_valid_api_key_format,
};

/// Create a new API key
#[utoipa::path(
    post,
    path = "/api/auth/keys",
    tag = "auth",
    summary = "Create API key",
    description = "Create a new API key with specified permissions and rate limiting",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 200, description = "API key created successfully", body = ApiResponse<CreateApiKeyResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
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
        return Ok(Json(ApiResponse {
            code: ResponseCode::BadRequest,
            data: None,
            request_id: Some(request_id),
        }));
    }

    if payload.permissions.is_empty() {
        warn!(
            request_id = request_id,
            "API key creation failed: no permissions specified"
        );
        return Ok(Json(ApiResponse {
            code: ResponseCode::BadRequest,
            data: None,
            request_id: Some(request_id),
        }));
    }

    // Validate expiration days
    let expires_in_days = payload.expires_in_days.unwrap_or(30);
    if expires_in_days > 365 {
        warn!(
            request_id = request_id,
            expires_in_days = expires_in_days,
            "API key creation failed: expiration too long"
        );
        return Ok(Json(ApiResponse {
            code: ResponseCode::BadRequest,
            data: None,
            request_id: Some(request_id),
        }));
    }

    // TODO: Forward to core service for actual API key creation
    // This would include:
    // 1. Generate cryptographically secure API key
    // 2. Hash the API key for storage
    // 3. Store key metadata in database
    // 4. Set up rate limiting configuration
    // 5. Log creation event for audit

    // Generate mock API key
    let api_key = format!("dlk_{}", generate_unique_id());
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
#[utoipa::path(
    post,
    path = "/api/auth/keys/validate",
    tag = "auth",
    summary = "Validate API key",
    description = "Validate an API key and return its permissions and status",
    request_body = ValidateApiKeyRequest,
    responses(
        (status = 200, description = "API key validation completed", body = ApiResponse<ValidateApiKeyResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
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
    let is_valid = is_valid_api_key_format(&payload.api_key);

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
#[utoipa::path(
    delete,
    path = "/api/auth/keys/{key_id}",
    tag = "auth",
    summary = "Revoke API key",
    description = "Revoke an existing API key and make it inactive",
    params(
        ("key_id" = String, Path, description = "API key ID to revoke")
    ),
    request_body = RevokeApiKeyRequest,
    responses(
        (status = 200, description = "API key revoked successfully", body = ApiResponse<RevokeApiKeyResponse>),
        (status = 400, description = "Invalid request parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 404, description = "API key not found", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
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
        return Ok(Json(ApiResponse {
            code: ResponseCode::BadRequest,
            data: None,
            request_id: Some(request_id),
        }));
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
#[utoipa::path(
    get,
    path = "/api/auth/keys",
    tag = "auth",
    summary = "List API keys",
    description = "Retrieve a paginated list of API keys with filtering options",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u32>, Query, description = "Items per page (default: 20, max: 100)"),
        ("is_active" = Option<bool>, Query, description = "Filter by active status"),
        ("rate_limit_tier" = Option<String>, Query, description = "Filter by rate limit tier"),
        ("created_after" = Option<String>, Query, description = "Filter keys created after this date (ISO 8601)"),
        ("created_before" = Option<String>, Query, description = "Filter keys created before this date (ISO 8601)")
    ),
    responses(
        (status = 200, description = "API keys retrieved successfully", body = ApiResponse<PaginatedResponse<ApiKeyInfo>>),
        (status = 400, description = "Invalid query parameters", body = ApiResponse<String>),
        (status = 401, description = "Unauthorized - invalid or missing API key", body = ApiResponse<String>),
        (status = 403, description = "Forbidden - admin access required", body = ApiResponse<String>),
        (status = 500, description = "Internal server error", body = ApiResponse<String>)
    ),
    security(
        ("api_key" = [])
    )
)]
#[instrument(fields(request_id))]
pub async fn list_api_keys_handler(
    Query(query): Query<ApiKeyListQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<ApiKeyInfo>>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        page = query.page,
        limit = query.limit,
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
    let start = ((query.page - 1) * query.limit) as usize;
    let end = std::cmp::min(start + query.limit as usize, filtered_keys.len());
    let page_keys = filtered_keys[start..end].to_vec();

    let response_data = PaginatedResponse::new(page_keys, query.page, query.limit, total);

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
        let key1 = format!("dlk_{}", generate_unique_id());
        let key2 = format!("dlk_{}", generate_unique_id());

        assert!(key1.starts_with("dlk_"));
        assert!(key2.starts_with("dlk_"));
        assert_ne!(key1, key2); // Should be unique
        assert!(key1.len() > 16);
    }

    #[test]
    fn test_validate_api_key_format() {
        assert!(is_valid_api_key_format("dlk_1234567890abcdef"));
        assert!(is_valid_api_key_format("test-api-key-123456"));
        assert!(!is_valid_api_key_format("invalid"));
        assert!(!is_valid_api_key_format("short"));
    }

    #[test]
    fn test_filter_api_keys_by_active() {
        let keys = create_mock_api_keys();
        let query = ApiKeyListQuery {
            page: 1,
            limit: 20,
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
            page: 1,
            limit: 20,
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
