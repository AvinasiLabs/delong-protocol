//! Test helpers for API integration tests
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use common::{
    models::auth::{Claims, JwtUtils},
    prelude::{ApiResponse, UserSession},
};
use jsonwebtoken::{encode, Header};
use serde_json::{json, Value};
use secure::{
    config::AppConfig,
    create_router,
    services::blockchain_sync::BlockchainSyncService,
    utils::jwt_utils::JWT_ALGORITHM,
    AppState,
};

use crate::common::test_utils::{create_test_pool, db::cleanup_test_database};

/// JWT secret for testing (must match secure service config)
const TEST_JWT_SECRET: &str = "test_secret";

/// Generate an admin JWT token for testing
pub fn generate_admin_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new("admin".to_string(), user_id.to_string(), 3600);
    jwt_utils.generate_token(&claims).unwrap()
}

/// Generate a user JWT token for testing
pub fn generate_user_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new("user".to_string(), user_id.to_string(), 3600);
    jwt_utils.generate_token(&claims).unwrap()
}

/// Generate a custom JWT token for testing
pub fn generate_custom_token(role: &str, user_id: &str, expires_in_seconds: i64) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET.to_string());
    let claims = Claims::new(role.to_string(), user_id.to_string(), expires_in_seconds);
    jwt_utils.generate_token(&claims).unwrap()
}

/// Helper function to create a test router with mock configuration
pub async fn create_test_router() -> Router {
    let config = AppConfig::mock();
    let db_pool = create_test_pool().await.unwrap();
    let blockchain_sync_service =
        Arc::new(BlockchainSyncService::new_for_test().await.unwrap());

    create_router(db_pool, config, blockchain_sync_service)
}

/// Helper function to create a test router with JWT authentication enabled
pub async fn create_auth_test_router() -> Router {
    let mut config = AppConfig::mock();
    config.auth.use_jwt = true;
    config.auth.jwt_secret = TEST_JWT_SECRET.to_string();

    let db_pool = create_test_pool().await.unwrap();
    let blockchain_sync_service =
        Arc::new(BlockchainSyncService::new_for_test().await.unwrap());

    create_router(db_pool, config, blockchain_sync_service)
}

/// Helper function to make JSON requests without authentication (for public endpoints)
pub async fn make_json_request<T: serde::Serialize>(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<T>,
) -> (StatusCode, String, Value) {
    make_request(router, method, path, body, None).await
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

/// Generic request helper
async fn make_request<T: serde::Serialize>(
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

pub async fn setup_test_environment() -> Router {
    let router = create_auth_test_router().await;
    let pool = create_test_pool().await.unwrap();
    cleanup_test_database(&pool).await.unwrap();
    router
}

/// Generate a JWT token for a regular user
pub fn generate_user_token(user_id: &str) -> String {
    let jwt_utils = JwtUtils::new(TEST_JWT_SECRET);
    let claims = Claims::new("user".to_string(), user_id.to_string(), 3600); // 1 hour expiration
    jwt_utils.generate_token(&claims).unwrap()
}
