//! Authentication and authorization data models
//!
//! This module contains all data structures related to authentication, authorization,
//! API key management, and user session handling.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Permission levels for API access control
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read access to dataset information
    #[serde(rename = "data_read")]
    DataRead,
    /// Write access to dataset operations
    #[serde(rename = "data_write")]
    DataWrite,
    /// Submit algorithms for execution
    #[serde(rename = "algorithm_submit")]
    AlgorithmSubmit,
    /// Read algorithm execution results
    #[serde(rename = "algorithm_read")]
    AlgorithmRead,
    /// Vote on algorithm approval (committee members only)
    #[serde(rename = "committee_vote")]
    CommitteeVote,
    /// Manage committee members (admin only)
    #[serde(rename = "committee_manage")]
    CommitteeManage,
    /// Administrative access to all operations
    #[serde(rename = "admin")]
    Admin,
}

/// Rate limiting tiers for API access
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RateLimitTier {
    /// Basic tier: 100 requests per minute
    #[serde(rename = "basic")]
    Basic,
    /// Standard tier: 500 requests per minute
    #[serde(rename = "standard")]
    Standard,
    /// Premium tier: 2000 requests per minute
    #[serde(rename = "premium")]
    Premium,
    /// Enterprise tier: 10000 requests per minute
    #[serde(rename = "enterprise")]
    Enterprise,
}

/// API key information returned to users
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKeyInfo {
    /// Unique identifier for the API key
    pub id: String,
    /// Human-readable name for the API key
    pub name: String,
    /// Optional description of the API key's purpose
    pub description: Option<String>,
    /// Permissions granted to this API key
    pub permissions: Vec<Permission>,
    /// Rate limiting tier for this API key
    pub rate_limit_tier: RateLimitTier,
    /// Whether the API key is currently active
    pub is_active: bool,
    /// Creation timestamp in RFC3339 format
    pub created_at: String,
    /// Last usage timestamp in RFC3339 format (if ever used)
    pub last_used_at: Option<String>,
    /// Expiration timestamp in RFC3339 format (if set)
    pub expires_at: Option<String>,
    /// The actual API key value (only returned during creation)
    pub api_key: Option<String>,
}

/// Request body for creating a new API key
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateApiKeyRequest {
    /// Human-readable name for the API key
    pub name: String,
    /// Optional description of the API key's purpose
    pub description: Option<String>,
    /// Permissions to grant to this API key
    pub permissions: Vec<Permission>,
    /// Rate limiting tier (optional, defaults to Basic)
    pub rate_limit_tier: Option<RateLimitTier>,
    /// Expiration time in days (optional, default 30 days)
    pub expires_in_days: Option<u32>,
    /// Whether the key should be active immediately (optional, default true)
    pub is_active: Option<bool>,
}

/// Response for successful API key creation
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateApiKeyResponse {
    /// The created API key information
    pub api_key_info: ApiKeyInfo,
    /// Security warning message
    pub warning: Option<String>,
}

/// Request body for API key validation
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidateApiKeyRequest {
    /// The API key to validate
    pub api_key: String,
    /// Optional context for validation logging
    pub context: Option<String>,
}

/// Response for API key validation
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidateApiKeyResponse {
    /// Whether the API key is valid
    pub is_valid: bool,
    /// User ID associated with the API key (if valid)
    pub user_id: Option<String>,
    /// Permissions granted by this API key (if valid)
    pub permissions: Option<Vec<Permission>>,
    /// Rate limiting tier for this API key (if valid)
    pub rate_limit_tier: Option<RateLimitTier>,
    /// Expiration timestamp (if valid and set)
    pub expires_at: Option<String>,
    /// Human-readable validation message
    pub validation_message: String,
}

/// Request body for revoking an API key
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RevokeApiKeyRequest {
    /// Optional reason for revocation
    pub reason: Option<String>,
    /// Whether to immediately revoke or schedule for later (optional, default true)
    pub immediate: Option<bool>,
}

/// Response for API key revocation
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RevokeApiKeyResponse {
    /// Whether the revocation was successful
    pub revoked: bool,
    /// Timestamp when the key was revoked
    pub revoked_at: String,
    /// Human-readable message about the revocation
    pub message: String,
}

/// Query parameters for listing API keys
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ApiKeyListQuery {
    /// Page number for pagination (1-based)
    pub page: u32,
    /// Number of items per page
    pub limit: u32,
    /// Filter by active status (optional)
    pub is_active: Option<bool>,
    /// Filter by rate limit tier (optional)
    pub rate_limit_tier: Option<RateLimitTier>,
    /// Filter by creation date (ISO 8601, optional)
    pub created_after: Option<String>,
    /// Filter by creation date (ISO 8601, optional)
    pub created_before: Option<String>,
}

/// JWT token claims structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JwtClaims {
    /// Subject (user ID)
    pub sub: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Not before (Unix timestamp)
    pub nbf: i64,
    /// JWT ID
    pub jti: String,
    /// User permissions
    pub permissions: Vec<Permission>,
    /// User rate limit tier
    pub rate_limit_tier: RateLimitTier,
}

/// User session information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserSession {
    /// Unique session ID
    pub session_id: String,
    /// User ID
    pub user_id: String,
    /// Session creation timestamp
    pub created_at: String,
    /// Last activity timestamp
    pub last_activity: String,
    /// Session expiration timestamp
    pub expires_at: String,
    /// Whether the session is active
    pub is_active: bool,
    /// User agent string
    pub user_agent: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
}

/// Authentication context for request processing
#[derive(Debug, Clone, PartialEq)]
pub struct AuthContext {
    /// User ID
    pub user_id: String,
    /// Authentication method used
    pub auth_method: AuthMethod,
    /// User permissions
    pub permissions: HashSet<Permission>,
    /// Rate limit tier
    pub rate_limit_tier: RateLimitTier,
    /// Request timestamp
    pub authenticated_at: String,
}

/// Authentication method enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// JWT token authentication
    JwtToken,
    /// API key authentication
    ApiKey,
    /// Session-based authentication
    Session,
}

impl Permission {
    /// Get all available permissions
    pub fn all() -> Vec<Permission> {
        vec![
            Permission::DataRead,
            Permission::DataWrite,
            Permission::AlgorithmSubmit,
            Permission::AlgorithmRead,
            Permission::CommitteeVote,
            Permission::CommitteeManage,
            Permission::Admin,
        ]
    }

    /// Check if this permission includes another permission
    pub fn includes(&self, other: &Permission) -> bool {
        match self {
            Permission::Admin => true, // Admin includes all permissions
            Permission::DataWrite => matches!(other, Permission::DataRead | Permission::DataWrite),
            Permission::CommitteeManage => matches!(
                other,
                Permission::CommitteeVote | Permission::CommitteeManage
            ),
            _ => self == other,
        }
    }

    /// Get the permission level (higher number = more privileged)
    pub fn level(&self) -> u8 {
        match self {
            Permission::DataRead => 1,
            Permission::AlgorithmRead => 2,
            Permission::DataWrite => 3,
            Permission::AlgorithmSubmit => 4,
            Permission::CommitteeVote => 5,
            Permission::CommitteeManage => 6,
            Permission::Admin => 10,
        }
    }
}

impl RateLimitTier {
    /// Get the requests per minute limit for this tier
    pub fn requests_per_minute(&self) -> u32 {
        match self {
            RateLimitTier::Basic => 100,
            RateLimitTier::Standard => 500,
            RateLimitTier::Premium => 2000,
            RateLimitTier::Enterprise => 10000,
        }
    }

    /// Get the requests per hour limit for this tier
    pub fn requests_per_hour(&self) -> u32 {
        self.requests_per_minute() * 60
    }

    /// Get the requests per day limit for this tier
    pub fn requests_per_day(&self) -> u32 {
        self.requests_per_hour() * 24
    }
}

impl ApiKeyInfo {
    /// Create a new API key info instance
    pub fn new(
        id: String,
        name: String,
        permissions: Vec<Permission>,
        rate_limit_tier: RateLimitTier,
        created_at: String,
    ) -> Self {
        Self {
            id,
            name,
            description: None,
            permissions,
            rate_limit_tier,
            is_active: true,
            created_at,
            last_used_at: None,
            expires_at: None,
            api_key: None,
        }
    }

    /// Check if the API key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = &self.expires_at {
            if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                return chrono::Utc::now() > exp_time;
            }
        }
        false
    }

    /// Check if the API key has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.iter().any(|p| p.includes(permission))
    }

    /// Get days until expiration (if set)
    pub fn days_until_expiration(&self) -> Option<i64> {
        if let Some(expires_at) = &self.expires_at {
            if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                let now = chrono::Utc::now();
                let duration = exp_time.signed_duration_since(now);
                return Some(duration.num_days());
            }
        }
        None
    }
}

impl CreateApiKeyRequest {
    /// Create a new API key creation request
    pub fn new(name: String, permissions: Vec<Permission>) -> Self {
        Self {
            name,
            description: None,
            permissions,
            rate_limit_tier: None,
            expires_in_days: None,
            is_active: None,
        }
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("API key name cannot be empty".to_string());
        }

        if self.name.len() > 100 {
            return Err("API key name cannot exceed 100 characters".to_string());
        }

        if self.permissions.is_empty() {
            return Err("At least one permission must be specified".to_string());
        }

        if let Some(desc) = &self.description {
            if desc.len() > 500 {
                return Err("Description cannot exceed 500 characters".to_string());
            }
        }

        if let Some(days) = self.expires_in_days {
            if days == 0 || days > 365 {
                return Err("Expiration must be between 1 and 365 days".to_string());
            }
        }

        Ok(())
    }
}

impl ValidateApiKeyRequest {
    /// Create a new API key validation request
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            context: None,
        }
    }

    /// Create a new API key validation request with context
    pub fn with_context(api_key: String, context: String) -> Self {
        Self {
            api_key,
            context: Some(context),
        }
    }
}

impl RevokeApiKeyRequest {
    /// Create a new API key revocation request
    pub fn new() -> Self {
        Self {
            reason: None,
            immediate: None,
        }
    }

    /// Create a new API key revocation request with reason
    pub fn with_reason(reason: String) -> Self {
        Self {
            reason: Some(reason),
            immediate: None,
        }
    }
}

impl Default for RevokeApiKeyRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiKeyListQuery {
    /// Create a new API key list query with defaults
    pub fn new() -> Self {
        Self {
            page: 1,
            limit: 20,
            is_active: None,
            rate_limit_tier: None,
            created_after: None,
            created_before: None,
        }
    }
}

impl Default for ApiKeyListQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthContext {
    /// Create a new authentication context
    pub fn new(
        user_id: String,
        auth_method: AuthMethod,
        permissions: Vec<Permission>,
        rate_limit_tier: RateLimitTier,
    ) -> Self {
        Self {
            user_id,
            auth_method,
            permissions: permissions.into_iter().collect(),
            rate_limit_tier,
            authenticated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Check if the user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.iter().any(|p| p.includes(permission))
    }

    /// Check if the user has admin privileges
    pub fn is_admin(&self) -> bool {
        self.has_permission(&Permission::Admin)
    }
}

/// Validate API key format
pub fn is_valid_api_key_format(api_key: &str) -> bool {
    // API keys should be at least 16 characters and start with a prefix
    api_key.len() >= 16 && (api_key.starts_with("dlk_") || api_key.starts_with("test-api-key"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_includes() {
        assert!(Permission::Admin.includes(&Permission::DataRead));
        assert!(Permission::Admin.includes(&Permission::CommitteeManage));
        assert!(Permission::DataWrite.includes(&Permission::DataRead));
        assert!(!Permission::DataRead.includes(&Permission::DataWrite));
        assert!(Permission::CommitteeManage.includes(&Permission::CommitteeVote));
        assert!(!Permission::CommitteeVote.includes(&Permission::CommitteeManage));
    }

    #[test]
    fn test_permission_level() {
        assert_eq!(Permission::Admin.level(), 10);
        assert_eq!(Permission::DataRead.level(), 1);
        assert!(Permission::Admin.level() > Permission::DataWrite.level());
    }

    #[test]
    fn test_rate_limit_tier_limits() {
        assert_eq!(RateLimitTier::Basic.requests_per_minute(), 100);
        assert_eq!(RateLimitTier::Premium.requests_per_hour(), 2000 * 60);
        assert_eq!(
            RateLimitTier::Enterprise.requests_per_day(),
            10000 * 60 * 24
        );
    }

    #[test]
    fn test_api_key_info_creation() {
        let api_key = ApiKeyInfo::new(
            "key_123".to_string(),
            "Test Key".to_string(),
            vec![Permission::DataRead, Permission::DataWrite],
            RateLimitTier::Basic,
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(api_key.id, "key_123");
        assert_eq!(api_key.name, "Test Key");
        assert!(api_key.is_active);
        assert!(!api_key.is_expired());
        assert!(api_key.has_permission(&Permission::DataRead));
        assert!(api_key.has_permission(&Permission::DataWrite));
        assert!(!api_key.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_api_key_info_permissions() {
        let admin_key = ApiKeyInfo::new(
            "admin_key".to_string(),
            "Admin Key".to_string(),
            vec![Permission::Admin],
            RateLimitTier::Enterprise,
            "2023-01-01T00:00:00Z".to_string(),
        );

        // Admin should have all permissions
        assert!(admin_key.has_permission(&Permission::DataRead));
        assert!(admin_key.has_permission(&Permission::DataWrite));
        assert!(admin_key.has_permission(&Permission::CommitteeVote));
        assert!(admin_key.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_create_api_key_request_validation() {
        // Valid request
        let valid_request =
            CreateApiKeyRequest::new("Test Key".to_string(), vec![Permission::DataRead]);
        assert!(valid_request.validate().is_ok());

        // Empty name
        let empty_name = CreateApiKeyRequest::new("".to_string(), vec![Permission::DataRead]);
        assert!(empty_name.validate().is_err());

        // No permissions
        let no_permissions = CreateApiKeyRequest::new("Test Key".to_string(), vec![]);
        assert!(no_permissions.validate().is_err());

        // Name too long
        let long_name = CreateApiKeyRequest::new("a".repeat(101), vec![Permission::DataRead]);
        assert!(long_name.validate().is_err());
    }

    #[test]
    fn test_validate_api_key_request() {
        let request = ValidateApiKeyRequest::new("dlk_test123456".to_string());
        assert_eq!(request.api_key, "dlk_test123456");
        assert!(request.context.is_none());

        let request_with_context = ValidateApiKeyRequest::with_context(
            "dlk_test123456".to_string(),
            "login_attempt".to_string(),
        );
        assert_eq!(
            request_with_context.context,
            Some("login_attempt".to_string())
        );
    }

    #[test]
    fn test_revoke_api_key_request() {
        let request = RevokeApiKeyRequest::new();
        assert!(request.reason.is_none());
        assert!(request.immediate.is_none());

        let request_with_reason = RevokeApiKeyRequest::with_reason("Security breach".to_string());
        assert_eq!(
            request_with_reason.reason,
            Some("Security breach".to_string())
        );
    }

    #[test]
    fn test_auth_context() {
        let context = AuthContext::new(
            "user_123".to_string(),
            AuthMethod::ApiKey,
            vec![Permission::DataRead, Permission::DataWrite],
            RateLimitTier::Premium,
        );

        assert_eq!(context.user_id, "user_123");
        assert!(context.has_permission(&Permission::DataRead));
        assert!(context.has_permission(&Permission::DataWrite));
        assert!(!context.has_permission(&Permission::Admin));
        assert!(!context.is_admin());
    }

    #[test]
    fn test_auth_context_admin() {
        let admin_context = AuthContext::new(
            "admin_user".to_string(),
            AuthMethod::JwtToken,
            vec![Permission::Admin],
            RateLimitTier::Enterprise,
        );

        assert!(admin_context.is_admin());
        assert!(admin_context.has_permission(&Permission::DataRead));
        assert!(admin_context.has_permission(&Permission::CommitteeManage));
    }

    #[test]
    fn test_is_valid_api_key_format() {
        assert!(is_valid_api_key_format("dlk_1234567890abcdef"));
        assert!(is_valid_api_key_format("test-api-key-123456"));
        assert!(!is_valid_api_key_format("invalid"));
        assert!(!is_valid_api_key_format("short"));
        assert!(!is_valid_api_key_format("wrong_prefix_123456"));
    }

    #[test]
    fn test_serialization_deserialization() {
        let api_key = ApiKeyInfo::new(
            "key_123".to_string(),
            "Test Key".to_string(),
            vec![Permission::DataRead],
            RateLimitTier::Basic,
            "2023-01-01T00:00:00Z".to_string(),
        );

        let json = serde_json::to_string(&api_key).unwrap();
        let deserialized: ApiKeyInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(api_key, deserialized);

        let request = CreateApiKeyRequest::new(
            "Test Key".to_string(),
            vec![Permission::DataRead, Permission::DataWrite],
        );

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CreateApiKeyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }
}
