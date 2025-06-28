//! Authentication middleware for API key validation
//!
//! This middleware handles API key authentication for the Delong gateway.
//! It supports multiple authentication methods and provides flexible
//! configuration for different security requirements.

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use serde::{Deserialize, Serialize};

use tracing::{debug, error, info, instrument, warn};

use crate::utils::{extract_api_key, is_valid_api_key_format};

/// Authentication context supporting both JWT and API key authentication
#[derive(Debug, Clone)]
pub enum AuthContext {
    /// JWT authenticated user
    JwtUser {
        user_id: String,
        role: String,
        permissions: Vec<Permission>,
        rate_limit_tier: RateLimitTier,
    },
    /// API Key authenticated client
    ApiKeyClient {
        user_id: String,
        api_key_id: String,
        permissions: Vec<Permission>,
        rate_limit_tier: RateLimitTier,
    },
}

/// User permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Read access to own data
    DataRead,
    /// Write access to own data
    DataWrite,
    /// Delete access to own data
    DataDelete,
    /// Submit algorithms for execution
    AlgorithmSubmit,
    /// View algorithm results
    AlgorithmRead,
    /// Administrative access to API keys
    ApiKeyManage,
    /// Administrative access to all resources
    AdminAccess,
}

/// Rate limiting tiers based on API key type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitTier {
    /// Basic tier with standard limits
    Basic,
    /// Premium tier with higher limits
    Premium,
    /// Enterprise tier with very high limits
    Enterprise,
    /// Internal services with no limits
    Internal,
}

/// API key information stored in the system
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ApiKeyInfo {
    pub id: String,
    pub user_id: String,
    pub key_hash: String,
    pub permissions: Vec<Permission>,
    pub rate_limit_tier: RateLimitTier,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Paths that don't require authentication
const PUBLIC_PATHS: &[&str] = &[
    "/health",
    "/health/live",
    "/health/ready",
    // Sample data access is public (no authentication required)
    "/api/sample/",
    // Add other public paths as needed
];

/// Main authentication middleware
#[instrument(skip(request, next), fields(path = %request.uri().path()))]
pub async fn auth_middleware(mut request: Request, next: Next) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();
    let method = request.method().as_str().to_string();

    // Skip authentication for public paths
    if is_public_path(&path) {
        debug!(path = %path, "Skipping authentication for public path");
        return Ok(next.run(request).await);
    }

    // Try JWT authentication first, then fallback to API key
    let auth_context = if let Some(jwt_token) = extract_jwt_token(request.headers()) {
        // JWT authentication
        match authenticate_jwt_token(&jwt_token).await {
            Ok(context) => context,
            Err(auth_error) => {
                warn!(
                    path = %path,
                    method = %method,
                    error = %auth_error,
                    "JWT authentication failed"
                );
                return Err(auth_error.status_code());
            }
        }
    } else if let Some(api_key) = extract_api_key(request.headers()) {
        // API key authentication
        if !is_valid_api_key_format(&api_key) {
            warn!(
                path = %path,
                method = %method,
                "Invalid API key format provided"
            );
            return Err(StatusCode::UNAUTHORIZED);
        }

        match authenticate_api_key(&api_key).await {
            Ok(context) => context,
            Err(auth_error) => {
                warn!(
                    path = %path,
                    method = %method,
                    error = %auth_error,
                    "API key authentication failed"
                );
                return Err(auth_error.status_code());
            }
        }
    } else {
        warn!(
            path = path,
            method = method,
            "No authentication credentials provided"
        );
        return Err(StatusCode::UNAUTHORIZED);
    };

    // Check permissions for the requested path/method
    if !has_permission(&auth_context, &path, &method) {
        let user_id = match &auth_context {
            AuthContext::JwtUser { user_id, .. } => user_id,
            AuthContext::ApiKeyClient { user_id, .. } => user_id,
        };
        warn!(
            path = %path,
            method = %method,
            user_id = user_id,
            "Insufficient permissions for requested resource"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Add authentication context to request extensions
    request.extensions_mut().insert(auth_context.clone());

    match &auth_context {
        AuthContext::JwtUser { user_id, role, .. } => {
            info!(
                path = %path,
                method = %method,
                user_id = user_id,
                auth_type = "jwt",
                role = role,
                "JWT authentication successful"
            );
        }
        AuthContext::ApiKeyClient {
            user_id,
            api_key_id,
            ..
        } => {
            info!(
                path = %path,
                method = %method,
                user_id = user_id,
                api_key_id = api_key_id,
                auth_type = "api_key",
                "API key authentication successful"
            );
        }
    }

    Ok(next.run(request).await)
}

/// Check if a path is public (doesn't require authentication)
fn is_public_path(path: &str) -> bool {
    PUBLIC_PATHS.iter().any(|&public_path| {
        path == public_path
            || path.starts_with(&format!("{}/", public_path))
            || (public_path.ends_with('/') && path.starts_with(public_path))
    })
}

/// Extract JWT token from Authorization header
fn extract_jwt_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| {
            if header.starts_with("Bearer ") {
                Some(header[7..].to_string())
            } else {
                None
            }
        })
}

/// Authenticate JWT token and return authentication context
async fn authenticate_jwt_token(jwt_token: &str) -> Result<AuthContext, AuthError> {
    // TODO: In a real implementation, this would:
    // 1. Validate JWT signature with secret/public key
    // 2. Check token expiration
    // 3. Extract user claims from token
    // 4. Query user permissions from database
    // 5. Return authentication context

    // Mock implementation for now
    if jwt_token == "mock-jwt-token-user" {
        Ok(AuthContext::JwtUser {
            user_id: "user_123".to_string(),
            role: "user".to_string(),
            permissions: vec![
                Permission::DataRead,
                Permission::DataWrite,
                Permission::AlgorithmSubmit,
                Permission::AlgorithmRead,
            ],
            rate_limit_tier: RateLimitTier::Basic,
        })
    } else if jwt_token == "mock-jwt-token-admin" {
        Ok(AuthContext::JwtUser {
            user_id: "admin_456".to_string(),
            role: "admin".to_string(),
            permissions: vec![Permission::AdminAccess],
            rate_limit_tier: RateLimitTier::Enterprise,
        })
    } else {
        Err(AuthError::InvalidToken)
    }
}

/// Authenticate an API key and return authentication context
async fn authenticate_api_key(api_key: &str) -> Result<AuthContext, AuthError> {
    // TODO: In a real implementation, this would:
    // 1. Hash the API key
    // 2. Query the database/core service to find the key
    // 3. Validate it's not expired or revoked
    // 4. Update last_used_at timestamp
    // 5. Return the associated user context

    // Mock implementation for now
    let mock_api_keys = get_mock_api_keys();

    if let Some(key_info) = mock_api_keys.iter().find(|k| {
        // In reality, we'd compare hashes, not plain text
        k.key_hash == api_key && k.is_active
    }) {
        // Check if key is expired
        if let Some(expires_at) = key_info.expires_at {
            if chrono::Utc::now() > expires_at {
                return Err(AuthError::KeyExpired);
            }
        }

        Ok(AuthContext::ApiKeyClient {
            user_id: key_info.user_id.clone(),
            api_key_id: key_info.id.clone(),
            permissions: key_info.permissions.clone(),
            rate_limit_tier: key_info.rate_limit_tier.clone(),
        })
    } else {
        Err(AuthError::InvalidKey)
    }
}

/// Check if the user has permission to access the requested resource
fn has_permission(auth_context: &AuthContext, path: &str, method: &str) -> bool {
    let (permissions, is_admin) = match auth_context {
        AuthContext::JwtUser {
            permissions, role, ..
        } => (
            permissions,
            role == "admin" || permissions.contains(&Permission::AdminAccess),
        ),
        AuthContext::ApiKeyClient { permissions, .. } => {
            (permissions, permissions.contains(&Permission::AdminAccess))
        }
    };

    // Admin users have access to everything
    if is_admin {
        return true;
    }

    // Check specific path permissions
    match (path, method) {
        // Static dataset endpoints
        (path, "GET") if path.starts_with("/api/static-datasets") => {
            permissions.contains(&Permission::DataRead)
        }
        (path, "POST") if path.starts_with("/api/static-datasets") => {
            permissions.contains(&Permission::DataWrite)
        }

        // Dynamic dataset endpoints (admin only for creation)
        (path, "POST") if path.starts_with("/api/datasets") => {
            is_admin || permissions.contains(&Permission::AdminAccess)
        }
        (path, "GET") if path.starts_with("/api/datasets") => {
            permissions.contains(&Permission::DataRead)
        }

        // API key management endpoints (admin only)
        (path, _) if path.starts_with("/api/auth") => {
            is_admin || permissions.contains(&Permission::ApiKeyManage)
        }

        // Default: deny access
        _ => false,
    }
}

/// Get mock API keys for testing
fn get_mock_api_keys() -> Vec<ApiKeyInfo> {
    vec![
        ApiKeyInfo {
            id: "key_001".to_string(),
            user_id: "user_123".to_string(),
            key_hash: "test-api-key-123456".to_string(), // In reality, this would be hashed
            permissions: vec![
                Permission::DataRead,
                Permission::DataWrite,
                Permission::AlgorithmSubmit,
                Permission::AlgorithmRead,
            ],
            rate_limit_tier: RateLimitTier::Basic,
            is_active: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
            expires_at: Some(chrono::Utc::now() + chrono::Duration::days(30)),
        },
        ApiKeyInfo {
            id: "key_002".to_string(),
            user_id: "admin_456".to_string(),
            key_hash: "admin-api-key-789012".to_string(),
            permissions: vec![Permission::AdminAccess],
            rate_limit_tier: RateLimitTier::Enterprise,
            is_active: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
            expires_at: None, // No expiration for admin keys
        },
    ]
}

/// Authentication errors
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AuthError {
    #[error("Invalid API key")]
    InvalidKey,
    #[error("API key has expired")]
    KeyExpired,
    #[error("API key has been revoked")]
    KeyRevoked,
    #[error("Invalid JWT token")]
    InvalidToken,
    #[error("JWT token has expired")]
    TokenExpired,
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Service unavailable")]
    ServiceUnavailable,
}

impl AuthError {
    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            AuthError::InvalidKey
            | AuthError::KeyExpired
            | AuthError::KeyRevoked
            | AuthError::InvalidToken
            | AuthError::TokenExpired => StatusCode::UNAUTHORIZED,
            AuthError::DatabaseError(_) | AuthError::ServiceUnavailable => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_public_path() {
        assert!(is_public_path("/health"));
        assert!(is_public_path("/health/live"));
        assert!(is_public_path("/api/sample/QmTest123"));
        assert!(!is_public_path("/api/static-datasets"));
        assert!(!is_public_path("/api/datasets"));
    }

    #[test]
    fn test_has_permission() {
        let auth_context = AuthContext::ApiKeyClient {
            user_id: "user_123".to_string(),
            api_key_id: "key_001".to_string(),
            permissions: vec![Permission::DataRead, Permission::DataWrite],
            rate_limit_tier: RateLimitTier::Basic,
        };

        assert!(has_permission(&auth_context, "/api/static-datasets", "GET"));
        assert!(has_permission(
            &auth_context,
            "/api/static-datasets",
            "POST"
        ));
        assert!(!has_permission(&auth_context, "/api/datasets", "POST")); // Admin only
        assert!(!has_permission(&auth_context, "/api/auth/keys", "POST"));
    }

    #[test]
    fn test_admin_permission() {
        let admin_context = AuthContext::JwtUser {
            user_id: "admin_456".to_string(),
            role: "admin".to_string(),
            permissions: vec![Permission::AdminAccess],
            rate_limit_tier: RateLimitTier::Enterprise,
        };

        // Admin should have access to everything
        assert!(has_permission(
            &admin_context,
            "/api/static-datasets",
            "POST"
        ));
        assert!(has_permission(&admin_context, "/api/datasets", "POST"));
        assert!(has_permission(&admin_context, "/api/auth/keys", "POST"));
    }

    #[tokio::test]
    async fn test_authenticate_valid_key() {
        let result = authenticate_api_key("test-api-key-123456").await;
        assert!(result.is_ok());

        let auth_context = result.unwrap();
        match auth_context {
            AuthContext::ApiKeyClient {
                user_id,
                permissions,
                ..
            } => {
                assert_eq!(user_id, "user_123");
                assert!(permissions.contains(&Permission::DataRead));
            }
            _ => panic!("Expected ApiKeyClient context"),
        }
    }

    #[tokio::test]
    async fn test_authenticate_invalid_key() {
        let result = authenticate_api_key("invalid-key").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidKey));
    }

    #[tokio::test]
    async fn test_authenticate_valid_jwt() {
        let result = authenticate_jwt_token("mock-jwt-token-user").await;
        assert!(result.is_ok());

        let auth_context = result.unwrap();
        match auth_context {
            AuthContext::JwtUser { user_id, role, .. } => {
                assert_eq!(user_id, "user_123");
                assert_eq!(role, "user");
            }
            _ => panic!("Expected JwtUser context"),
        }
    }

    #[tokio::test]
    async fn test_authenticate_invalid_jwt() {
        let result = authenticate_jwt_token("invalid-jwt-token").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidToken));
    }

    #[test]
    fn test_extract_jwt_token() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "authorization",
            "Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9"
                .parse()
                .unwrap(),
        );

        let token = extract_jwt_token(&headers);
        assert!(token.is_some());
        assert_eq!(token.unwrap(), "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9");

        // Test without Bearer prefix
        let mut headers2 = axum::http::HeaderMap::new();
        headers2.insert("authorization", "Basic dXNlcjpwYXNz".parse().unwrap());
        let token2 = extract_jwt_token(&headers2);
        assert!(token2.is_none());
    }
}
