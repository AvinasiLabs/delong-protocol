//! Example demonstrating how to use mock HTTP client for testing
//!
//! This example shows how the new trait-based HTTP client design enables
//! easy testing with mocked backend responses.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use gateway::{
    config::GatewayConfig,
    create_router,
    utils::http_client::{BackendClient, HttpClientError},
};
use serde_json::json;
use std::collections::HashMap;
use tower::util::ServiceExt;

/// Mock HTTP client for testing
/// This implements the BackendClient trait and allows you to control responses
pub struct MockHttpClient {
    responses: HashMap<String, serde_json::Value>,
    should_fail: bool,
    fail_status: u16,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            should_fail: false,
            fail_status: 500,
        }
    }

    /// Add a mock response for a specific URL
    pub fn with_response(mut self, url: &str, response: serde_json::Value) -> Self {
        self.responses.insert(url.to_string(), response);
        self
    }

    /// Configure the client to always fail with the given status code
    pub fn with_failure(mut self, status: u16) -> Self {
        self.should_fail = true;
        self.fail_status = status;
        self
    }
}

#[async_trait]
impl BackendClient for MockHttpClient {
    async fn forward_request_json(
        &self,
        url: &str,
        _method: reqwest::Method,
        _payload: Option<serde_json::Value>,
        _headers: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, HttpClientError> {
        if self.should_fail {
            return Err(HttpClientError::HttpError {
                status: self.fail_status,
                message: "Mock failure".to_string(),
            });
        }

        if let Some(response) = self.responses.get(url) {
            Ok(response.clone())
        } else {
            Err(HttpClientError::HttpError {
                status: 404,
                message: "Mock response not found".to_string(),
            })
        }
    }
}

#[tokio::main]
async fn main() {
    // Example 1: Testing successful backend responses
    println!("=== Example 1: Testing Successful Response ===");

    let mock_client = MockHttpClient::new().with_response(
        "http://localhost:8081/api/algo-exes?page=1&limit=20",
        json!({
            "items": [
                {
                    "id": 1,
                    "algo_id": "test-algo-123",
                    "used_dataset": "test-dataset",
                    "scientist_wallet": "0x123",
                    "review_status": "approved",
                    "vote_start_time": null,
                    "vote_end_time": null,
                    "status": "completed",
                    "start_time": "2023-01-01T00:00:00Z",
                    "end_time": "2023-01-01T01:00:00Z",
                    "result": "successful execution",
                    "error_msg": null,
                    "created_at": "2023-01-01T00:00:00Z",
                    "updated_at": "2023-01-01T01:00:00Z",
                    "algo_name": "Test Algorithm",
                    "algo_link": "https://github.com/test/repo",
                    "cid": "QmTestCID123"
                }
            ],
            "total": 1,
            "page": 1,
            "limit": 20,
            "total_pages": 1
        }),
    );

    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = false; // Disable auth for testing
    let app = create_router(&config, mock_client);

    let request = Request::builder()
        .uri("/api/algo-exes")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    println!("Response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::OK);

    // Example 2: Testing backend failure scenarios
    println!("\n=== Example 2: Testing Backend Failure ===");

    let failing_client = MockHttpClient::new().with_failure(503);
    let app_failing = create_router(&config, failing_client);

    let request = Request::builder()
        .uri("/api/algo-exes")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();

    let response = app_failing.oneshot(request).await.unwrap();
    println!("Response status: {}", response.status());
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // Example 3: Testing different endpoints with different responses
    println!("\n=== Example 3: Testing Multiple Endpoints ===");

    let multi_mock_client = MockHttpClient::new()
        .with_response(
            "http://localhost:8081/api/committee?page=1&limit=20",
            json!({
                "items": [
                    {
                        "id": 1,
                        "member_wallet": "0x456",
                        "is_approved": true,
                        "created_at": "2023-01-01T00:00:00Z",
                        "updated_at": "2023-01-01T00:00:00Z"
                    }
                ],
                "total": 1,
                "page": 1,
                "limit": 20,
                "total_pages": 1
            }),
        )
        .with_response(
            "http://localhost:8081/api/votes?page=1&limit=20",
            json!({
                "items": [],
                "total": 0,
                "page": 1,
                "limit": 20,
                "total_pages": 0
            }),
        );

    let app_multi = create_router(&config, multi_mock_client);

    // Test committee endpoint
    let committee_request = Request::builder()
        .uri("/api/committee")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();

    let committee_response = app_multi.clone().oneshot(committee_request).await.unwrap();
    println!("Committee response status: {}", committee_response.status());
    assert_eq!(committee_response.status(), StatusCode::OK);

    // Test votes endpoint
    let votes_request = Request::builder()
        .uri("/api/votes")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();

    let votes_response = app_multi.oneshot(votes_request).await.unwrap();
    println!("Votes response status: {}", votes_response.status());
    assert_eq!(votes_response.status(), StatusCode::OK);

    println!("\n=== All Examples Completed Successfully! ===");

    // Benefits of this approach:
    println!("\n=== Benefits of Trait-Based HTTP Client Design ===");
    println!("✅ Easy to mock backend responses for testing");
    println!("✅ Can test both success and failure scenarios");
    println!("✅ No need for actual backend services in tests");
    println!("✅ Fast and reliable unit tests");
    println!("✅ Dependency injection enables better software design");
    println!("✅ Can easily switch between real and mock clients");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_client_integration() {
        let mock_client = MockHttpClient::new().with_response(
            "http://localhost:8081/api/algo-exes?page=1&limit=20",
            json!({
                "items": [],
                "total": 0,
                "page": 1,
                "limit": 20,
                "total_pages": 0
            }),
        );

        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = false;
        let app = create_router(&config, mock_client);

        let request = Request::builder()
            .uri("/api/algo-exes")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_mock_client_failure_scenarios() {
        // Test 503 Service Unavailable
        let failing_client = MockHttpClient::new().with_failure(503);
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = false;
        let app = create_router(&config, failing_client);

        let request = Request::builder()
            .uri("/api/algo-exes")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_real_vs_mock_client_compatibility() {
        // This test demonstrates that you can easily switch between
        // real and mock clients since they implement the same trait

        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = false;

        // Both clients implement BackendClient trait
        let _real_client = HttpBackendClient::new(30);
        let _mock_client = MockHttpClient::new();

        // Both can be used to create the router
        // let app_real = create_router(&config, real_client);
        // let app_mock = create_router(&config, mock_client);

        // This flexibility allows for:
        // - Production: Use HttpBackendClient
        // - Testing: Use MockHttpClient
        // - Integration tests: Use HttpBackendClient with test backends
        // - Unit tests: Use MockHttpClient with predefined responses
    }
}
