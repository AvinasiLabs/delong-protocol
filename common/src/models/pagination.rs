//! Pagination data models
//!
//! This module contains shared pagination structures for consistent
//! paginated responses across all services in the DeLong Protocol.

use serde::{Deserialize, Serialize};

/// Pagination parameters for requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationParams {
    /// Page number (1-based indexing)
    #[serde(default = "default_page")]
    pub page: u32,

    /// Number of items per page
    #[serde(default = "default_limit")]
    pub limit: u32,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self { page: 1, limit: 20 }
    }
}

impl PaginationParams {
    /// Create new pagination params with validation
    pub fn new(page: u32, limit: u32) -> Result<Self, String> {
        if page == 0 {
            return Err("Page must be greater than 0".to_string());
        }

        if limit == 0 {
            return Err("Limit must be greater than 0".to_string());
        }

        if limit > MAX_LIMIT {
            return Err(format!("Limit cannot exceed {}", MAX_LIMIT));
        }

        Ok(Self { page, limit })
    }

    /// Calculate offset for database queries (0-based)
    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.limit
    }

    /// Validate pagination parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.page == 0 {
            return Err("Page must be greater than 0".to_string());
        }

        if self.limit == 0 {
            return Err("Limit must be greater than 0".to_string());
        }

        if self.limit > MAX_LIMIT {
            return Err(format!("Limit cannot exceed {}", MAX_LIMIT));
        }

        Ok(())
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaginatedResponse<T> {
    /// Items for the current page
    pub items: Vec<T>,

    /// Current page number (1-based)
    pub page: u32,

    /// Number of items per page
    pub limit: u32,

    /// Total number of items across all pages
    pub total_items: u64,

    /// Total number of pages
    pub total_pages: u32,

    /// Whether there is a next page
    pub has_next: bool,

    /// Whether there is a previous page
    pub has_previous: bool,
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(items: Vec<T>, page: u32, limit: u32, total_items: u64) -> Self {
        let total_pages = if total_items == 0 {
            0
        } else {
            ((total_items - 1) / limit as u64 + 1) as u32
        };

        let has_next = page < total_pages;
        let has_previous = page > 1;

        Self {
            items,
            page,
            limit,
            total_items,
            total_pages,
            has_next,
            has_previous,
        }
    }

    /// Create an empty paginated response
    pub fn empty(page: u32, limit: u32) -> Self {
        Self::new(Vec::new(), page, limit, 0)
    }

    /// Get the number of items on the current page
    pub fn current_page_size(&self) -> usize {
        self.items.len()
    }

    /// Check if the current page is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the range of items shown on current page (1-based)
    pub fn item_range(&self) -> Option<(u64, u64)> {
        if self.is_empty() {
            return None;
        }

        let start = ((self.page - 1) as u64 * self.limit as u64) + 1;
        let end = start + self.items.len() as u64 - 1;
        Some((start, end))
    }
}

/// Maximum allowed items per page
pub const MAX_LIMIT: u32 = 100;

/// Default page number
fn default_page() -> u32 {
    1
}

/// Default limit per page
fn default_limit() -> u32 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_params_default() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
    }

    #[test]
    fn test_pagination_params_new_valid() {
        let params = PaginationParams::new(2, 50).unwrap();
        assert_eq!(params.page, 2);
        assert_eq!(params.limit, 50);
    }

    #[test]
    fn test_pagination_params_new_invalid_page() {
        let result = PaginationParams::new(0, 20);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Page must be greater than 0");
    }

    #[test]
    fn test_pagination_params_new_invalid_limit() {
        let result = PaginationParams::new(1, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Limit must be greater than 0");
    }

    #[test]
    fn test_pagination_params_new_limit_too_large() {
        let result = PaginationParams::new(1, 200);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Limit cannot exceed 100");
    }

    #[test]
    fn test_pagination_params_offset() {
        let params = PaginationParams::new(1, 20).unwrap();
        assert_eq!(params.offset(), 0);

        let params = PaginationParams::new(2, 20).unwrap();
        assert_eq!(params.offset(), 20);

        let params = PaginationParams::new(3, 10).unwrap();
        assert_eq!(params.offset(), 20);
    }

    #[test]
    fn test_pagination_params_validate() {
        let params = PaginationParams::new(1, 20).unwrap();
        assert!(params.validate().is_ok());

        let params = PaginationParams { page: 0, limit: 20 };
        assert!(params.validate().is_err());

        let params = PaginationParams { page: 1, limit: 0 };
        assert!(params.validate().is_err());

        let params = PaginationParams {
            page: 1,
            limit: 200,
        };
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_pagination_params_serialization() {
        let params = PaginationParams::new(2, 50).unwrap();
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"page\":2"));
        assert!(json.contains("\"limit\":50"));

        let deserialized: PaginationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_pagination_params_deserialization_with_defaults() {
        let json = "{}";
        let params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
    }

    #[test]
    fn test_paginated_response_new() {
        let items = vec![1, 2, 3];
        let response = PaginatedResponse::new(items, 1, 10, 25);

        assert_eq!(response.items, vec![1, 2, 3]);
        assert_eq!(response.page, 1);
        assert_eq!(response.limit, 10);
        assert_eq!(response.total_items, 25);
        assert_eq!(response.total_pages, 3);
        assert!(response.has_next);
        assert!(!response.has_previous);
    }

    #[test]
    fn test_paginated_response_empty() {
        let response: PaginatedResponse<i32> = PaginatedResponse::empty(1, 10);

        assert!(response.items.is_empty());
        assert_eq!(response.page, 1);
        assert_eq!(response.limit, 10);
        assert_eq!(response.total_items, 0);
        assert_eq!(response.total_pages, 0);
        assert!(!response.has_next);
        assert!(!response.has_previous);
    }

    #[test]
    fn test_paginated_response_last_page() {
        let items = vec![21, 22, 23, 24, 25];
        let response = PaginatedResponse::new(items, 3, 10, 25);

        assert_eq!(response.items.len(), 5);
        assert_eq!(response.page, 3);
        assert_eq!(response.total_pages, 3);
        assert!(!response.has_next);
        assert!(response.has_previous);
    }

    #[test]
    fn test_paginated_response_middle_page() {
        let items = vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
        let response = PaginatedResponse::new(items, 2, 10, 25);

        assert_eq!(response.items.len(), 10);
        assert_eq!(response.page, 2);
        assert_eq!(response.total_pages, 3);
        assert!(response.has_next);
        assert!(response.has_previous);
    }

    #[test]
    fn test_paginated_response_current_page_size() {
        let items = vec![1, 2, 3];
        let response = PaginatedResponse::new(items, 1, 10, 25);
        assert_eq!(response.current_page_size(), 3);
    }

    #[test]
    fn test_paginated_response_is_empty() {
        let response: PaginatedResponse<i32> = PaginatedResponse::empty(1, 10);
        assert!(response.is_empty());

        let items = vec![1, 2, 3];
        let response = PaginatedResponse::new(items, 1, 10, 25);
        assert!(!response.is_empty());
    }

    #[test]
    fn test_paginated_response_item_range() {
        let response: PaginatedResponse<i32> = PaginatedResponse::empty(1, 10);
        assert_eq!(response.item_range(), None);

        let items = vec![1, 2, 3];
        let response = PaginatedResponse::new(items, 1, 10, 25);
        assert_eq!(response.item_range(), Some((1, 3)));

        let items = vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
        let response = PaginatedResponse::new(items, 2, 10, 25);
        assert_eq!(response.item_range(), Some((11, 20)));
    }

    #[test]
    fn test_paginated_response_total_pages_calculation() {
        // Exact division
        let response: PaginatedResponse<i32> = PaginatedResponse::new(vec![], 1, 10, 20);
        assert_eq!(response.total_pages, 2);

        // With remainder
        let response: PaginatedResponse<i32> = PaginatedResponse::new(vec![], 1, 10, 25);
        assert_eq!(response.total_pages, 3);

        // Zero items
        let response: PaginatedResponse<i32> = PaginatedResponse::new(vec![], 1, 10, 0);
        assert_eq!(response.total_pages, 0);

        // One item
        let response: PaginatedResponse<i32> = PaginatedResponse::new(vec![], 1, 10, 1);
        assert_eq!(response.total_pages, 1);
    }

    #[test]
    fn test_paginated_response_serialization() {
        let items = vec![1, 2, 3];
        let response = PaginatedResponse::new(items, 1, 10, 25);

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: PaginatedResponse<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }
}
