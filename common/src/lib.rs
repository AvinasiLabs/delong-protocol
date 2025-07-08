//! Common utilities and middleware for DeLong Protocol services
//!
//! This crate provides shared functionality that can be used across multiple
//! services in the DeLong Protocol, including middleware, utilities, and
//! common data structures.

pub mod config;
pub mod middleware;
pub mod models;
pub mod server;
pub mod utils;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Conditional imports for error conversion
#[cfg(feature = "database")]
use sqlx;

#[cfg(feature = "auth")]
use bcrypt;

// Re-export commonly used items for convenience
pub use models::{
    // Algorithm execution models
    AlgoExeData,
    AlgoExeStatus,
    AlgoExeSubmissionRequest,
    AlgoExeSubmissionResponse,
    AlgoReviewStatus,

    // Authentication models
    ApiKeyInfo,
    ApiKeyListQuery,
    AuthContext,
    AuthMethod,
    // WebSocket models
    BlockchainTransactionNotification,
    // Vote models
    CastVoteRequest,
    // Committee models
    CommitteeMemberData,
    CommitteeMemberResponse,
    ConnectionStats,
    // Contract models
    ContractData,
    ContractResponse,
    CreateApiKeyRequest,
    CreateApiKeyResponse,
    CreateContractRequest,
    // Dataset models
    CreateDatasetRequest,
    DatasetFormat,
    DatasetPaginatedResponse,
    DatasetStatus,
    DelongApiResponse,
    DynamicDatasetInfo,
    DynamicDatasetListQuery,
    ExtendedContractData,
    ExtendedVoteData,
    JwtClaims,
    // Pagination models
    MAX_LIMIT,
    MembershipCheckResponse,
    NotificationMessage,
    PaginatedResponse,
    PaginationParams,

    Permission,
    RateLimitTier,
    // Report models
    ReportInfo,
    ReportQuery,
    ReportStatus,
    ReportSummary,
    ReportType,
    RevokeApiKeyRequest,
    RevokeApiKeyResponse,
    SetCommitteeMemberRequest,

    SetVoteDurationRequest,
    StaticDatasetInfo,
    StaticDatasetListQuery,
    SubscriptionRequest,
    SubscriptionResponse,
    TransactionStatus,
    UpdateContractRequest,

    UpdateDatasetRequest,
    UpdateStaticDatasetRequest,

    UploadReportRequest,
    UploadReportResponse,

    UserSession,
    ValidateApiKeyRequest,
    ValidateApiKeyResponse,
    VoteData,
    VoteDecision,
    VoteDurationResponse,
    VoteQuery,
    VoteStatus,
    VoteSummary,
    VotingSession,

    WebSocketClient,
    generate_client_id,
    is_valid_api_key_format,
};

pub use middleware::{
    MiddlewareUtils,
    logging::{
        LoggingConfig, log_large_request, log_slow_request, logging_middleware,
        security_logging_middleware,
    },
    request_id::{
        RequestIdConfig, RequestIdGenerator, generate_request_id, get_request_id_from_headers,
        request_id_middleware, request_id_middleware_with_config,
    },
};

// Re-export key constants
pub use middleware::REQUEST_ID_HEADER;

// Re-export server utilities
pub use server::{
    OpenTelemetryConfig, ServerConfig, get_env_bool, get_env_string, get_env_u16, get_env_u64,
    init_logging, load_env_file, parse_socket_addr, shutdown_signal, start_server,
};

/// Common error types that can be used across services
#[derive(Debug, thiserror::Error)]
pub enum CommonError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Time error: {0}")]
    Time(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Standard API response codes following the project design
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum ResponseCode {
    #[serde(rename = "SUCCESS")]
    Success,
    #[serde(rename = "BAD_REQUEST")]
    BadRequest,
    #[serde(rename = "UNAUTHORIZED")]
    Unauthorized,
    #[serde(rename = "FORBIDDEN")]
    Forbidden,
    #[serde(rename = "NOT_FOUND")]
    NotFound,
    #[serde(rename = "INTERNAL_SERVER_ERROR")]
    InternalServerError,
    #[serde(rename = "TIMEOUT")]
    Timeout,
    #[serde(rename = "TOO_MANY_REQUESTS")]
    TooManyRequests,
}

impl ResponseCode {
    /// Convert ResponseCode to HTTP status code
    pub fn to_status_code(&self) -> u16 {
        match self {
            ResponseCode::Success => 200,
            ResponseCode::BadRequest => 400,
            ResponseCode::Unauthorized => 401,
            ResponseCode::Forbidden => 403,
            ResponseCode::NotFound => 404,
            ResponseCode::Timeout => 408,
            ResponseCode::TooManyRequests => 429,
            ResponseCode::InternalServerError => 500,
        }
    }
}

/// Standard API response structure following the project design
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    pub code: ResponseCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            code: ResponseCode::Success,
            data: Some(data),
            request_id: None,
        }
    }

    /// Create a successful response with request ID
    pub fn success_with_id(data: T, request_id: String) -> Self {
        Self {
            code: ResponseCode::Success,
            data: Some(data),
            request_id: Some(request_id),
        }
    }
}

impl ApiResponse<()> {
    /// Create an error response
    pub fn error(code: ResponseCode) -> Self {
        Self {
            code,
            data: None,
            request_id: None,
        }
    }

    /// Create an error response with request ID
    pub fn error_with_id(code: ResponseCode, request_id: String) -> Self {
        Self {
            code,
            data: None,
            request_id: Some(request_id),
        }
    }

    /// Create a bad request error
    pub fn bad_request() -> Self {
        Self::error(ResponseCode::BadRequest)
    }

    /// Create a bad request error with request ID
    pub fn bad_request_with_id(request_id: String) -> Self {
        Self::error_with_id(ResponseCode::BadRequest, request_id)
    }

    /// Create an unauthorized error
    pub fn unauthorized() -> Self {
        Self::error(ResponseCode::Unauthorized)
    }

    /// Create an unauthorized error with request ID
    pub fn unauthorized_with_id(request_id: String) -> Self {
        Self::error_with_id(ResponseCode::Unauthorized, request_id)
    }

    /// Create a forbidden error
    pub fn forbidden() -> Self {
        Self::error(ResponseCode::Forbidden)
    }

    /// Create a forbidden error with request ID
    pub fn forbidden_with_id(request_id: String) -> Self {
        Self::error_with_id(ResponseCode::Forbidden, request_id)
    }

    /// Create a not found error
    pub fn not_found() -> Self {
        Self::error(ResponseCode::NotFound)
    }

    /// Create a not found error with request ID
    pub fn not_found_with_id(request_id: String) -> Self {
        Self::error_with_id(ResponseCode::NotFound, request_id)
    }

    /// Create an internal server error
    pub fn internal_error() -> Self {
        Self::error(ResponseCode::InternalServerError)
    }

    /// Create an internal server error with request ID
    pub fn internal_error_with_id(request_id: String) -> Self {
        Self::error_with_id(ResponseCode::InternalServerError, request_id)
    }
}

/// Common error types that can be converted to API responses
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Bad request")]
    BadRequest,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Not found")]
    NotFound,

    #[error("Internal server error")]
    InternalError,

    #[error("Timeout")]
    Timeout,

    #[error("Too many requests")]
    TooManyRequests,

    // Authentication errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Account is not active")]
    AccountInactive,

    #[error("Resource already exists: {0}")]
    AlreadyExists(String),

    #[error("Server error")]
    ServerError,

    #[error("Email configuration error")]
    EmailConfigError,

    #[error("Invalid verification code")]
    VerificationCodeInvalid,

    #[error("Email is required")]
    EmailRequired,

    #[error("Password is required")]
    PasswordRequired,

    #[error("Username is required")]
    UsernameRequired,

    #[error("Verification code is required")]
    VerificationCodeRequired,

    #[error("Invalid email format")]
    EmailInvalidFormat,

    #[error("Password too short")]
    PasswordTooShort,

    #[error("Invalid username format")]
    UsernameInvalidFormat,

    #[error("Username already taken")]
    UsernameTaken,

    #[error("Duplicate entry")]
    DuplicateEntry,

    #[error("Email already sent")]
    EmailAlreadySent,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Migration error")]
    MigrationError,

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Account locked")]
    AccountLocked,

    #[error("Email verification required")]
    EmailVerificationRequired,

    #[error("Two factor authentication required")]
    TwoFactorRequired,

    #[error("Validation failed")]
    ValidationFailed,

    #[error("Invalid request")]
    InvalidRequest,

    #[error("Missing field")]
    MissingField,

    #[error("Invalid field")]
    InvalidField,

    #[error("Conflict")]
    Conflict,

    #[error("External API error")]
    ExternalApiError,

    #[error("OAuth error: {0}")]
    OAuthError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("AI audit error")]
    AiAuditError,

    #[error("Blockchain error")]
    BlockchainError,

    #[error("File upload error")]
    FileUploadError,

    #[error("File too large")]
    FileTooLarge,

    #[error("Unsupported file type")]
    UnsupportedFileType,

    #[error("File not found")]
    FileNotFound,

    #[error("File processing error")]
    FileProcessingError,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Request timeout")]
    RequestTimeout,

    #[error("Payload too large")]
    PayloadTooLarge,

    #[error("Invalid content type")]
    InvalidContentType,

    #[error("Email error")]
    EmailError,

    #[error("SMS error")]
    SmsError,

    #[error("Notification error")]
    NotificationError,

    #[error("Cache error")]
    CacheError,

    #[error("Session error")]
    SessionError,

    #[error("Session expired")]
    SessionExpired,

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("Business logic error")]
    BusinessLogicError,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Operation not allowed")]
    OperationNotAllowed,

    #[error("Quota exceeded")]
    QuotaExceeded,

    #[error("Audit failed")]
    AuditFailed,

    #[error("Vote already exists")]
    VoteAlreadyExists,

    #[error("Invalid wallet address")]
    InvalidWalletAddress,

    #[error("Invalid transaction hash")]
    InvalidTransactionHash,

    #[error("Method not allowed")]
    MethodNotAllowed,

    #[error("Unsupported media type")]
    UnsupportedMediaType,

    #[error("Network error")]
    NetworkError,

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Serialization error")]
    SerializationError,
}

impl ApiError {
    /// Convert ApiError to ResponseCode
    pub fn to_response_code(&self) -> ResponseCode {
        match self {
            ApiError::BadRequest => ResponseCode::BadRequest,
            ApiError::Unauthorized => ResponseCode::Unauthorized,
            ApiError::Forbidden => ResponseCode::Forbidden,
            ApiError::NotFound => ResponseCode::NotFound,
            ApiError::InternalError => ResponseCode::InternalServerError,
            ApiError::Timeout => ResponseCode::Timeout,
            ApiError::TooManyRequests => ResponseCode::TooManyRequests,

            // Authentication and input validation errors
            ApiError::InvalidInput(_) => ResponseCode::BadRequest,
            ApiError::UserNotFound => ResponseCode::NotFound,
            ApiError::InvalidPassword => ResponseCode::Unauthorized,
            ApiError::AccountInactive => ResponseCode::Forbidden,
            ApiError::AlreadyExists(_) => ResponseCode::BadRequest,
            ApiError::ServerError => ResponseCode::InternalServerError,
            ApiError::EmailConfigError => ResponseCode::InternalServerError,
            ApiError::VerificationCodeInvalid => ResponseCode::BadRequest,
            ApiError::EmailRequired => ResponseCode::BadRequest,
            ApiError::PasswordRequired => ResponseCode::BadRequest,
            ApiError::UsernameRequired => ResponseCode::BadRequest,
            ApiError::VerificationCodeRequired => ResponseCode::BadRequest,
            ApiError::EmailInvalidFormat => ResponseCode::BadRequest,
            ApiError::PasswordTooShort => ResponseCode::BadRequest,
            ApiError::UsernameInvalidFormat => ResponseCode::BadRequest,
            ApiError::UsernameTaken => ResponseCode::BadRequest,
            ApiError::DuplicateEntry => ResponseCode::BadRequest,
            ApiError::EmailAlreadySent => ResponseCode::TooManyRequests,

            // Database and system errors
            ApiError::DatabaseError(_) => ResponseCode::InternalServerError,
            ApiError::MigrationError => ResponseCode::InternalServerError,
            ApiError::AuthenticationFailed => ResponseCode::Unauthorized,
            ApiError::InvalidToken => ResponseCode::Unauthorized,
            ApiError::TokenExpired => ResponseCode::Unauthorized,
            ApiError::InvalidApiKey => ResponseCode::Unauthorized,
            ApiError::AccountLocked => ResponseCode::Forbidden,
            ApiError::EmailVerificationRequired => ResponseCode::Forbidden,
            ApiError::TwoFactorRequired => ResponseCode::Forbidden,
            ApiError::ValidationFailed => ResponseCode::BadRequest,
            ApiError::InvalidRequest => ResponseCode::BadRequest,
            ApiError::MissingField => ResponseCode::BadRequest,
            ApiError::InvalidField => ResponseCode::BadRequest,
            ApiError::Conflict => ResponseCode::BadRequest,

            // External service errors
            ApiError::ExternalApiError => ResponseCode::InternalServerError,
            ApiError::OAuthError(_) => ResponseCode::BadRequest,
            ApiError::ConfigurationError(_) => ResponseCode::InternalServerError,
            ApiError::AiAuditError => ResponseCode::InternalServerError,
            ApiError::BlockchainError => ResponseCode::InternalServerError,

            // File handling errors
            ApiError::FileUploadError => ResponseCode::BadRequest,
            ApiError::FileTooLarge => ResponseCode::BadRequest,
            ApiError::UnsupportedFileType => ResponseCode::BadRequest,
            ApiError::FileNotFound => ResponseCode::NotFound,
            ApiError::FileProcessingError => ResponseCode::InternalServerError,

            // Rate limiting and request errors
            ApiError::RateLimitExceeded => ResponseCode::TooManyRequests,
            ApiError::RequestTimeout => ResponseCode::Timeout,
            ApiError::PayloadTooLarge => ResponseCode::BadRequest,
            ApiError::InvalidContentType => ResponseCode::BadRequest,

            // Communication errors
            ApiError::EmailError => ResponseCode::InternalServerError,
            ApiError::SmsError => ResponseCode::InternalServerError,
            ApiError::NotificationError => ResponseCode::InternalServerError,

            // Session and cache errors
            ApiError::CacheError => ResponseCode::InternalServerError,
            ApiError::SessionError => ResponseCode::InternalServerError,
            ApiError::SessionExpired => ResponseCode::Unauthorized,
            ApiError::ServiceUnavailable => ResponseCode::InternalServerError,

            // Business logic errors
            ApiError::BusinessLogicError => ResponseCode::BadRequest,
            ApiError::InsufficientPermissions => ResponseCode::Forbidden,
            ApiError::OperationNotAllowed => ResponseCode::Forbidden,
            ApiError::QuotaExceeded => ResponseCode::TooManyRequests,
            ApiError::AuditFailed => ResponseCode::BadRequest,
            ApiError::VoteAlreadyExists => ResponseCode::BadRequest,
            ApiError::InvalidWalletAddress => ResponseCode::BadRequest,
            ApiError::InvalidTransactionHash => ResponseCode::BadRequest,

            // HTTP method errors
            ApiError::MethodNotAllowed => ResponseCode::BadRequest,
            ApiError::UnsupportedMediaType => ResponseCode::BadRequest,
            ApiError::NetworkError => ResponseCode::InternalServerError,
            ApiError::NotImplemented(_) => ResponseCode::InternalServerError,
            ApiError::SerializationError => ResponseCode::InternalServerError,
        }
    }

    /// Convert to ApiResponse
    pub fn to_response(self) -> ApiResponse<()> {
        ApiResponse::error(self.to_response_code())
    }

    /// Convert to ApiResponse with request ID
    pub fn to_response_with_id(self, request_id: String) -> ApiResponse<()> {
        ApiResponse::error_with_id(self.to_response_code(), request_id)
    }
}

// Error conversion implementations
#[cfg(feature = "database")]
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ApiError::NotFound,
            sqlx::Error::Database(db_err) if db_err.constraint().is_some() => {
                ApiError::AlreadyExists("Resource already exists".to_string())
            }
            _ => ApiError::DatabaseError("Database operation failed".to_string()),
        }
    }
}

#[cfg(feature = "auth")]
impl From<bcrypt::BcryptError> for ApiError {
    fn from(_: bcrypt::BcryptError) -> Self {
        ApiError::AuthenticationFailed
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(_: serde_json::Error) -> Self {
        ApiError::SerializationError
    }
}

/// Result type alias for common operations
pub type Result<T> = std::result::Result<T, CommonError>;

/// Result type alias for API operations
pub type ApiResult<T> = std::result::Result<T, ApiError>;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_code_to_status_code() {
        assert_eq!(ResponseCode::Success.to_status_code(), 200);
        assert_eq!(ResponseCode::BadRequest.to_status_code(), 400);
        assert_eq!(ResponseCode::Unauthorized.to_status_code(), 401);
        assert_eq!(ResponseCode::Forbidden.to_status_code(), 403);
        assert_eq!(ResponseCode::NotFound.to_status_code(), 404);
        assert_eq!(ResponseCode::Timeout.to_status_code(), 408);
        assert_eq!(ResponseCode::TooManyRequests.to_status_code(), 429);
        assert_eq!(ResponseCode::InternalServerError.to_status_code(), 500);
    }

    #[test]
    fn test_response_code_serialization() {
        let success = ResponseCode::Success;
        let json = serde_json::to_string(&success).unwrap();
        assert_eq!(json, "\"SUCCESS\"");

        let bad_request = ResponseCode::BadRequest;
        let json = serde_json::to_string(&bad_request).unwrap();
        assert_eq!(json, "\"BAD_REQUEST\"");
    }

    #[test]
    fn test_response_code_deserialization() {
        let json = "\"SUCCESS\"";
        let code: ResponseCode = serde_json::from_str(json).unwrap();
        assert_eq!(code, ResponseCode::Success);

        let json = "\"INTERNAL_SERVER_ERROR\"";
        let code: ResponseCode = serde_json::from_str(json).unwrap();
        assert_eq!(code, ResponseCode::InternalServerError);
    }

    #[test]
    fn test_api_response_success() {
        let data = "test data";
        let response = ApiResponse::success(data);

        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.request_id.is_none());
    }

    #[test]
    fn test_api_response_success_with_id() {
        let data = vec![1, 2, 3];
        let response = ApiResponse::success_with_id(data, "req_123".to_string());

        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some(vec![1, 2, 3]));
        assert_eq!(response.request_id, Some("req_123".to_string()));
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::error(ResponseCode::BadRequest);

        assert_eq!(response.code, ResponseCode::BadRequest);
        assert!(response.data.is_none());
        assert!(response.request_id.is_none());
    }

    #[test]
    fn test_api_response_error_with_id() {
        let response: ApiResponse<()> =
            ApiResponse::error_with_id(ResponseCode::InternalServerError, "req_789".to_string());

        assert_eq!(response.code, ResponseCode::InternalServerError);
        assert!(response.data.is_none());
        assert_eq!(response.request_id, Some("req_789".to_string()));
    }

    #[test]
    fn test_api_response_convenience_errors() {
        let bad_request = ApiResponse::bad_request();
        assert_eq!(bad_request.code, ResponseCode::BadRequest);

        let unauthorized = ApiResponse::unauthorized();
        assert_eq!(unauthorized.code, ResponseCode::Unauthorized);

        let forbidden = ApiResponse::forbidden();
        assert_eq!(forbidden.code, ResponseCode::Forbidden);

        let not_found = ApiResponse::not_found();
        assert_eq!(not_found.code, ResponseCode::NotFound);

        let internal_error = ApiResponse::internal_error();
        assert_eq!(internal_error.code, ResponseCode::InternalServerError);

        // Test with request IDs
        let bad_request_with_id = ApiResponse::bad_request_with_id("req_123".to_string());
        assert_eq!(bad_request_with_id.code, ResponseCode::BadRequest);
        assert_eq!(bad_request_with_id.request_id, Some("req_123".to_string()));
    }

    #[test]
    fn test_api_error_to_response_code() {
        assert_eq!(
            ApiError::BadRequest.to_response_code(),
            ResponseCode::BadRequest
        );
        assert_eq!(
            ApiError::Unauthorized.to_response_code(),
            ResponseCode::Unauthorized
        );
        assert_eq!(
            ApiError::Forbidden.to_response_code(),
            ResponseCode::Forbidden
        );
        assert_eq!(
            ApiError::NotFound.to_response_code(),
            ResponseCode::NotFound
        );
        assert_eq!(
            ApiError::InternalError.to_response_code(),
            ResponseCode::InternalServerError
        );
        assert_eq!(ApiError::Timeout.to_response_code(), ResponseCode::Timeout);
        assert_eq!(
            ApiError::TooManyRequests.to_response_code(),
            ResponseCode::TooManyRequests
        );
    }

    #[test]
    fn test_api_error_to_response() {
        let error = ApiError::BadRequest;
        let response = error.to_response();

        assert_eq!(response.code, ResponseCode::BadRequest);
        assert!(response.data.is_none());
        assert!(response.request_id.is_none());
    }

    #[test]
    fn test_api_error_to_response_with_id() {
        let error = ApiError::InternalError;
        let response = error.to_response_with_id("req_error_123".to_string());

        assert_eq!(response.code, ResponseCode::InternalServerError);
        assert!(response.data.is_none());
        assert_eq!(response.request_id, Some("req_error_123".to_string()));
    }

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::success("test");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"code\":\"SUCCESS\""));
        assert!(json.contains("\"data\":\"test\""));
        // request_id should be omitted when None due to skip_serializing_if
        assert!(!json.contains("\"request_id\""));
    }

    #[test]
    fn test_api_response_error_serialization() {
        let response: ApiResponse<()> = ApiResponse::bad_request();
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"code\":\"BAD_REQUEST\""));
        // data should be omitted when None due to skip_serializing_if
        assert!(!json.contains("\"data\""));
        // request_id should be omitted when None due to skip_serializing_if
        assert!(!json.contains("\"request_id\""));
    }

    #[test]
    fn test_api_response_deserialization() {
        let json = r#"{
            "code": "SUCCESS",
            "data": "test",
            "request_id": "req_123"
        }"#;

        let response: ApiResponse<String> = serde_json::from_str(json).unwrap();
        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some("test".to_string()));
        assert_eq!(response.request_id, Some("req_123".to_string()));
    }

    #[test]
    fn test_common_error_display() {
        let error = CommonError::Internal("Test error".to_string());
        assert_eq!(error.to_string(), "Internal error: Test error");

        let error = CommonError::InvalidConfig("Bad config".to_string());
        assert_eq!(error.to_string(), "Invalid configuration: Bad config");
    }

    #[test]
    fn test_api_error_display() {
        let error = ApiError::BadRequest;
        assert_eq!(error.to_string(), "Bad request");

        let error = ApiError::NotFound;
        assert_eq!(error.to_string(), "Not found");
    }
}
