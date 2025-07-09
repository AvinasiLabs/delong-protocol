//! Common utilities and middleware for DeLong Protocol services
//!
//! This crate provides shared functionality that can be used across multiple
//! services in the DeLong Protocol, including middleware, utilities, and
//! common data structures.

pub mod middleware;
pub mod models;
pub mod utils;

use serde::{Deserialize, Serialize};

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
    AuthConfig,
    AuthContext,
    // Blockchain models
    BlockchainTransaction,
    BlockchainTransactionStatus,
    CreateTransactionRequest,
    EntityType,
    TransactionQuery,
    TransactionResponse,
    UpdateTransactionStatusRequest,
    is_valid_tx_hash,
    JOIN_CONFIRMED_TX,

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
    Claims,
    JwtUtils,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: ResponseCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub timestamp: String,
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
            message: "Operation completed successfully".to_string(),
            request_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a successful response with custom message
    pub fn success_with_message(data: T, message: &str) -> Self {
        Self {
            code: ResponseCode::Success,
            data: Some(data),
            message: message.to_string(),
            request_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a successful response with request ID
    pub fn success_with_id(data: T, request_id: String) -> Self {
        Self {
            code: ResponseCode::Success,
            data: Some(data),
            message: "Operation completed successfully".to_string(),
            request_id: Some(request_id),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a successful response with message and request ID
    pub fn success_with_message_and_id(data: T, message: &str, request_id: String) -> Self {
        Self {
            code: ResponseCode::Success,
            data: Some(data),
            message: message.to_string(),
            request_id: Some(request_id),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl ApiResponse<()> {
    /// Create an error response
    pub fn error(code: ResponseCode, message: &str) -> Self {
        Self {
            code,
            data: None,
            message: message.to_string(),
            request_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create an error response with request ID
    pub fn error_with_id(code: ResponseCode, message: &str, request_id: String) -> Self {
        Self {
            code,
            data: None,
            message: message.to_string(),
            request_id: Some(request_id),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a bad request error
    pub fn bad_request(message: &str) -> Self {
        Self::error(ResponseCode::BadRequest, message)
    }

    /// Create an unauthorized error
    pub fn unauthorized(message: &str) -> Self {
        Self::error(ResponseCode::Unauthorized, message)
    }

    /// Create a forbidden error
    pub fn forbidden(message: &str) -> Self {
        Self::error(ResponseCode::Forbidden, message)
    }

    /// Create a not found error
    pub fn not_found(message: &str) -> Self {
        Self::error(ResponseCode::NotFound, message)
    }

    /// Create an internal server error
    pub fn internal_error(message: &str) -> Self {
        Self::error(ResponseCode::InternalServerError, message)
    }
}

/// Common error types that can be converted to API responses
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),
}

impl ApiError {
    /// Convert ApiError to ResponseCode
    pub fn to_response_code(&self) -> ResponseCode {
        match self {
            ApiError::BadRequest(_) => ResponseCode::BadRequest,
            ApiError::Unauthorized(_) => ResponseCode::Unauthorized,
            ApiError::Forbidden(_) => ResponseCode::Forbidden,
            ApiError::NotFound(_) => ResponseCode::NotFound,
            ApiError::InternalError(_) => ResponseCode::InternalServerError,
            ApiError::Timeout(_) => ResponseCode::Timeout,
            ApiError::TooManyRequests(_) => ResponseCode::TooManyRequests,
        }
    }

    /// Convert to ApiResponse
    pub fn to_response(self) -> ApiResponse<()> {
        ApiResponse::error(self.to_response_code(), &self.to_string())
    }

    /// Convert to ApiResponse with request ID
    pub fn to_response_with_id(self, request_id: String) -> ApiResponse<()> {
        ApiResponse::error_with_id(self.to_response_code(), &self.to_string(), request_id)
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
        assert_eq!(response.message, "Operation completed successfully");
        assert!(response.request_id.is_none());
        assert!(!response.timestamp.is_empty());
    }

    #[test]
    fn test_api_response_success_with_message() {
        let data = 42;
        let response = ApiResponse::success_with_message(data, "Custom success message");

        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some(42));
        assert_eq!(response.message, "Custom success message");
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
    fn test_api_response_success_with_message_and_id() {
        let data = true;
        let response = ApiResponse::success_with_message_and_id(
            data,
            "Operation successful",
            "req_456".to_string(),
        );

        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some(true));
        assert_eq!(response.message, "Operation successful");
        assert_eq!(response.request_id, Some("req_456".to_string()));
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> =
            ApiResponse::error(ResponseCode::BadRequest, "Invalid input provided");

        assert_eq!(response.code, ResponseCode::BadRequest);
        assert!(response.data.is_none());
        assert_eq!(response.message, "Invalid input provided");
        assert!(response.request_id.is_none());
    }

    #[test]
    fn test_api_response_error_with_id() {
        let response: ApiResponse<()> = ApiResponse::error_with_id(
            ResponseCode::InternalServerError,
            "Something went wrong",
            "req_789".to_string(),
        );

        assert_eq!(response.code, ResponseCode::InternalServerError);
        assert!(response.data.is_none());
        assert_eq!(response.message, "Something went wrong");
        assert_eq!(response.request_id, Some("req_789".to_string()));
    }

    #[test]
    fn test_api_response_convenience_errors() {
        let bad_request = ApiResponse::bad_request("Bad input");
        assert_eq!(bad_request.code, ResponseCode::BadRequest);
        assert_eq!(bad_request.message, "Bad input");

        let unauthorized = ApiResponse::unauthorized("Access denied");
        assert_eq!(unauthorized.code, ResponseCode::Unauthorized);
        assert_eq!(unauthorized.message, "Access denied");

        let forbidden = ApiResponse::forbidden("Permission denied");
        assert_eq!(forbidden.code, ResponseCode::Forbidden);
        assert_eq!(forbidden.message, "Permission denied");

        let not_found = ApiResponse::not_found("Resource not found");
        assert_eq!(not_found.code, ResponseCode::NotFound);
        assert_eq!(not_found.message, "Resource not found");

        let internal_error = ApiResponse::internal_error("Server error");
        assert_eq!(internal_error.code, ResponseCode::InternalServerError);
        assert_eq!(internal_error.message, "Server error");
    }

    #[test]
    fn test_api_error_to_response_code() {
        assert_eq!(
            ApiError::BadRequest("test".to_string()).to_response_code(),
            ResponseCode::BadRequest
        );
        assert_eq!(
            ApiError::Unauthorized("test".to_string()).to_response_code(),
            ResponseCode::Unauthorized
        );
        assert_eq!(
            ApiError::Forbidden("test".to_string()).to_response_code(),
            ResponseCode::Forbidden
        );
        assert_eq!(
            ApiError::NotFound("test".to_string()).to_response_code(),
            ResponseCode::NotFound
        );
        assert_eq!(
            ApiError::InternalError("test".to_string()).to_response_code(),
            ResponseCode::InternalServerError
        );
        assert_eq!(
            ApiError::Timeout("test".to_string()).to_response_code(),
            ResponseCode::Timeout
        );
        assert_eq!(
            ApiError::TooManyRequests("test".to_string()).to_response_code(),
            ResponseCode::TooManyRequests
        );
    }

    #[test]
    fn test_api_error_to_response() {
        let error = ApiError::BadRequest("Invalid data".to_string());
        let response = error.to_response();

        assert_eq!(response.code, ResponseCode::BadRequest);
        assert!(response.data.is_none());
        assert_eq!(response.message, "Bad request: Invalid data");
        assert!(response.request_id.is_none());
    }

    #[test]
    fn test_api_error_to_response_with_id() {
        let error = ApiError::InternalError("Database error".to_string());
        let response = error.to_response_with_id("req_error_123".to_string());

        assert_eq!(response.code, ResponseCode::InternalServerError);
        assert!(response.data.is_none());
        assert_eq!(response.message, "Internal server error: Database error");
        assert_eq!(response.request_id, Some("req_error_123".to_string()));
    }

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse::success("test");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"code\":\"SUCCESS\""));
        assert!(json.contains("\"data\":\"test\""));
        assert!(json.contains("\"message\":\"Operation completed successfully\""));
    }

    #[test]
    fn test_api_response_error_serialization() {
        let response: ApiResponse<()> = ApiResponse::bad_request("Invalid request");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"code\":\"BAD_REQUEST\""));
        assert!(json.contains("\"message\":\"Invalid request\""));
        // data should be omitted when None due to skip_serializing_if
        assert!(!json.contains("\"data\""));
    }

    #[test]
    fn test_api_response_deserialization() {
        let json = r#"{
            "code": "SUCCESS",
            "data": "test",
            "message": "Success",
            "timestamp": "2023-01-01T00:00:00Z"
        }"#;

        let response: ApiResponse<String> = serde_json::from_str(json).unwrap();
        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some("test".to_string()));
        assert_eq!(response.message, "Success");
        assert_eq!(response.timestamp, "2023-01-01T00:00:00Z");
        assert!(response.request_id.is_none());
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
        let error = ApiError::BadRequest("Invalid input".to_string());
        assert_eq!(error.to_string(), "Bad request: Invalid input");

        let error = ApiError::NotFound("Resource missing".to_string());
        assert_eq!(error.to_string(), "Not found: Resource missing");
    }
}
