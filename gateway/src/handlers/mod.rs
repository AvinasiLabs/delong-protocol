//! Request handlers for the Delong gateway
//!
//! This module contains all the HTTP request handlers organized by functionality:
//! - algorithm: Algorithm submission and execution handlers
//! - auth: API key management and authentication handlers
//! - data: Dataset upload and management handlers
//! - health: Health check handlers
//! - metrics: Metrics and monitoring handlers

pub mod algorithm;
pub mod auth;
pub mod data;
pub mod health;
// pub mod metrics;

// Common response types used across handlers
use serde::{Deserialize, Serialize};

/// Standard API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: String,
    pub request_id: Option<String>,
}

#[allow(dead_code)]
impl<T> ApiResponse<T>
where
    T: Serialize,
{
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: None,
        }
    }

    /// Create a successful response with request ID
    pub fn success_with_id(data: T, request_id: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: Some(request_id),
        }
    }
}

#[allow(dead_code)]
impl ApiResponse<()> {
    /// Create an error response
    pub fn error(error: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: None,
        }
    }

    /// Create an error response with request ID
    pub fn error_with_id(error: &str, request_id: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: Some(request_id),
        }
    }
}

/// Common pagination parameters
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            limit: default_limit(),
        }
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub limit: u32,
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(items: Vec<T>, total: u64, page: u32, limit: u32) -> Self {
        let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
        Self {
            items,
            total,
            page,
            limit,
            total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::error("test error");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
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
        let response = PaginatedResponse::new(items, 100, 1, 20);
        assert_eq!(response.items.len(), 3);
        assert_eq!(response.total, 100);
        assert_eq!(response.total_pages, 5);
    }
}
