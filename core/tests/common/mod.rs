//! Test helper functions and utilities for integration tests
//!
//! This module provides common test setup functionality to ensure
//! consistent test environments across all test suites.

use axum::Router;
use delong_core::{
    config::Config,
    infra::{ai_audit::AiAuditService, verification::VerificationStore},
    routes::{AppState, create_router},
    utils::jwt::create_jwt_config,
};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::info;

/// Initialize tracing for tests (only once)
fn init_test_tracing() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("core=info,tower_http=debug")
            .with_test_writer()
            .try_init();
    });
}

/// Set up a test database connection
///
/// This function creates a database connection for testing purposes
pub async fn setup_test_db() -> PgPool {
    // Initialize test environment - load from core/.env
    if let Err(e) = dotenvy::from_filename(".env") {
        eprintln!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:test_password@localhost:11021/db_core_test".to_string()
    });

    // Create database pool
    let db = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Run any pending migrations
    info!("Database connected for tests");

    db
}

/// Set up a test configuration
///
/// This function loads configuration from environment for testing
pub fn setup_test_config() -> Config {
    // Initialize test environment - load from core/.env
    if let Err(e) = dotenvy::from_filename(".env") {
        eprintln!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Initialize and return configuration
    Config::load().expect("Failed to load config")
}

/// Create a test Redis client
///
/// This function creates a Redis client for testing
#[allow(dead_code)]
pub fn setup_test_redis() -> redis::Client {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:11022".to_string());

    redis::Client::open(redis_url).expect("Failed to create Redis client")
}

/// Set up a test application with all routes and dependencies
///
/// This function creates a complete test application instance
#[allow(dead_code)]
pub async fn setup_test_app() -> Router {
    // Initialize tracing
    init_test_tracing();

    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        eprintln!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Load configuration
    let config = setup_test_config();
    let config = Arc::new(config);

    // Initialize database
    let db = setup_test_db().await;

    // Initialize verification store
    let verification_store = Arc::new(VerificationStore::new());

    // Initialize JWT config
    let jwt_config = Arc::new(create_jwt_config());

    // Initialize AI audit service (use service URL from config)
    let ai_audit_service = AiAuditService::new(None).expect("Failed to create AI audit service");
    let ai_audit_service = Arc::new(ai_audit_service);

    // Create application state
    let state = AppState {
        db,
        verification_store,
        jwt_config,
        config,
        ai_audit_service,
        proxy_client: None,
    };

    // Create and return the router
    create_router(state)
}

/// Set up a clean test application (clears test data before setup)
///
/// This function creates a test application with a fresh database state
pub async fn setup_clean_test_app() -> Router {
    // Initialize tracing
    init_test_tracing();

    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        eprintln!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Load configuration
    let _config = setup_test_config();

    // Initialize database
    let db = setup_test_db().await;

    // Clear test data - only for test tables
    // Be careful not to truncate system tables
    let _ = sqlx::query(
        "TRUNCATE TABLE
         ai_audit_reports,
         api_keys,
         user_sessions,
         verification_codes
         CASCADE",
    )
    .execute(&db)
    .await;
    // Load configuration
    let config = setup_test_config();
    let config = Arc::new(config);

    // Initialize database
    let db = setup_test_db().await;

    // Initialize verification store
    let verification_store = Arc::new(VerificationStore::new());

    // Initialize JWT config
    let jwt_config = Arc::new(create_jwt_config());

    // Initialize AI audit service (use service URL from config)
    let ai_audit_service = AiAuditService::new(None).expect("Failed to create AI audit service");
    let ai_audit_service = Arc::new(ai_audit_service);

    // Create application state
    let state = AppState {
        db,
        verification_store,
        jwt_config,
        config,
        ai_audit_service,
        proxy_client: None,
    };

    // Create and return the router
    create_router(state)
}

/// Extract JSON body from an axum response
///
/// Helper function to extract and parse JSON body from test responses
pub async fn extract_json_body(response: axum::response::Response) -> serde_json::Value {
    use axum::body::to_bytes;

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    serde_json::from_slice(&body).expect("Failed to parse JSON response")
}

/// Create a JSON request for testing
///
/// Helper function to create JSON requests for testing
pub fn json_request<T: serde::Serialize>(
    method: &str,
    uri: &str,
    body: T,
) -> axum::http::Request<axum::body::Body> {
    use axum::body::Body;
    use axum::http::{Request, header};

    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Create an authenticated request with JWT token
///
/// Helper function to create authenticated requests for testing
pub fn authenticated_request<T: serde::Serialize>(
    method: &str,
    uri: &str,
    body: T,
    token: &str,
) -> axum::http::Request<axum::body::Body> {
    use axum::body::Body;
    use axum::http::{Request, header};

    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Generate a test email address
///
/// Creates a unique email address for testing
pub fn generate_test_email(prefix: &str) -> String {
    use uuid::Uuid;
    format!("{}+{}@test.com", prefix, Uuid::new_v4())
}

/// Generate a test username
///
/// Creates a unique username for testing
pub fn generate_test_username(prefix: &str) -> String {
    use uuid::Uuid;
    let uuid = Uuid::new_v4().to_string();
    let suffix = &uuid[..8];
    format!("{}_{}", prefix, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_test_db() {
        let db = setup_test_db().await;
        assert!(db.acquire().await.is_ok());
    }

    #[test]
    fn test_setup_test_config() {
        let config = setup_test_config();
        assert!(!config.jwt_secret.is_empty());
    }

    #[test]
    fn test_generate_test_email() {
        let email = generate_test_email("user");
        assert!(email.starts_with("user+"));
        assert!(email.ends_with("@test.com"));
    }

    #[test]
    fn test_generate_test_username() {
        let username = generate_test_username("user");
        assert!(username.starts_with("user_"));
        assert!(username.len() > 5);
    }
}
