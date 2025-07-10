
//! Common test utilities for Core Service integration tests

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use delong_core::{config::AppConfig, create_app, AppState};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use common::models::auth::{Claims, JwtUtils};
use sqlx::PgPool;

/// JWT secret for testing (must match core service config)
const TEST_JWT_SECRET: &str = "a_secure_secret_for_jwt_that_is_long_enough"; // Make sure this matches your config

/// Create a test database connection pool
pub async fn create_test_pool() -> anyhow::Result<PgPool> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://delong:delong_test_2025@localhost:5433/delong".to_string());

    PgPool::connect(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to test database: {}", e))
}

/// Helper function to create a test router with mock configuration
pub async fn create_test_router() -> (Router, Arc<AppState>) {
    // Simplified AppState creation for testing
    let mut config = AppConfig::from_env().expect("Failed to load config for test");
    
    // Override database URL for testing
    config.database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://delong:delong_test_2025@localhost:5433/delong".to_string());
        
    config.jwt_secret = TEST_JWT_SECRET.to_string();

    let state = Arc::new(AppState::new(config).await.unwrap());
    
    (create_app(state.clone()), state)
}


/// Generic request helper
pub async fn make_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    uri: &str,
    body: Option<T>,
    token: Option<&str>,
) -> (StatusCode, String, Value) {
    let mut request_builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Content-Type", "application/json");

    if let Some(token) = token {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    }

    let body = match body {
        Some(b) => Body::from(serde_json::to_vec(&b).unwrap()),
        None => Body::empty(),
    };

    let request = request_builder.body(body).unwrap();

    let response = router.clone().oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_string = String::from_utf8_lossy(&body_bytes).to_string();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

    (status, body_string, body_json)
}

/// Helper function to make authenticated JSON requests with JWT token
pub async fn make_auth_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<T>,
    token: &str,
) -> (StatusCode, String, Value) {
    make_request(router, method, path, body, Some(token)).await
}

/// Generate an admin JWT token for testing
pub fn generate_admin_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new("admin".to_string(), user_id.to_string(), 3600); // 1 hour expiration
    jwt_utils.generate_token(&claims).unwrap()
}

/// Generate a user JWT token for testing
pub fn generate_user_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new("user".to_string(), user_id.to_string(), 3600); // 1 hour expiration
    jwt_utils.generate_token(&claims).unwrap()
}

/// Generate a custom JWT token for testing
pub fn generate_custom_token(role: &str, user_id: &str, expires_in_seconds: i64) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new(role.to_string(), user_id.to_string(), expires_in_seconds);
    jwt_utils.generate_token(&claims).unwrap()
} 