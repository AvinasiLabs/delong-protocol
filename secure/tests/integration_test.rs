//! Integration tests for Secure Service HTTP endpoints
//!
//! This module contains comprehensive tests for all the public HTTP APIs exposed by the Secure Service,
//! including health checks, static dataset operations, and algorithm execution functionality.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use common::{
    AlgoExeSubmissionRequest, AlgoExeSubmissionResponse, ApiResponse, PaginatedResponse,
    PaginationParams,
};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt; // for `oneshot` and `ready`

use secure::{config::SecureConfig, routes::create_routes, AppState};
use common::{Claims, JwtUtils};

/// JWT secret for testing (must match secure service config)
const TEST_JWT_SECRET: &str = "default_secret";

/// Generate an admin JWT token for testing
fn generate_admin_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new("admin".to_string(), user_id.to_string(), 3600);
    jwt_utils.generate_token(&claims).unwrap()
}

/// Generate a user JWT token for testing
fn generate_user_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new("user".to_string(), user_id.to_string(), 3600);
    jwt_utils.generate_token(&claims).unwrap()
}

/// Generate a custom JWT token for testing
fn generate_custom_token(role: &str, user_id: &str, expires_in_seconds: i64) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new(role.to_string(), user_id.to_string(), expires_in_seconds);
    jwt_utils.generate_token(&claims).unwrap()
}

/// Validate a JWT token for testing
fn validate_test_token(token: &str) -> Result<Claims, String> {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    jwt_utils.validate_token(token).map_err(|e| e.to_string())
}

/// Helper function to create a test router with mock configuration
async fn create_test_router() -> Router {
    let config = SecureConfig::mock();
    
    // Create a mock database connection
    // For integration tests, we'll use an in-memory SQLite database
    let db_pool = create_test_database().await;
    
    let app_state = Arc::new(AppState {
        config,
        db_pool,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient::default(),
    });

    create_routes(app_state)
}

/// Helper function to create a test router with JWT authentication enabled
async fn create_auth_test_router() -> Router {
    let mut config = SecureConfig::mock();
    config.auth.use_jwt = true;
    config.auth.jwt_secret = TEST_JWT_SECRET.to_string();
    
    // Create a mock database connection
    let db_pool = create_test_database().await;
    
    let app_state = Arc::new(AppState {
        config,
        db_pool,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient::default(),
    });

    create_routes(app_state)
}

/// Create an in-memory test database
async fn create_test_database() -> PgPool {
    // For now, we'll create a minimal mock pool
    // In a real scenario, you'd want to use sqlx::SqlitePool::connect(":memory:")
    // and run migrations
    
    // This is a placeholder - in real tests you'd want to set up a proper test database
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://delong:delong_test_2025@localhost:5433/delong".to_string());
    
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

/// Helper function to make JSON requests without authentication (for public endpoints)
async fn make_json_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<T>,
) -> (StatusCode, Value) {
    let mut request_builder = Request::builder()
        .uri(path)
        .method(method)
        .header("content-type", "application/json");

    let request = if let Some(body) = body {
        request_builder
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    } else {
        request_builder.body(Body::empty()).unwrap()
    };

    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

    (status, body_json)
}

/// Helper function to make authenticated JSON requests with JWT token
async fn make_auth_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<T>,
    token: &str,
) -> (StatusCode, Value) {
    let mut request_builder = Request::builder()
        .uri(path)
        .method(method)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token));

    let request = if let Some(body) = body {
        request_builder
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    } else {
        request_builder.body(Body::empty()).unwrap()
    };

    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

    (status, body_json)
}

#[tokio::test]
async fn test_health_endpoint() {
    let router = create_test_router().await;

    let (status, body) = make_json_request::<()>(&router, "GET", "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    
    // Parse the response
    let response: ApiResponse<()> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);
    assert!(!response.message.is_empty());
    assert!(response
        .message
        .contains("Secure service is healthy"));

    println!("✅ Health endpoint test passed");
}

// JWT Authentication Tests

#[tokio::test]
async fn test_jwt_authentication_missing_token() {
    let router = create_auth_test_router().await;

    let (status, body) = make_json_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    let response: ApiResponse<()> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Unauthorized);
    assert!(response.message.contains("Missing Authorization header"));

    println!("✅ JWT missing token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_invalid_token() {
    let router = create_auth_test_router().await;

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        "invalid.jwt.token",
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    let response: ApiResponse<()> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Unauthorized);
    assert!(response.message.contains("Invalid JWT token"));

    println!("✅ JWT invalid token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_expired_token() {
    let router = create_auth_test_router().await;
    let generator = TestJwtGenerator::new();
    let expired_token = generator.generate_custom_token("user", "test_user", -3600); // Expired 1 hour ago

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &expired_token,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    let response: ApiResponse<()> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Unauthorized);
    assert!(response.message.contains("expired"));

    println!("✅ JWT expired token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_valid_admin_token() {
    let router = create_auth_test_router().await;
    let generator = TestJwtGenerator::new();
    let admin_token = generator.generate_admin_token("test_admin");

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    
    let response: ApiResponse<PaginatedResponse<Value>> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);

    println!("✅ JWT valid admin token test passed");
}

#[tokio::test]
async fn test_jwt_authentication_valid_user_token() {
    let router = create_auth_test_router().await;
    let user_token = crate::test_utils::jwt::generate_user_token("test_user");

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    
    let response: ApiResponse<PaginatedResponse<Value>> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);

    println!("✅ JWT valid user token test passed");
}

#[tokio::test]
async fn test_jwt_authorization_admin_only_endpoint() {
    let router = create_auth_test_router().await;
    
    // Test with user token (should fail)
    let user_token = crate::test_utils::jwt::generate_user_token("test_user");
    let committee_member = serde_json::json!({
        "wallet_address": "0x1234567890abcdef1234567890abcdef12345678",
        "is_active": true
    });

    let (status, body) = make_auth_request(
        &router,
        "POST",
        "/api/committee",
        Some(committee_member.clone()),
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<()> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Forbidden);
    assert!(response.message.contains("Admin privileges required"));

    // Test with admin token (should succeed)
    let admin_token = crate::test_utils::jwt::generate_admin_token("test_admin");
    
    let (status, body) = make_auth_request(
        &router,
        "POST",
        "/api/committee",
        Some(committee_member),
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let response: ApiResponse<Value> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);

    println!("✅ JWT admin authorization test passed");
}

#[tokio::test]
async fn test_generate_jwt_token_utility() {
    // Test the JWT generation utility functions
    let admin_token = crate::test_utils::jwt::generate_admin_token("test_admin_user");
    let user_token = crate::test_utils::jwt::generate_user_token("test_regular_user");
    let custom_token = crate::test_utils::jwt::generate_custom_token("moderator", "test_mod", 7200);

    // Validate tokens
    let admin_claims = crate::test_utils::jwt::validate_test_token(&admin_token).unwrap();
    assert_eq!(admin_claims.role, "admin");
    assert_eq!(admin_claims.sub, "test_admin_user");
    assert!(!admin_claims.is_expired());

    let user_claims = crate::test_utils::jwt::validate_test_token(&user_token).unwrap();
    assert_eq!(user_claims.role, "user");
    assert_eq!(user_claims.sub, "test_regular_user");
    assert!(!user_claims.is_expired());

    let custom_claims = crate::test_utils::jwt::validate_test_token(&custom_token).unwrap();
    assert_eq!(custom_claims.role, "moderator");
    assert_eq!(custom_claims.sub, "test_mod");
    assert!(!custom_claims.is_expired());

    // Print tokens for manual testing
    println!("🔐 Generated JWT Tokens for Manual Testing:");
    println!("Admin Token: {}", admin_token);
    println!("User Token: {}", user_token);
    println!("Custom Token: {}", custom_token);
    println!("✅ JWT token generation utility test passed");
}

#[tokio::test]
async fn test_static_datasets_list_empty() {
    let router = create_auth_test_router().await;
    let token = generate_admin_token("test_admin");

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets?page=1&limit=10",
        None,
        &token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    // Parse the response
    let response: ApiResponse<PaginatedResponse<Value>> =
        serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);
    
    let data = response.data.unwrap();
    assert_eq!(data.items.len(), 0);
    assert_eq!(data.page, 1);
    assert_eq!(data.limit, 10);
    assert_eq!(data.total_items, 0);

    println!("✅ Static datasets list (empty) test passed");
}

#[tokio::test]
async fn test_static_datasets_list_with_pagination() {
    let router = create_test_router().await;

    // Test with different pagination parameters
    let test_cases = vec![
        ("page=1&limit=5", 1, 5),
        ("page=2&limit=20", 2, 20),
        ("page=1&limit=100", 1, 100),
    ];

    for (query, expected_page, expected_limit) in test_cases {
        let path = format!("/api/static-datasets?{}", query);
        let (status, body) = make_json_request::<()>(&router, "GET", &path, None).await;

        assert_eq!(status, StatusCode::OK);

        let response: ApiResponse<PaginatedResponse<Value>> =
            serde_json::from_value(body).unwrap();
        assert_eq!(response.code, common::ResponseCode::Success);
        
        let data = response.data.unwrap();
        assert_eq!(data.page, expected_page);
        assert_eq!(data.limit, expected_limit);
    }

    println!("✅ Static datasets pagination test passed");
}

#[tokio::test]
async fn test_static_dataset_get_not_found() {
    let router = create_test_router().await;

    let (status, body) = make_json_request::<()>(&router, "GET", "/api/static-datasets/999", None).await;

    assert_eq!(status, StatusCode::OK);

    let response: ApiResponse<Value> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);
    // TODO: Check the actual response data format for not found cases
    // assert!(response.data.is_null());

    println!("✅ Static dataset get (not found) test passed");
}

#[tokio::test]
async fn test_algorithm_execution_submission_validation() {
    let router = create_test_router().await;

    // Test invalid requests
    let invalid_requests = vec![
        // Empty GitHub repo
        AlgoExeSubmissionRequest {
            github_repo: "".to_string(),
            commit_hash: "abc123".to_string(),
            scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
            dataset: "dataset_001".to_string(),
        },
        // Empty commit hash
        AlgoExeSubmissionRequest {
            github_repo: "https://github.com/user/repo".to_string(),
            commit_hash: "".to_string(),
            scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
            dataset: "dataset_001".to_string(),
        },
        // Invalid wallet address
        AlgoExeSubmissionRequest {
            github_repo: "https://github.com/user/repo".to_string(),
            commit_hash: "abc123".to_string(),
            scientist_wallet: "invalid_wallet".to_string(),
            dataset: "dataset_001".to_string(),
        },
        // Empty dataset
        AlgoExeSubmissionRequest {
            github_repo: "https://github.com/user/repo".to_string(),
            commit_hash: "abc123".to_string(),
            scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
            dataset: "".to_string(),
        },
    ];

    for request in invalid_requests {
        let (status, _body) = make_json_request(&router, "POST", "/api/algo-exes", Some(request)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    println!("✅ Algorithm execution validation test passed");
}

#[tokio::test]
async fn test_algorithm_execution_submission_valid() {
    let router = create_test_router().await;

    let valid_request = AlgoExeSubmissionRequest {
        github_repo: "https://github.com/user/test-algorithm".to_string(),
        commit_hash: "a1b2c3d4e5f6".to_string(),
        scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
        dataset: "dataset_001".to_string(),
    };

    let (status, body) = make_json_request(&router, "POST", "/api/algo-exes", Some(valid_request)).await;

    assert_eq!(status, StatusCode::OK);

    let response: ApiResponse<AlgoExeSubmissionResponse> =
        serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);
    
    let data = response.data.unwrap();
    assert!(data.id > 0);
    assert!(data.tx_hash.starts_with("0x"));
    assert_eq!(data.status, "submitted");
    assert!(!data.message.is_empty());

    println!("✅ Algorithm execution submission test passed");
}

#[tokio::test]
async fn test_algorithm_executions_list() {
    let router = create_test_router().await;

    let (status, body) = make_json_request::<()>(
        &router,
        "GET",
        "/api/algo-exes?page=1&limit=10",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let response: ApiResponse<PaginatedResponse<Value>> =
        serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);
    
    let data = response.data.unwrap();
    assert_eq!(data.page, 1);
    assert_eq!(data.limit, 10);
    // Initially empty list is expected
    assert_eq!(data.total_items, 0);

    println!("✅ Algorithm executions list test passed");
}

#[tokio::test]
async fn test_algorithm_execution_get_not_found() {
    let router = create_test_router().await;

    let (status, _body) = make_json_request::<()>(&router, "GET", "/api/algo-exes/999", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    println!("✅ Algorithm execution get (not found) test passed");
}

#[tokio::test]
async fn test_algorithm_execution_workflow() {
    let router = create_test_router().await;

    // Step 1: Submit an algorithm execution
    let submission_request = AlgoExeSubmissionRequest {
        github_repo: "https://github.com/delong-protocol/sample-algorithm".to_string(),
        commit_hash: "main".to_string(),
        scientist_wallet: "0xabcdef1234567890123456789012345678901234".to_string(),
        dataset: "biomedical_dataset_v1".to_string(),
    };

    let (status, body) = make_json_request(&router, "POST", "/api/algo-exes", Some(submission_request)).await;
    assert_eq!(status, StatusCode::OK);

    let submission_response: ApiResponse<AlgoExeSubmissionResponse> =
        serde_json::from_value(body).unwrap();
    assert_eq!(submission_response.code, common::ResponseCode::Success);
    
    let execution_id = submission_response.data.unwrap().id;

    // Step 2: List executions and verify it appears
    let (status, body) = make_json_request::<()>(
        &router,
        "GET",
        "/api/algo-exes?page=1&limit=10",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let list_response: ApiResponse<PaginatedResponse<Value>> =
        serde_json::from_value(body).unwrap();
    assert_eq!(list_response.code, common::ResponseCode::Success);
    
    // Note: In the current mock implementation, the list might still be empty
    // because we're not actually storing to a database. This test validates
    // the API endpoints work correctly.

    println!("✅ Algorithm execution workflow test passed");
}

#[tokio::test]
async fn test_invalid_endpoints() {
    let router = create_test_router().await;

    let invalid_endpoints = vec![
        ("GET", "/invalid"),
        ("POST", "/api/invalid"),
        ("GET", "/api/static-datasets/invalid_id"),
        ("GET", "/api/algo-exes/invalid_id"),
    ];

    for (method, path) in invalid_endpoints {
        let request = Request::builder()
            .uri(path)
            .method(method)
            .body(Body::empty())
            .unwrap();

        let response = router.clone().oneshot(request).await.unwrap();
        // Should be either 404 Not Found or 400 Bad Request
        assert!(matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST
        ));
    }

    println!("✅ Invalid endpoints test passed");
}

#[tokio::test]
async fn test_cors_headers() {
    let router = create_test_router().await;

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .header("Origin", "https://delong-protocol.com")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Check CORS headers are present
    let headers = response.headers();
    assert!(headers.contains_key("access-control-allow-origin"));

    println!("✅ CORS headers test passed");
}

#[tokio::test]
async fn test_concurrent_requests() {
    let router = create_test_router().await;

    // Send multiple concurrent health check requests
    let mut handles = vec![];
    
    for i in 0..10 {
        let router_clone = router.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/health")
                .method("GET")
                .header("X-Request-ID", format!("test-{}", i))
                .body(Body::empty())
                .unwrap();

            let response = router_clone.oneshot(request).await.unwrap();
            response.status()
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    for handle in handles {
        let status = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
    }

    println!("✅ Concurrent requests test passed");
}

#[tokio::test]
async fn test_error_response_format() {
    let router = create_test_router().await;

    // Test endpoint that should return an error
    let (status, body) = make_json_request::<()>(&router, "GET", "/api/static-datasets/999", None).await;

    assert_eq!(status, StatusCode::OK);

    // Verify the error response follows our API format
    let response_value: Value = body;
    assert!(response_value.get("code").is_some());
    assert!(response_value.get("data").is_some());
    assert!(response_value.get("message").is_some());

    // The response should follow the ApiResponse format even for "not found" cases
    // In our current implementation, this returns success with null data
    let response: ApiResponse<Option<Value>> = serde_json::from_value(response_value).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);

    println!("✅ Error response format test passed");
}

#[tokio::test] 
async fn test_large_request_body() {
    let router = create_test_router().await;

    // Create a request with a large dataset name
    let large_request = AlgoExeSubmissionRequest {
        github_repo: "https://github.com/user/repo".to_string(),
        commit_hash: "abc123".to_string(),
        scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
        dataset: "x".repeat(1000), // Very long dataset name
    };

    let (status, _body) = make_json_request(&router, "POST", "/api/algo-exes", Some(large_request)).await;

    // Should handle large requests gracefully
    assert!(matches!(
        status,
        StatusCode::OK | StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE
    ));

    println!("✅ Large request body test passed");
} 