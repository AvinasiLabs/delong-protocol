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
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

// Re-export from submodules
pub use ai_audit::*;

// Re-export from api_key module with specific names to avoid conflicts
// Re-export from api_key module
pub use api_key::{ApiKey as ApiKeyModel, ApiKeyStats, RateLimitTier};

// Re-export from auth module
pub use auth::VerificationType;

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

/// Pagination response (alias for compatibility)
/// Pagination metadata for list responses
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginationResponse {
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

/// Search query parameters
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
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
