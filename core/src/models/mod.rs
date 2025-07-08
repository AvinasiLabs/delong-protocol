//! Database models and schemas for the DeLong Protocol
//!
//! This module defines all the data structures used throughout the application,
//! including database models, request/response types, and business logic entities.

pub mod ai_audit;
pub mod api_key;
pub mod auth;
pub mod user;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// Re-export from submodules
pub use ai_audit::*;

// Re-export from api_key module with specific names to avoid conflicts
pub use api_key::{
    ApiKey as ApiKeyModel, ApiKeyListResponse, ApiKeyQuery, ApiKeyStats, RateLimitTier,
    UpdateApiKeyRequest,
};

// Re-export from auth module
pub use auth::{
    ApiKey as AuthApiKey, ApiKeyResponse, AssignRoleRequest, AuditAction, AuditLog, AuditLogEntry,
    AuditLogQuery, AuthResponse, ChangePasswordRequest, Claims, ConfirmPasswordResetRequest,
    CreateApiKeyRequest, CreateApiKeyResponse, CreateRoleRequest, GoogleAuthRequest, LoginRequest,
    LogoutRequest, Permission, PermissionCheckRequest, PermissionCheckResponse, PermissionType,
    RefreshTokenRequest, ResetPasswordRequest, Role, RoleWithPermissions, SendCodeRequest,
    SendVerificationCodeRequest, SessionInfo, Setup2FARequest, UpdateRoleRequest,
    UpdateWalletRequest, UserSession, UserSessionsResponse, VerificationCode, VerificationType,
    Verify2FARequest, VerifyCodeRequest,
};

pub use user::*;

/// Pagination parameters
#[derive(Debug, Deserialize, Validate)]
pub struct PaginationParams {
    #[validate(range(min = 1, max = 100))]
    pub page: Option<i32>,
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<i32>,
}

/// Paginated response
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationInfo,
}

/// Pagination information
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub page: i32,
    pub limit: i32,
    pub total: i64,
    pub total_pages: i32,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub uptime: u64,
    pub memory_usage: MemoryUsage,
    pub database: DatabaseHealth,
    pub redis: Option<RedisHealth>,
}

/// Memory usage information
#[derive(Debug, Serialize)]
pub struct MemoryUsage {
    pub used_mb: f64,
    pub total_mb: f64,
}

/// Database health information
#[derive(Debug, Serialize)]
pub struct DatabaseHealth {
    pub connected: bool,
    pub pool_size: u32,
    pub idle_connections: u32,
}

/// Redis health information
#[derive(Debug, Serialize)]
pub struct RedisHealth {
    pub connected: bool,
    pub version: Option<String>,
    pub memory_usage: Option<String>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: Option<String>,
    pub details: Option<serde_json::Value>,
}

/// Success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

/// File upload metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUpload {
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub hash: String,
    pub path: String,
}

/// Dataset type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dataset_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DatasetType {
    Static,
    Dynamic,
    Sample,
}

/// Dataset status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dataset_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DatasetStatus {
    Active,
    Archived,
    Processing,
    Error,
}

/// Algorithm status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "algorithm_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmStatus {
    Active,
    Inactive,
    Deprecated,
}

/// Dataset model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Dataset {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub dataset_type: DatasetType,
    pub status: DatasetStatus,
    pub owner_id: Uuid,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub file_hash: Option<String>,
    pub mime_type: Option<String>,
    pub schema_definition: Option<serde_json::Value>,
    pub sample_data: Option<serde_json::Value>,
    pub row_count: Option<i32>,
    pub column_count: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub accessed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
}

/// Dataset response
#[derive(Debug, Serialize)]
pub struct DatasetResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub dataset_type: DatasetType,
    pub status: DatasetStatus,
    pub owner_id: Uuid,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub row_count: Option<i32>,
    pub column_count: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub accessed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
}

impl From<Dataset> for DatasetResponse {
    fn from(dataset: Dataset) -> Self {
        Self {
            id: dataset.id,
            name: dataset.name,
            description: dataset.description,
            dataset_type: dataset.dataset_type,
            status: dataset.status,
            owner_id: dataset.owner_id,
            file_size: dataset.file_size,
            mime_type: dataset.mime_type,
            row_count: dataset.row_count,
            column_count: dataset.column_count,
            created_at: dataset.created_at,
            updated_at: dataset.updated_at,
            accessed_at: dataset.accessed_at,
            metadata: dataset.metadata,
            tags: dataset.tags,
        }
    }
}

/// Algorithm model
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Algorithm {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub status: AlgorithmStatus,
    pub owner_id: Uuid,
    pub algorithm_type: String,
    pub code_hash: Option<String>,
    pub parameters: serde_json::Value,
    pub requirements: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
}

/// Algorithm response
#[derive(Debug, Serialize)]
pub struct AlgorithmResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub status: AlgorithmStatus,
    pub owner_id: Uuid,
    pub algorithm_type: String,
    pub parameters: serde_json::Value,
    pub requirements: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
}

impl From<Algorithm> for AlgorithmResponse {
    fn from(algorithm: Algorithm) -> Self {
        Self {
            id: algorithm.id,
            name: algorithm.name,
            description: algorithm.description,
            version: algorithm.version,
            status: algorithm.status,
            owner_id: algorithm.owner_id,
            algorithm_type: algorithm.algorithm_type,
            parameters: algorithm.parameters,
            requirements: algorithm.requirements,
            created_at: algorithm.created_at,
            updated_at: algorithm.updated_at,
            metadata: algorithm.metadata,
            tags: algorithm.tags,
        }
    }
}

/// Create dataset request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateDatasetRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
    pub dataset_type: DatasetType,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

/// Update dataset request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDatasetRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<DatasetStatus>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

/// Create algorithm request
#[derive(Debug, Deserialize, Validate)]
pub struct CreateAlgorithmRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub version: String,
    #[validate(length(min = 1, max = 100))]
    pub algorithm_type: String,
    pub parameters: Option<serde_json::Value>,
    pub requirements: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

/// Update algorithm request
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAlgorithmRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    pub description: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub version: Option<String>,
    pub status: Option<AlgorithmStatus>,
    pub parameters: Option<serde_json::Value>,
    pub requirements: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

/// Search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub dataset_type: Option<DatasetType>,
    pub dataset_status: Option<DatasetStatus>,
    pub algorithm_type: Option<String>,
    pub algorithm_status: Option<AlgorithmStatus>,
    pub owner_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// Global statistics
#[derive(Debug, Serialize)]
pub struct GlobalStatistics {
    pub total_users: i64,
    pub active_users: i64,
    pub total_datasets: i64,
    pub active_datasets: i64,
    pub total_algorithms: i64,
    pub active_algorithms: i64,
    pub total_votes: i64,
    pub total_audits: i64,
    pub system_uptime: u64,
    pub last_updated: DateTime<Utc>,
}

/// System configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemConfig {
    pub max_file_size: u64,
    pub allowed_file_types: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub session_timeout_hours: u32,
    pub email_verification_required: bool,
    pub two_factor_auth_enabled: bool,
    pub audit_log_retention_days: u32,
}

/// Notification settings
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub email_notifications: bool,
    pub push_notifications: bool,
    pub webhook_url: Option<String>,
    pub notification_types: Vec<String>,
}

/// User preferences
#[derive(Debug, Serialize, Deserialize)]
pub struct UserPreferences {
    pub theme: String,
    pub language: String,
    pub timezone: String,
    pub notifications: NotificationSettings,
    pub privacy_settings: PrivacySettings,
}

/// Privacy settings
#[derive(Debug, Serialize, Deserialize)]
pub struct PrivacySettings {
    pub profile_visibility: String,
    pub show_email: bool,
    pub show_activity: bool,
    pub allow_data_collection: bool,
}

/// Application metadata
#[derive(Debug, Serialize)]
pub struct AppMetadata {
    pub name: String,
    pub version: String,
    pub build_date: String,
    pub git_commit: String,
    pub environment: String,
}

/// Rate limiting information
#[derive(Debug, Serialize)]
pub struct RateLimitInfo {
    pub requests_remaining: u32,
    pub requests_limit: u32,
    pub reset_time: DateTime<Utc>,
    pub retry_after: Option<u64>,
}
