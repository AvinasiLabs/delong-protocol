//! Authentication-related models for the DeLong Protocol
//!
//! This module contains all data structures related to authentication,
//! authorization, API keys, permissions, and roles.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

/// Verification type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "verification_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VerificationType {
    Email,
    Phone,
    PasswordReset,
    AccountActivation,
}

/// Permission type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "permission_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PermissionType {
    Read,
    Write,
    Delete,
    Admin,
    CreateDataset,
    ViewAudit,
}

/// Audit action enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "audit_action", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Create,
    Update,
    Delete,
    Login,
    Logout,
    DatasetAccess,
}

/// Verification code model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VerificationCode {
    pub id: Uuid,
    pub user_id: Uuid,
    pub code: String,
    pub verification_type: VerificationType,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub is_used: bool,
    pub attempts: i32,
    pub metadata: serde_json::Value,
}

/// User session model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub is_active: bool,
}

/// API Key model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub permissions: serde_json::Value,
    pub rate_limit: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub metadata: serde_json::Value,
}

/// Permission model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Permission {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub permission_type: PermissionType,
    pub resource: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Role model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system_role: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Audit log model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: AuditAction,
    pub resource: Option<String>,
    pub resource_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub session_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// Role with permissions
#[derive(Debug, Serialize)]
pub struct RoleWithPermissions {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system_role: bool,
    pub permissions: Vec<Permission>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API Key response (without sensitive data)
#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub permissions: serde_json::Value,
    pub rate_limit: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

impl From<ApiKey> for ApiKeyResponse {
    fn from(api_key: ApiKey) -> Self {
        Self {
            id: api_key.id,
            name: api_key.name,
            key_prefix: api_key.key_prefix,
            permissions: api_key.permissions,
            rate_limit: api_key.rate_limit,
            expires_at: api_key.expires_at,
            created_at: api_key.created_at,
            last_used_at: api_key.last_used_at,
            is_active: api_key.is_active,
        }
    }
}

/// Create API key request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateApiKeyRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub permissions: Vec<String>,
    pub rate_limit: Option<i32>,
    pub expires_in_days: Option<i32>,
}

/// Create API key response
#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub api_key: ApiKeyResponse,
    pub key: String, // The actual API key (only returned once)
}

/// Permission check request
#[derive(Debug, Deserialize, Validate)]
pub struct PermissionCheckRequest {
    #[validate(length(min = 1))]
    pub permission: String,
    pub resource: Option<String>,
}

/// Permission check response
#[derive(Debug, Serialize)]
pub struct PermissionCheckResponse {
    pub has_permission: bool,
    pub permission: String,
    pub resource: Option<String>,
}

/// Authentication response
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: crate::models::user::UserResponse,
    pub expires_at: DateTime<Utc>,
}

/// JWT claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // User ID
    pub email: String,
    pub username: String,
    pub iat: i64, // Issued at
    pub exp: i64, // Expiration time
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

/// Login request
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6))]
    pub password: String,
}

/// Refresh token request
#[derive(Debug, Deserialize, Validate)]
pub struct RefreshTokenRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

/// Send verification code request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct SendVerificationCodeRequest {
    #[validate(email)]
    pub email: String,
    pub verification_type: VerificationType,
    pub language: Option<String>,
}

/// Verify code request
#[derive(Debug, Deserialize, Validate)]
pub struct VerifyCodeRequest {
    #[validate(length(min = 4, max = 10))]
    pub code: String,
    pub verification_type: VerificationType,
}

/// Reset password request
#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(email)]
    pub email: String,
}

/// Change password request
#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 6))]
    pub current_password: String,
    #[validate(length(min = 6, max = 128))]
    pub new_password: String,
}

/// Confirm password reset request
#[derive(Debug, Deserialize, Validate)]
pub struct ConfirmPasswordResetRequest {
    #[validate(length(min = 4, max = 10))]
    pub code: String,
    #[validate(length(min = 6, max = 128))]
    pub new_password: String,
}

/// Create role request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub description: Option<String>,
    pub permission_ids: Vec<Uuid>,
}

/// Update role request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub permission_ids: Option<Vec<Uuid>>,
}

/// Assign role request
#[derive(Debug, Deserialize, Validate)]
pub struct AssignRoleRequest {
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Session info for user
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_current: bool,
}

/// User sessions response
#[derive(Debug, Serialize)]
pub struct UserSessionsResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: i32,
}

/// Audit log entry for response
#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: AuditAction,
    pub resource: Option<String>,
    pub resource_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<AuditLog> for AuditLogEntry {
    fn from(log: AuditLog) -> Self {
        Self {
            id: log.id,
            user_id: log.user_id,
            action: log.action,
            resource: log.resource,
            resource_id: log.resource_id,
            details: log.details,
            ip_address: log.ip_address,
            user_agent: log.user_agent,
            created_at: log.created_at,
        }
    }
}

/// Audit log query parameters
#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub user_id: Option<Uuid>,
    pub action: Option<AuditAction>,
    pub resource: Option<String>,
    pub resource_id: Option<Uuid>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// Logout request
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub session_id: Option<Uuid>,
    pub all_sessions: Option<bool>,
}

/// Two-factor authentication setup request
#[derive(Debug, Deserialize, Validate)]
pub struct Setup2FARequest {
    pub method: String, // "totp", "sms", "email"
    pub phone: Option<String>,
}

/// Two-factor authentication verify request
#[derive(Debug, Deserialize, Validate)]
pub struct Verify2FARequest {
    #[validate(length(min = 6, max = 6))]
    pub code: String,
}

/// Google authentication request
#[derive(Debug, Deserialize, Validate)]
pub struct GoogleAuthRequest {
    pub code: String,
    pub state: Option<String>,
}

/// Send code request
#[derive(Debug, Deserialize, Validate)]
pub struct SendCodeRequest {
    #[validate(email)]
    pub email: String,
    pub verification_type: Option<VerificationType>,
}

/// Update wallet address request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateWalletRequest {
    #[validate(length(min = 1))]
    pub wallet_address: String,
}
