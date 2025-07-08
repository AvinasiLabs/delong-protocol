//! Core service handlers module
//!
//! This module contains all HTTP request handlers organized by business domain.
//! Each handler module contains functions that process specific API endpoints
//! and return appropriate HTTP responses.

// Handler modules organized by business domain
pub mod admin;
pub mod ai_audit;
pub mod auth;

pub mod health;

// Re-export commonly used handlers for easier access
pub use admin::{
    check_permission, create_api_key, create_user_admin, delete_user, get_api_key_stats,
    get_permissions, get_roles, get_user_by_id, get_users, list_api_keys, revoke_api_key,
    update_user,
};
pub use ai_audit::{create_ai_audit, get_ai_audit_report, list_ai_audit_reports};
pub use auth::{
    get_current_user, google_auth_callback, google_auth_url, login_user, logout_user,
    refresh_token, register_user, send_verification_code, update_wallet_address,
};
pub use health::health_check;

// Helper functions are defined below

// Use unified API response types from common crate
pub use common::{ApiError, ApiResponse, ApiResult, ResponseCode};

use axum::{http::StatusCode, response::Json};
use serde_json::{Value, json};

/// Create an error response with status code
pub fn error_response(code: String, _message: String) -> (StatusCode, Json<ApiResponse<()>>) {
    match code.as_str() {
        "NOT_FOUND" => (StatusCode::NOT_FOUND, Json(ApiResponse::not_found())),
        "CONFLICT" => (StatusCode::CONFLICT, Json(ApiResponse::bad_request())),
        "FORBIDDEN" => (StatusCode::FORBIDDEN, Json(ApiResponse::forbidden())),
        "UNAUTHORIZED" => (StatusCode::UNAUTHORIZED, Json(ApiResponse::unauthorized())),
        "BAD_REQUEST" => (StatusCode::BAD_REQUEST, Json(ApiResponse::bad_request())),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::internal_error()),
        ),
    }
}

/// Create an error response with status code
pub fn error_response_with_status(
    _message: &str,
    status: StatusCode,
) -> (StatusCode, Json<ApiResponse<()>>) {
    (status, Json(ApiResponse::bad_request()))
}

/// Create a JSON response with data
pub fn json_response<T: serde::Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse::success(data))
}

/// Create a JSON response with data and message
pub fn json_response_with_message<T: serde::Serialize>(
    data: T,
    _message: &str,
) -> Json<ApiResponse<T>> {
    Json(ApiResponse::success(data))
}

/// Create a successful response with status code
pub fn success_response_with_status<T: serde::Serialize>(
    data: T,
    status: StatusCode,
) -> (StatusCode, Json<ApiResponse<T>>) {
    (status, Json(ApiResponse::success(data)))
}

/// Create a not found error response
pub fn not_found_response(_message: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (StatusCode::NOT_FOUND, Json(ApiResponse::not_found()))
}

/// Create an internal server error response
pub fn internal_error_response(_message: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::internal_error()),
    )
}

/// Create an unauthorized error response
pub fn unauthorized_response(_message: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (StatusCode::UNAUTHORIZED, Json(ApiResponse::unauthorized()))
}

/// Create a forbidden error response
pub fn forbidden_response(_message: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (StatusCode::FORBIDDEN, Json(ApiResponse::forbidden()))
}

/// Create a bad request error response
pub fn bad_request_response(_message: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (StatusCode::BAD_REQUEST, Json(ApiResponse::bad_request()))
}

/// Root handler for API base path
pub async fn root_handler() -> Json<Value> {
    Json(json!({
        "service": "DeLong Protocol Core Service",
        "version": crate::VERSION,
        "status": "running"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_handler_response() {
        // Test that root handler returns expected JSON structure
        let response = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(root_handler());
        let json: Value = response.0;

        assert!(json.get("service").is_some());
        assert!(json.get("version").is_some());
        assert!(json.get("status").is_some());
        assert_eq!(json["service"], "DeLong Protocol Core Service");
        assert_eq!(json["status"], "running");
    }
}
