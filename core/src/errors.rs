//! Error handling for the DeLong Protocol
//!
//! This module provides comprehensive error handling with standardized error types,
//! HTTP status code mapping, and user-friendly error messages.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{error, warn};

/// Main application error type
#[derive(Error, Debug)]
pub enum AppError {
    // Database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    // Authentication and authorization errors
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Authorization failed: {message}")]
    AuthorizationFailed { message: String },

    #[error("Invalid JWT token: {message}")]
    InvalidToken { message: String },

    #[error("Token expired")]
    TokenExpired,

    #[error("API key invalid or expired")]
    InvalidApiKey,

    #[error("Account locked due to too many failed login attempts")]
    AccountLocked,

    #[error("Email verification required")]
    EmailVerificationRequired,

    #[error("Two-factor authentication required")]
    TwoFactorRequired,

    // Validation errors
    #[error("Validation failed")]
    ValidationFailed { errors: ValidationErrors },

    #[error("Invalid request data: {message}")]
    InvalidRequest { message: String },

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid field value: {field}")]
    InvalidField { field: String },

    #[error("Duplicate entry: {message}")]
    DuplicateEntry { message: String },

    // Resource errors
    #[error("Resource not found: {resource_type}")]
    NotFound { resource_type: String },

    #[error("Resource already exists: {resource_type}")]
    AlreadyExists { resource_type: String },

    #[error("Resource access denied: {resource_type}")]
    AccessDenied { resource_type: String },

    #[error("Resource conflict: {message}")]
    Conflict { message: String },

    // External service errors
    #[error("External API error: {service} - {message}")]
    ExternalApiError { service: String, message: String },

    #[error("External service error: {service} - {message}")]
    ExternalServiceError { service: String, message: String },

    #[error("OAuth error: {message}")]
    OAuthError { message: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("AI audit service error: {message}")]
    AiAuditError { message: String },

    #[error("Blockchain RPC error: {message}")]
    BlockchainError { message: String },

    #[error("IPFS error: {message}")]
    IpfsError { message: String },

    // File and upload errors
    #[error("File upload error: {message}")]
    FileUploadError { message: String },

    #[error("File too large: {size} bytes, max allowed: {max_size} bytes")]
    FileTooLarge { size: u64, max_size: u64 },

    #[error("Unsupported file type: {file_type}")]
    UnsupportedFileType { file_type: String },

    #[error("File not found: {filename}")]
    FileNotFound { filename: String },

    #[error("File processing error: {message}")]
    FileProcessingError { message: String },

    // Rate limiting and security errors
    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u32, window: String },

    #[error("Request timeout")]
    RequestTimeout,

    #[error("Payload too large")]
    PayloadTooLarge,

    #[error("Invalid content type: {content_type}")]
    InvalidContentType { content_type: String },

    // Email and communication errors
    #[error("Email sending failed: {message}")]
    EmailError { message: String },

    #[error("SMS sending failed: {message}")]
    SmsError { message: String },

    #[error("Notification error: {message}")]
    NotificationError { message: String },

    // Cache and session errors
    #[error("Cache error: {message}")]
    CacheError { message: String },

    #[error("Session error: {message}")]
    SessionError { message: String },

    #[error("Session expired")]
    SessionExpired,

    // Configuration and system errors
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("System error: {message}")]
    SystemError { message: String },

    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },

    #[error("Internal server error: {message}")]
    InternalError { message: String },

    // Business logic errors
    #[error("Business logic error: {message}")]
    BusinessLogicError { message: String },

    #[error("Insufficient permissions: {permission}")]
    InsufficientPermissions { permission: String },

    #[error("Operation not allowed: {operation}")]
    OperationNotAllowed { operation: String },

    #[error("Quota exceeded: {quota_type}")]
    QuotaExceeded { quota_type: String },

    #[error("Algorithm audit failed: score {score}, minimum required: {min_score}")]
    AuditFailed { score: i32, min_score: i32 },

    #[error("Vote already exists for this algorithm")]
    VoteAlreadyExists,

    #[error("Invalid wallet address: {address}")]
    InvalidWalletAddress { address: String },

    #[error("Invalid transaction hash: {hash}")]
    InvalidTransactionHash { hash: String },

    // Generic errors
    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Forbidden: {message}")]
    Forbidden { message: String },

    #[error("Method not allowed")]
    MethodNotAllowed,

    #[error("Unsupported media type")]
    UnsupportedMediaType,

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Timeout error: {message}")]
    TimeoutError { message: String },

    #[error("Not implemented: {message}")]
    NotImplemented { message: String },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },
}

/// Validation errors collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrors {
    pub errors: HashMap<String, Vec<String>>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self {
            errors: HashMap::new(),
        }
    }

    pub fn add_error(&mut self, field: String, message: String) {
        self.errors
            .entry(field)
            .or_insert_with(Vec::new)
            .push(message);
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        !self.is_empty()
    }
}

/// API error response format
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub message: String,
    pub code: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: Option<String>,
}

impl ErrorResponse {
    pub fn new(
        error: String,
        message: String,
        code: String,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            success: false,
            error,
            message,
            code,
            details,
            timestamp: chrono::Utc::now(),
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

impl AppError {
    /// Get HTTP status code for the error
    pub fn status_code(&self) -> StatusCode {
        match self {
            // 400 Bad Request
            AppError::ValidationFailed { .. }
            | AppError::InvalidRequest { .. }
            | AppError::MissingField { .. }
            | AppError::InvalidField { .. }
            | AppError::BadRequest { .. }
            | AppError::InvalidWalletAddress { .. }
            | AppError::InvalidTransactionHash { .. }
            | AppError::UnsupportedFileType { .. }
            | AppError::FileTooLarge { .. }
            | AppError::InvalidContentType { .. }
            | AppError::BusinessLogicError { .. } => StatusCode::BAD_REQUEST,

            // 401 Unauthorized
            AppError::AuthenticationFailed { .. }
            | AppError::InvalidToken { .. }
            | AppError::TokenExpired
            | AppError::InvalidApiKey
            | AppError::EmailVerificationRequired
            | AppError::TwoFactorRequired
            | AppError::OAuthError { .. } => StatusCode::UNAUTHORIZED,

            // 403 Forbidden
            AppError::AuthorizationFailed { .. }
            | AppError::AccessDenied { .. }
            | AppError::AccountLocked
            | AppError::InsufficientPermissions { .. }
            | AppError::Forbidden { .. }
            | AppError::OperationNotAllowed { .. } => StatusCode::FORBIDDEN,

            // 404 Not Found
            AppError::NotFound { .. } | AppError::FileNotFound { .. } => StatusCode::NOT_FOUND,

            // 405 Method Not Allowed
            AppError::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,

            // 409 Conflict
            AppError::DuplicateEntry { .. }
            | AppError::AlreadyExists { .. }
            | AppError::Conflict { .. }
            | AppError::VoteAlreadyExists => StatusCode::CONFLICT,

            // 413 Payload Too Large
            AppError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,

            // 415 Unsupported Media Type
            AppError::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,

            // 422 Unprocessable Entity
            AppError::AuditFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,

            // 429 Too Many Requests
            AppError::RateLimitExceeded { .. } | AppError::QuotaExceeded { .. } => {
                StatusCode::TOO_MANY_REQUESTS
            }

            // 500 Internal Server Error
            AppError::Database(_)
            | AppError::Migration(_)
            | AppError::InternalError { .. }
            | AppError::SystemError { .. }
            | AppError::ConfigError { .. }
            | AppError::ConfigurationError { .. }
            | AppError::CacheError { .. }
            | AppError::SessionError { .. }
            | AppError::EmailError { .. }
            | AppError::SmsError { .. }
            | AppError::NotificationError { .. }
            | AppError::FileProcessingError { .. }
            | AppError::SerializationError { .. } => StatusCode::INTERNAL_SERVER_ERROR,

            // 501 Not Implemented
            AppError::NotImplemented { .. } => StatusCode::NOT_IMPLEMENTED,

            // 502 Bad Gateway
            AppError::ExternalApiError { .. }
            | AppError::ExternalServiceError { .. }
            | AppError::AiAuditError { .. }
            | AppError::BlockchainError { .. }
            | AppError::IpfsError { .. }
            | AppError::NetworkError { .. } => StatusCode::BAD_GATEWAY,

            // 503 Service Unavailable
            AppError::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,

            // 504 Gateway Timeout
            AppError::RequestTimeout | AppError::TimeoutError { .. } => StatusCode::GATEWAY_TIMEOUT,

            // Default to 500
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get error code for API responses
    pub fn error_code(&self) -> String {
        match self {
            AppError::Database(_) => "DATABASE_ERROR".to_string(),
            AppError::Migration(_) => "MIGRATION_ERROR".to_string(),
            AppError::AuthenticationFailed { .. } => "AUTHENTICATION_FAILED".to_string(),
            AppError::AuthorizationFailed { .. } => "AUTHORIZATION_FAILED".to_string(),
            AppError::InvalidToken { .. } => "INVALID_TOKEN".to_string(),
            AppError::TokenExpired => "TOKEN_EXPIRED".to_string(),
            AppError::InvalidApiKey => "INVALID_API_KEY".to_string(),
            AppError::AccountLocked => "ACCOUNT_LOCKED".to_string(),
            AppError::EmailVerificationRequired => "EMAIL_VERIFICATION_REQUIRED".to_string(),
            AppError::TwoFactorRequired => "TWO_FACTOR_REQUIRED".to_string(),
            AppError::ValidationFailed { .. } => "VALIDATION_FAILED".to_string(),
            AppError::InvalidRequest { .. } => "INVALID_REQUEST".to_string(),
            AppError::MissingField { .. } => "MISSING_FIELD".to_string(),
            AppError::InvalidField { .. } => "INVALID_FIELD".to_string(),
            AppError::DuplicateEntry { .. } => "DUPLICATE_ENTRY".to_string(),
            AppError::NotFound { .. } => "NOT_FOUND".to_string(),
            AppError::AlreadyExists { .. } => "ALREADY_EXISTS".to_string(),
            AppError::AccessDenied { .. } => "ACCESS_DENIED".to_string(),
            AppError::Conflict { .. } => "CONFLICT".to_string(),
            AppError::ExternalApiError { .. } => "EXTERNAL_API_ERROR".to_string(),
            AppError::AiAuditError { .. } => "AI_AUDIT_ERROR".to_string(),
            AppError::BlockchainError { .. } => "BLOCKCHAIN_ERROR".to_string(),
            AppError::IpfsError { .. } => "IPFS_ERROR".to_string(),
            AppError::FileUploadError { .. } => "FILE_UPLOAD_ERROR".to_string(),
            AppError::FileTooLarge { .. } => "FILE_TOO_LARGE".to_string(),
            AppError::UnsupportedFileType { .. } => "UNSUPPORTED_FILE_TYPE".to_string(),
            AppError::FileNotFound { .. } => "FILE_NOT_FOUND".to_string(),
            AppError::FileProcessingError { .. } => "FILE_PROCESSING_ERROR".to_string(),
            AppError::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED".to_string(),
            AppError::RequestTimeout => "REQUEST_TIMEOUT".to_string(),
            AppError::PayloadTooLarge => "PAYLOAD_TOO_LARGE".to_string(),
            AppError::InvalidContentType { .. } => "INVALID_CONTENT_TYPE".to_string(),
            AppError::EmailError { .. } => "EMAIL_ERROR".to_string(),
            AppError::SmsError { .. } => "SMS_ERROR".to_string(),
            AppError::NotificationError { .. } => "NOTIFICATION_ERROR".to_string(),
            AppError::CacheError { .. } => "CACHE_ERROR".to_string(),
            AppError::SessionError { .. } => "SESSION_ERROR".to_string(),
            AppError::SessionExpired => "SESSION_EXPIRED".to_string(),
            AppError::ConfigError { .. } => "CONFIG_ERROR".to_string(),
            AppError::SystemError { .. } => "SYSTEM_ERROR".to_string(),
            AppError::ServiceUnavailable { .. } => "SERVICE_UNAVAILABLE".to_string(),
            AppError::InternalError { .. } => "INTERNAL_ERROR".to_string(),
            AppError::BusinessLogicError { .. } => "BUSINESS_LOGIC_ERROR".to_string(),
            AppError::InsufficientPermissions { .. } => "INSUFFICIENT_PERMISSIONS".to_string(),
            AppError::OperationNotAllowed { .. } => "OPERATION_NOT_ALLOWED".to_string(),
            AppError::QuotaExceeded { .. } => "QUOTA_EXCEEDED".to_string(),
            AppError::AuditFailed { .. } => "AUDIT_FAILED".to_string(),
            AppError::VoteAlreadyExists => "VOTE_ALREADY_EXISTS".to_string(),
            AppError::InvalidWalletAddress { .. } => "INVALID_WALLET_ADDRESS".to_string(),
            AppError::InvalidTransactionHash { .. } => "INVALID_TRANSACTION_HASH".to_string(),
            AppError::BadRequest { .. } => "BAD_REQUEST".to_string(),
            AppError::Forbidden { .. } => "FORBIDDEN".to_string(),
            AppError::MethodNotAllowed => "METHOD_NOT_ALLOWED".to_string(),
            AppError::UnsupportedMediaType => "UNSUPPORTED_MEDIA_TYPE".to_string(),
            AppError::NetworkError { .. } => "NETWORK_ERROR".to_string(),
            AppError::TimeoutError { .. } => "TIMEOUT_ERROR".to_string(),
            AppError::NotImplemented { .. } => "NOT_IMPLEMENTED".to_string(),
            AppError::SerializationError { .. } => "SERIALIZATION_ERROR".to_string(),
            AppError::ExternalServiceError { .. } => "EXTERNAL_SERVICE_ERROR".to_string(),
            AppError::OAuthError { .. } => "OAUTH_ERROR".to_string(),
            AppError::ConfigurationError { .. } => "CONFIGURATION_ERROR".to_string(),
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            AppError::Database(_) => {
                "A database error occurred. Please try again later.".to_string()
            }
            AppError::Migration(_) => {
                "Database migration failed. Please contact support.".to_string()
            }
            AppError::AuthenticationFailed { .. } => {
                "Authentication failed. Please check your credentials.".to_string()
            }
            AppError::AuthorizationFailed { .. } => {
                "You don't have permission to perform this action.".to_string()
            }
            AppError::InvalidToken { .. } => {
                "Invalid or expired token. Please log in again.".to_string()
            }
            AppError::TokenExpired => "Your session has expired. Please log in again.".to_string(),
            AppError::InvalidApiKey => "Invalid API key. Please check your API key.".to_string(),
            AppError::AccountLocked => {
                "Your account has been temporarily locked due to too many failed login attempts."
                    .to_string()
            }
            AppError::EmailVerificationRequired => {
                "Please verify your email address before continuing.".to_string()
            }
            AppError::TwoFactorRequired => "Two-factor authentication is required.".to_string(),
            AppError::ValidationFailed { .. } => {
                "The provided data is invalid. Please check your input.".to_string()
            }
            AppError::NotFound { resource_type } => format!("{} not found.", resource_type),
            AppError::AlreadyExists { resource_type } => {
                format!("{} already exists.", resource_type)
            }
            AppError::DuplicateEntry { message } => format!("Duplicate entry: {}", message),
            AppError::FileTooLarge { size, max_size } => format!(
                "File is too large ({} bytes). Maximum allowed size is {} bytes.",
                size, max_size
            ),
            AppError::UnsupportedFileType { file_type } => {
                format!("Unsupported file type: {}", file_type)
            }
            AppError::RateLimitExceeded { limit, window } => format!(
                "Rate limit exceeded. Maximum {} requests per {}.",
                limit, window
            ),
            AppError::ServiceUnavailable { service } => format!(
                "{} service is currently unavailable. Please try again later.",
                service
            ),
            AppError::AuditFailed { score, min_score } => format!(
                "Algorithm audit failed. Score: {}, minimum required: {}",
                score, min_score
            ),
            AppError::VoteAlreadyExists => "You have already voted for this algorithm.".to_string(),
            AppError::InvalidWalletAddress { .. } => "Invalid wallet address format.".to_string(),
            AppError::InvalidTransactionHash { .. } => {
                "Invalid transaction hash format.".to_string()
            }
            AppError::NotImplemented { .. } => "This feature is not yet implemented.".to_string(),
            _ => self.to_string(),
        }
    }

    /// Get detailed error information for debugging
    pub fn details(&self) -> Option<serde_json::Value> {
        match self {
            AppError::ValidationFailed { errors } => {
                Some(serde_json::to_value(errors).unwrap_or_default())
            }
            AppError::FileTooLarge { size, max_size } => Some(serde_json::json!({
                "file_size": size,
                "max_allowed_size": max_size
            })),
            AppError::RateLimitExceeded { limit, window } => Some(serde_json::json!({
                "limit": limit,
                "window": window
            })),
            AppError::AuditFailed { score, min_score } => Some(serde_json::json!({
                "audit_score": score,
                "minimum_required_score": min_score
            })),
            _ => None,
        }
    }
}

// Implement IntoResponse for AppError to make it work with Axum
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        let error_code = self.error_code();
        let user_message = self.user_message();
        let details = self.details();

        // Log error for debugging
        match status_code {
            StatusCode::INTERNAL_SERVER_ERROR => {
                error!(
                    error = %self,
                    code = %error_code,
                    "Internal server error occurred"
                );
            }
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE => {
                warn!(
                    error = %self,
                    code = %error_code,
                    "External service error occurred"
                );
            }
            _ => {
                warn!(
                    error = %self,
                    code = %error_code,
                    status = %status_code,
                    "Request error occurred"
                );
            }
        }

        let error_response =
            ErrorResponse::new(self.to_string(), user_message, error_code, details);

        (status_code, Json(error_response)).into_response()
    }
}

// Implement conversions from common error types
impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        AppError::InternalError {
            message: error.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::SerializationError {
            message: error.to_string(),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            AppError::TimeoutError {
                message: error.to_string(),
            }
        } else if error.is_connect() {
            AppError::NetworkError {
                message: error.to_string(),
            }
        } else {
            AppError::ExternalApiError {
                service: "HTTP Client".to_string(),
                message: error.to_string(),
            }
        }
    }
}

impl From<redis::RedisError> for AppError {
    fn from(error: redis::RedisError) -> Self {
        AppError::CacheError {
            message: error.to_string(),
        }
    }
}

impl From<lettre::error::Error> for AppError {
    fn from(error: lettre::error::Error) -> Self {
        AppError::EmailError {
            message: error.to_string(),
        }
    }
}

impl From<lettre::transport::smtp::Error> for AppError {
    fn from(error: lettre::transport::smtp::Error) -> Self {
        AppError::EmailError {
            message: error.to_string(),
        }
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(error: jsonwebtoken::errors::Error) -> Self {
        match error.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::TokenExpired,
            _ => AppError::InvalidToken {
                message: error.to_string(),
            },
        }
    }
}

impl From<config::ConfigError> for AppError {
    fn from(error: config::ConfigError) -> Self {
        AppError::ConfigError {
            message: error.to_string(),
        }
    }
}

/// Helper function to create validation errors
pub fn validation_error(field: &str, message: &str) -> AppError {
    let mut errors = ValidationErrors::new();
    errors.add_error(field.to_string(), message.to_string());
    AppError::ValidationFailed { errors }
}

/// Helper function to create multiple validation errors
pub fn validation_errors(errors: Vec<(String, String)>) -> AppError {
    let mut validation_errors = ValidationErrors::new();
    for (field, message) in errors {
        validation_errors.add_error(field, message);
    }
    AppError::ValidationFailed {
        errors: validation_errors,
    }
}

/// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            AppError::NotFound {
                resource_type: "User".to_string()
            }
            .status_code(),
            StatusCode::NOT_FOUND
        );

        assert_eq!(
            AppError::AuthenticationFailed {
                message: "Invalid credentials".to_string()
            }
            .status_code(),
            StatusCode::UNAUTHORIZED
        );

        assert_eq!(
            AppError::ValidationFailed {
                errors: ValidationErrors::new()
            }
            .status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            AppError::NotFound {
                resource_type: "User".to_string()
            }
            .error_code(),
            "NOT_FOUND"
        );

        assert_eq!(AppError::TokenExpired.error_code(), "TOKEN_EXPIRED");
    }

    #[test]
    fn test_validation_errors() {
        let mut errors = ValidationErrors::new();
        errors.add_error("email".to_string(), "Invalid email format".to_string());
        errors.add_error("password".to_string(), "Password too short".to_string());

        assert!(!errors.is_empty());
        assert!(errors.has_errors());
        assert_eq!(errors.errors.len(), 2);
    }

    #[test]
    fn test_user_messages() {
        let error = AppError::AccountLocked;
        assert!(error.user_message().contains("temporarily locked"));

        let error = AppError::FileTooLarge {
            size: 1000000,
            max_size: 500000,
        };
        assert!(error.user_message().contains("too large"));
    }

    #[test]
    fn test_error_details() {
        let error = AppError::AuditFailed {
            score: 65,
            min_score: 70,
        };
        let details = error.details().unwrap();
        assert_eq!(details["audit_score"], 65);
        assert_eq!(details["minimum_required_score"], 70);
    }
}
