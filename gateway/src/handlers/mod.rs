//! Request handlers for the Delong gateway
//!
//! This module contains all the HTTP request handlers organized by functionality:
//! - algo_exe: Algorithm execution lifecycle management handlers
//! - auth: API key management and authentication handlers
//! - committee: Committee member management handlers
//! - contract: Smart contract metadata handlers
//! - dataset: Dataset upload and management handlers
//! - health: Health check handlers
//! - report: Test report management handlers
//! - vote: Voting system handlers
//! - websocket: Real-time WebSocket notification handlers

pub mod algo_exe;
pub mod auth;
pub mod committee;
pub mod contract;
pub mod dataset;
pub mod health;
pub mod report;
pub mod vote;
pub mod websocket;

// Common response types used across handlers

// Use unified API response and pagination types from common crate
pub use common::{
    ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginationParams, ResponseCode,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert_eq!(response.code, ResponseCode::Success);
        assert_eq!(response.data, Some("test data"));
        assert_eq!(response.message, "Operation completed successfully");
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::bad_request("test error");
        assert_eq!(response.code, ResponseCode::BadRequest);
        assert!(response.data.is_none());
        assert_eq!(response.message, "test error");
    }

    #[test]
    fn test_pagination_params_default() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
    }

    #[test]
    fn test_paginated_response() {
        let items = vec!["item1", "item2", "item3"];
        let response = PaginatedResponse::new(items, 1, 20, 100);
        assert_eq!(response.items.len(), 3);
        assert_eq!(response.total_items, 100);
        assert_eq!(response.total_pages, 5);
    }
}
