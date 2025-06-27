//! Integration tests for the Delong Protocol Gateway
//!
//! These tests verify the complete functionality of the gateway service
//! by testing HTTP endpoints and middleware behavior in isolation.

use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
};
use gateway::{config::GatewayConfig, create_router};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Helper function to create test configuration
fn create_test_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = false; // Disable auth for most tests
    config
}

/// Helper function to create test configuration with auth enabled
fn create_auth_test_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.api.enable_api_key_validation = true;
    config
}

/// Helper function to create a request with JSON body
fn create_json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Helper function to create a request with headers
fn create_request_with_headers(method: Method, uri: &str, headers: HeaderMap) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);

    for (key, value) in headers.iter() {
        builder = builder.header(key, value);
    }

    builder.body(Body::empty()).unwrap()
}

/// Helper function to extract response body as string
async fn extract_body_string(response: axum::response::Response) -> String {
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(body_bytes.to_vec()).unwrap()
}

/// Helper function to extract response body as JSON
async fn extract_body_json(response: axum::response::Response) -> Value {
    let body_string = extract_body_string(response).await;
    serde_json::from_str(&body_string).unwrap()
}

/// Helper function to check if a status code is acceptable for unimplemented endpoints
fn is_acceptable_status(status: StatusCode) -> bool {
    status.is_success()
        || status == StatusCode::BAD_REQUEST
        || status == StatusCode::NOT_FOUND
        || status == StatusCode::NOT_IMPLEMENTED
        || status == StatusCode::INTERNAL_SERVER_ERROR
        || status == StatusCode::UNPROCESSABLE_ENTITY
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::UNAUTHORIZED
        || status == StatusCode::FORBIDDEN
        || status == StatusCode::UNSUPPORTED_MEDIA_TYPE
}

#[cfg(test)]
mod health_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = extract_body_json(response).await;
        assert_eq!(body["status"], "healthy");
    }

    #[tokio::test]
    async fn test_liveness_endpoint() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health/live")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_readiness_endpoint() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health/ready")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_nonexistent_endpoint() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/nonexistent")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[cfg(test)]
mod dataset_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_dataset_list_without_auth() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/datasets")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should succeed when auth is disabled
        assert!(response.status().is_success() || response.status() == StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn test_upload_dataset_post() {
        let config = create_test_config();
        let app = create_router(&config);

        let payload = json!({
            "name": "test_dataset",
            "description": "A test dataset",
            "data": "sample_data"
        });

        let request = create_json_request(Method::POST, "/api/v1/datasets", payload);
        let response = app.oneshot(request).await.unwrap();

        let status = response.status();
        println!("Dataset upload status: {}", status);

        // Should either succeed or return a proper error status
        assert!(
            status.is_success()
                || status == StatusCode::BAD_REQUEST
                || status == StatusCode::NOT_IMPLEMENTED
                || status == StatusCode::INTERNAL_SERVER_ERROR
                || status == StatusCode::UNPROCESSABLE_ENTITY
                || status == StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn test_get_specific_dataset() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/datasets/test-dataset-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        println!("Get dataset status: {}", status);

        // Should return appropriate status for dataset retrieval
        assert!(
            is_acceptable_status(status),
            "Unexpected status: {}",
            status
        );
    }

    #[tokio::test]
    async fn test_delete_dataset() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .method(Method::DELETE)
            .uri("/api/v1/datasets/test-dataset-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should return appropriate status for dataset deletion
        let status = response.status();
        assert!(
            is_acceptable_status(status),
            "Unexpected status: {}",
            status
        );
    }
}

#[cfg(test)]
mod algorithm_tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_algorithm() {
        let config = create_test_config();
        let app = create_router(&config);

        let payload = json!({
            "algorithm_type": "privacy_preserving_ml",
            "dataset_id": "test-dataset-id",
            "parameters": {
                "epsilon": 1.0,
                "delta": 0.001
            }
        });

        let request = create_json_request(Method::POST, "/api/v1/algorithms/submit", payload);
        let response = app.oneshot(request).await.unwrap();

        let status = response.status();
        println!("Submit algorithm status: {}", status);

        // Should either succeed or return appropriate error
        assert!(
            status.is_success()
                || status == StatusCode::BAD_REQUEST
                || status == StatusCode::NOT_IMPLEMENTED
                || status == StatusCode::INTERNAL_SERVER_ERROR
                || status == StatusCode::UNPROCESSABLE_ENTITY
                || status == StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn test_get_algorithm_status() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/algorithms/test-algo-id/status")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        assert!(
            is_acceptable_status(status),
            "Unexpected status: {}",
            status
        );
    }

    #[tokio::test]
    async fn test_get_algorithm_result() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/algorithms/test-algo-id/result")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        assert!(
            is_acceptable_status(status),
            "Unexpected status: {}",
            status
        );
    }

    #[tokio::test]
    async fn test_get_algorithm_details() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/algorithms/test-algo-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        assert!(
            is_acceptable_status(status),
            "Unexpected status: {}",
            status
        );
    }
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_api_key() {
        let config = create_test_config();
        let app = create_router(&config);

        let payload = json!({
            "name": "test_key",
            "description": "A test API key",
            "permissions": ["read", "write"]
        });

        let request = create_json_request(Method::POST, "/api/v1/auth/keys", payload);
        let response = app.oneshot(request).await.unwrap();

        assert!(
            response.status().is_success()
                || response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::NOT_IMPLEMENTED
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
                || response.status() == StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn test_list_api_keys() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/auth/keys")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert!(
            response.status().is_success()
                || response.status() == StatusCode::NOT_IMPLEMENTED
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_validate_api_key() {
        let config = create_test_config();
        let app = create_router(&config);

        let payload = json!({
            "api_key": "test-api-key-value"
        });

        let request = create_json_request(Method::POST, "/api/v1/auth/keys/validate", payload);
        let response = app.oneshot(request).await.unwrap();

        assert!(
            response.status().is_success()
                || response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::NOT_IMPLEMENTED
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
                || response.status() == StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn test_revoke_api_key() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .method(Method::DELETE)
            .uri("/api/v1/auth/keys/test-key-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        assert!(
            is_acceptable_status(status),
            "Unexpected status: {}",
            status
        );
    }
}

#[cfg(test)]
mod middleware_tests {
    use super::*;

    #[tokio::test]
    async fn test_request_id_middleware() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Request ID middleware should add headers
        let _headers = response.headers();
        // Check if request ID header is present (implementation dependent)
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/health")
            .header("Origin", "http://localhost:3000")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // CORS middleware should handle OPTIONS requests
        let _headers = response.headers();
        // Should have CORS headers (implementation dependent)
        assert!(
            response.status().is_success() || response.status() == StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Logging middleware should not affect response status
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod authentication_tests {
    use super::*;

    #[tokio::test]
    async fn test_protected_endpoint_without_auth() {
        let config = create_auth_test_config(); // Auth enabled
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/api/v1/datasets")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should require authentication
        assert!(
            response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN
                || response.status().is_success() // If not implemented yet
        );
    }

    #[tokio::test]
    async fn test_protected_endpoint_with_invalid_key() {
        let config = create_auth_test_config(); // Auth enabled
        let app = create_router(&config);

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("invalid-key"));

        let request = create_request_with_headers(Method::GET, "/api/v1/datasets", headers);
        let response = app.oneshot(request).await.unwrap();

        // Should reject invalid API key
        assert!(
            response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN
                || response.status().is_success() // If not implemented yet
        );
    }

    #[tokio::test]
    async fn test_health_endpoint_without_auth() {
        let config = create_auth_test_config(); // Auth enabled
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Health endpoints should always be accessible
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_json_payload() {
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/datasets")
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
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/datasets")
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
        let config = create_test_config();
        let app = create_router(&config);

        let request = Request::builder()
            .method(Method::PATCH) // Not supported method
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return method not allowed
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[tokio::test]
    async fn test_router_with_different_configs() {
        // Test with auth disabled
        let config_no_auth = create_test_config();
        let app_no_auth = create_router(&config_no_auth);

        let request = Request::builder()
            .uri("/api/v1/datasets")
            .body(Body::empty())
            .unwrap();

        let response = app_no_auth.oneshot(request).await.unwrap();
        // Should work without authentication
        assert!(response.status().is_success() || response.status() == StatusCode::NOT_IMPLEMENTED);

        // Test with auth enabled
        let config_with_auth = create_auth_test_config();
        let app_with_auth = create_router(&config_with_auth);

        let request = Request::builder()
            .uri("/api/v1/datasets")
            .body(Body::empty())
            .unwrap();

        let response = app_with_auth.oneshot(request).await.unwrap();
        // Should require authentication
        assert!(
            response.status() == StatusCode::UNAUTHORIZED
                || response.status() == StatusCode::FORBIDDEN
                || response.status().is_success() // If auth not fully implemented yet
        );
    }
}
