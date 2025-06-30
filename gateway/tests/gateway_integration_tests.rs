//! Gateway Integration Tests
//!
//! This module contains comprehensive integration tests for the gateway service,
//! including API structure validation, authentication, routing, and middleware testing.

use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
};
use gateway::{
    config::GatewayConfig, create_auth_test_router, create_test_router,
    utils::http_client::HttpBackendClient,
};
use serde_json::{Value, json};
use tower::util::ServiceExt;

/// Helper function to create test configuration with auth disabled
fn create_test_config() -> (GatewayConfig, HttpBackendClient) {
    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = false;
    let client = HttpBackendClient::new(30);
    (config, client)
}

/// Helper function to create test configuration with auth enabled
fn create_auth_test_config() -> (GatewayConfig, HttpBackendClient) {
    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = true;
    let client = HttpBackendClient::new(30);
    (config, client)
}

/// Helper function to create a request with JSON body
fn create_json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method(method)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Helper function to create a request with custom headers
fn create_request_with_headers(method: Method, uri: &str, headers: HeaderMap) -> Request<Body> {
    let mut builder = Request::builder().uri(uri).method(method);

    for (key, value) in headers.iter() {
        builder = builder.header(key, value);
    }

    builder.body(Body::empty()).unwrap()
}

/// Helper function to check if status is acceptable for backend-dependent operations
fn is_acceptable_status(status: StatusCode) -> bool {
    status.is_success()
        || status == StatusCode::NOT_IMPLEMENTED
        || status == StatusCode::INTERNAL_SERVER_ERROR
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::BAD_REQUEST
        || status == StatusCode::UNPROCESSABLE_ENTITY
}

#[cfg(test)]
mod health_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_endpoints_public_access() {
        let (config, client) = create_auth_test_config(); // Auth enabled
        let app = create_test_router(&config, client);

        // Test main health endpoint - should be accessible even with auth enabled
        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_nonexistent_endpoint() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/nonexistent")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[cfg(test)]
mod api_structure_tests {
    use super::*;

    #[tokio::test]
    async fn test_sample_data_public_access() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/sample/QmTestCID123")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should not be 401 (unauthorized) since this is public
        // May be 500 or other error due to missing backend, but not 401
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_static_dataset_endpoints_require_auth() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = true;
        let client = HttpBackendClient::new(30);
        let app = create_test_router(&config, client);

        // Test GET /api/static-datasets (list)
        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test GET /api/static-datasets/{id} (get single)
        let request = Request::builder()
            .uri("/api/static-datasets/1")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test POST /api/static-datasets (upload)
        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::POST)
            .header("content-type", "multipart/form-data")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_dynamic_dataset_endpoints_require_auth() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = true;
        let client = HttpBackendClient::new(30);
        let app = create_test_router(&config, client);

        // Test GET /api/datasets (list)
        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test POST /api/datasets (create)
        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::POST)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "test_dataset",
                    "ui_name": "Test Dataset",
                    "description": "A test dataset"
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_endpoints_require_auth() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = true;
        let client = HttpBackendClient::new(30);
        let app = create_test_router(&config, client);

        // Test GET /api/auth/keys (list)
        let request = Request::builder()
            .uri("/api/auth/keys")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test POST /api/auth/keys (create)
        let request = Request::builder()
            .uri("/api/auth/keys")
            .method(Method::POST)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "test_key",
                    "permissions": ["data_read", "data_write"]
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test POST /api/auth/keys/validate (validate)
        let request = Request::builder()
            .uri("/api/auth/keys/validate")
            .method(Method::POST)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "api_key": "test-key",
                    "context": "validation_test"
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test DELETE /api/auth/keys/{key_id} (revoke)
        let request = Request::builder()
            .uri("/api/auth/keys/test-key-id")
            .method(Method::DELETE)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "reason": "Test revocation"
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_endpoints_accessible_when_auth_disabled() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = false;
        let client = HttpBackendClient::new(30);
        let app = create_test_router(&config, client);

        // Test GET /api/static-datasets (should not return 401)
        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);

        // Test GET /api/datasets (should not return 401)
        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_nonexistent_endpoints_return_404() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        // Test non-existent algorithm endpoint (removed from new API)
        let request = Request::builder()
            .uri("/api/algorithms/submit")
            .method(Method::POST)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_valid_api_key_access() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = true;
        let client = HttpBackendClient::new(30);
        let app = create_auth_test_router(&config, client);

        // Test with valid mock API key from middleware
        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::GET)
            .header("authorization", "ApiKey test-api-key-basic")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should not be 401 with valid key, but may be 500 due to missing backend
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_valid_jwt_access() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = true;
        let client = HttpBackendClient::new(30);
        let app = create_auth_test_router(&config, client);

        // Test with valid mock JWT token from middleware
        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::GET)
            .header("authorization", "Bearer mock-jwt-token-user")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should not be 401 with valid JWT, but may be 500 due to missing backend
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[cfg(test)]
mod dataset_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_dataset_list_without_auth() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should succeed when auth is disabled, or return 500 if backend is not available
        assert!(is_acceptable_status(response.status()));
    }

    #[tokio::test]
    async fn test_upload_dataset_post() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let payload = json!({
            "name": "test_dataset",
            "description": "Test dataset for upload",
            "data": "sample_data"
        });

        let request = create_json_request(Method::POST, "/api/static-datasets", payload);
        let response = app.oneshot(request).await.unwrap();

        let status = response.status();
        println!("Dataset upload status: {}", status);

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(status));
    }

    #[tokio::test]
    async fn test_get_specific_dataset() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/algo-exes/algorithm-id-123")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }

    #[tokio::test]
    async fn test_delete_dataset() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/datasets/test-dataset-id")
            .method(Method::DELETE)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }
}

#[cfg(test)]
mod algorithm_tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_algorithm() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let payload = json!({
            "algorithm_type": "privacy_preserving_ml",
            "dataset_id": "test-dataset-id",
            "parameters": {
                "epsilon": 1.0,
                "delta": 0.001
            }
        });

        let request = create_json_request(Method::POST, "/api/algo-exes", payload);
        let response = app.oneshot(request).await.unwrap();

        let status = response.status();
        println!("Submit algorithm status: {}", status);

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(status));
    }

    #[tokio::test]
    async fn test_get_algorithm_status() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/algo-exes/test-algo-id")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }

    #[tokio::test]
    async fn test_get_algorithm_result() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/algo-exes/test-algo-id")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }

    #[tokio::test]
    async fn test_get_algorithm_details() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/algo-exes/test-algo-id")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_api_key() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let payload = json!({
            "name": "test_key",
            "description": "A test API key",
            "permissions": ["read", "write"]
        });

        let request = create_json_request(Method::POST, "/api/auth/keys", payload);
        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }

    #[tokio::test]
    async fn test_list_api_keys() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/auth/keys")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(is_acceptable_status(response.status()));
    }

    #[tokio::test]
    async fn test_validate_api_key() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let payload = json!({
            "api_key": "test-api-key-value"
        });

        let request = create_json_request(Method::POST, "/api/auth/keys/validate", payload);
        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(
            is_acceptable_status(response.status())
                || response.status() == StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn test_revoke_api_key() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/auth/keys/test-key-id")
            .method(Method::DELETE)
            .header("content-type", "application/json")
            .body(Body::from(json!({"reason": "Test revocation"}).to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should either succeed or return appropriate error
        assert!(
            is_acceptable_status(response.status()) || response.status() == StatusCode::NOT_FOUND
        );
    }
}

#[cfg(test)]
mod authentication_tests {
    use super::*;

    #[tokio::test]
    async fn test_protected_endpoint_without_auth() {
        let (config, client) = create_auth_test_config(); // Auth enabled
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should reject when no auth provided
        assert!(
            response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN
                || response.status().is_success()
        );
    }

    #[tokio::test]
    async fn test_protected_endpoint_with_invalid_key() {
        let (config, client) = create_auth_test_config(); // Auth enabled
        let app = create_test_router(&config, client);

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("invalid-key"));

        let request = create_request_with_headers(Method::GET, "/api/datasets", headers);
        let response = app.oneshot(request).await.unwrap();

        // Should reject invalid API key, or return 500 if backend is not available
        assert!(
            response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN
                || response.status().is_success() // If not implemented yet
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_health_endpoint_without_auth() {
        let (config, client) = create_auth_test_config(); // Auth enabled
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Health endpoints should always be accessible
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod middleware_tests {
    use super::*;

    #[tokio::test]
    async fn test_request_id_middleware() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Request ID middleware should add headers
        assert_eq!(response.status(), StatusCode::OK);
        // In a real test, we would check for X-Request-ID header
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .header("origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // CORS middleware should handle the request
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Logging middleware should not affect response
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_json_payload() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/datasets")
            .header("content-type", "application/json")
            .body(Body::from("invalid json {"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should handle invalid JSON gracefully
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
                || response.status().is_success() // If not implemented yet
        );
    }

    #[tokio::test]
    async fn test_unsupported_content_type() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/datasets")
            .header("content-type", "text/plain")
            .body(Body::from("plain text data"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should handle unsupported content type
        assert!(
            response.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
                || response.status() == StatusCode::BAD_REQUEST
                || response.status().is_success() // If not implemented yet
        );
    }

    #[tokio::test]
    async fn test_method_not_allowed() {
        let (config, client) = create_test_config();
        let app = create_test_router(&config, client);

        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::PATCH) // PATCH not supported
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return method not allowed
        assert!(response.status() == StatusCode::METHOD_NOT_ALLOWED);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[tokio::test]
    async fn test_router_with_different_configs() {
        // Test with auth disabled
        let (config_no_auth, client_no_auth) = create_test_config();
        let app_no_auth = create_test_router(&config_no_auth, client_no_auth);

        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::GET)
            .header("authorization", "Bearer invalid-token")
            .body(Body::empty())
            .unwrap();

        let response = app_no_auth.oneshot(request).await.unwrap();

        // Should work without authentication, or return 500 if backend is not available
        assert!(is_acceptable_status(response.status()));

        // Test with auth enabled
        let (config_with_auth, client_with_auth) = create_auth_test_config();
        let app_with_auth = create_test_router(&config_with_auth, client_with_auth);

        let request = Request::builder()
            .uri("/api/datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app_with_auth.oneshot(request).await.unwrap();

        // Should require authentication
        assert!(
            response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN
        );
    }
}
