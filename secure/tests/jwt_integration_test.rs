//! JWT Integration Tests for Secure Service
//!
//! This module contains tests specifically for JWT authentication and authorization
//! functionality in the Secure Service.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use common::{ApiResponse, Claims, JwtUtils};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use secure::{config::SecureConfig, routes::create_routes, AppState};

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

/// Create a test router with JWT authentication enabled
async fn create_auth_test_router() -> Router {
    let mut config = SecureConfig::mock();
    config.auth.use_jwt = true;
    config.auth.jwt_secret = TEST_JWT_SECRET.to_string();
    
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://delong:delong_test_2025@localhost:5433/delong".to_string());
    
    let db_pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");
    
    let app_state = Arc::new(AppState {
        config,
        db_pool,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient::default(),
    });

    create_routes(app_state)
}

/// Helper function to make authenticated JSON requests with JWT token
async fn make_auth_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<T>,
    token: &str,
) -> (StatusCode, Value) {
    let request_builder = Request::builder()
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

/// Helper function to make requests without authentication
async fn make_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<T>,
) -> (StatusCode, Value) {
    let request_builder = Request::builder()
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

#[tokio::test]
async fn test_jwt_missing_token() {
    let router = create_auth_test_router().await;

    let (status, body) = make_request::<()>(&router, "GET", "/api/static-datasets", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    
    let response: ApiResponse<()> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Unauthorized);
    assert!(response.message.contains("Missing Authorization header"));

    println!("✅ JWT missing token test passed");
}

#[tokio::test]
async fn test_jwt_invalid_token() {
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
async fn test_jwt_expired_token() {
    let router = create_auth_test_router().await;
    let expired_token = generate_custom_token("user", "test_user", -3600); // Expired 1 hour ago

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
    
    // Print the actual message to debug
    println!("Actual error message: '{}'", response.message);
    
    // Check for either "expired" or "Invalid JWT token" (expired tokens are also invalid)
    assert!(
        response.message.contains("expired") || response.message.contains("Invalid JWT token"),
        "Expected error message to contain 'expired' or 'Invalid JWT token', got: '{}'", 
        response.message
    );

    println!("✅ JWT expired token test passed");
}

#[tokio::test]
async fn test_jwt_valid_admin_token() {
    let router = create_auth_test_router().await;
    let admin_token = generate_admin_token("test_admin");

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &admin_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    
    let response: ApiResponse<Value> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);

    println!("✅ JWT valid admin token test passed");
}

#[tokio::test]
async fn test_jwt_valid_user_token() {
    let router = create_auth_test_router().await;
    let user_token = generate_user_token("test_user");

    let (status, body) = make_auth_request::<()>(
        &router,
        "GET",
        "/api/static-datasets",
        None,
        &user_token,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    
    let response: ApiResponse<Value> = serde_json::from_value(body).unwrap();
    assert_eq!(response.code, common::ResponseCode::Success);

    println!("✅ JWT valid user token test passed");
}

#[tokio::test]
async fn test_jwt_admin_authorization() {
    let router = create_auth_test_router().await;
    
    // Test with user token (should fail)
    let user_token = generate_user_token("test_user");
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
    let admin_token = generate_admin_token("test_admin");
    
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
async fn test_print_jwt_tokens_for_manual_testing() {
    let admin_token = generate_admin_token("test_admin_user");
    let user_token = generate_user_token("test_regular_user");
    let moderator_token = generate_custom_token("moderator", "test_moderator", 7200);

    println!("\n🔐 JWT Tokens for Manual Testing:");
    println!("{}", "=".repeat(60));
    println!("\n📋 Admin Token (role: admin, user: test_admin_user):");
    println!("{}\n", admin_token);
    
    println!("📋 User Token (role: user, user: test_regular_user):");
    println!("{}\n", user_token);
    
    println!("📋 Moderator Token (role: moderator, user: test_moderator):");
    println!("{}\n", moderator_token);

    println!("🔧 Usage Examples:");
    println!("{}", "=".repeat(60));
    println!("# Test health endpoint (no auth required):");
    println!("curl http://localhost:8082/health\n");
    
    println!("# Test authenticated endpoint with admin token:");
    println!("curl -H \"Authorization: Bearer {}\" \\", admin_token);
    println!("     http://localhost:8082/api/static-datasets\n");
    
    println!("# Test admin-only endpoint (add committee member):");
    println!("curl -X POST \\");
    println!("     -H \"Authorization: Bearer {}\" \\", admin_token);
    println!("     -H \"Content-Type: application/json\" \\");
    println!("     -d '{{\"wallet_address\":\"0x1234567890abcdef1234567890abcdef12345678\",\"is_active\":true}}' \\");
    println!("     http://localhost:8082/api/committee\n");
    
    println!("# Test with user token (should be forbidden for admin endpoints):");
    println!("curl -X POST \\");
    println!("     -H \"Authorization: Bearer {}\" \\", user_token);
    println!("     -H \"Content-Type: application/json\" \\");
    println!("     -d '{{\"wallet_address\":\"0xabcdef1234567890abcdef1234567890abcdef12\",\"is_active\":true}}' \\");
    println!("     http://localhost:8082/api/committee\n");
    
    println!("💡 Tips:");
    println!("- Tokens expire in 1 hour (3600 seconds)");
    println!("- Use admin token for all endpoints including admin-only operations");
    println!("- Use user token to test authorization (should get 403 for admin endpoints)");
    println!("- Health endpoint (/health) doesn't require authentication");
    println!("- All /api/* endpoints require JWT authentication");
    println!("{}", "=".repeat(60));

    println!("✅ JWT token generation for manual testing completed");
} 