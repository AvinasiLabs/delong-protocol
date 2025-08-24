//! Test helper functions and utilities for integration tests
//!
//! This module provides common test setup functionality to ensure
//! consistent test environments across all test suites.

use axum::Router;
use deadpool_redis::{Config as RedisConfig, Runtime};
use delong_core::{
    config::Config,
    infra::{
        ai_audit::AiAuditService, db::Database, google_oauth::GoogleOAuthService,
        proxy::ProxyClient, verification::VerificationStore,
    },
    routes::{AppState, create_router},
    utils::jwt::create_jwt_config,
};
use sqlx::migrate::MigrateDatabase;
use std::sync::Arc;

// Removed init_test_tracing - it may cause tests to hang
// Tracing is not necessary for integration tests

/// Set up a test database connection
///
/// This function creates a database connection for testing purposes
pub async fn setup_test_db(config: Arc<Config>) -> Database {
    // Set TEST_MODE to help with testing
    unsafe {
        std::env::set_var("TEST_MODE", "true");
    }

    println!("Connecting to test database: {}", config.database.url);

    // Ensure database exists
    if !sqlx::Postgres::database_exists(&config.database.url)
        .await
        .unwrap_or(false)
    {
        println!("Creating test database...");
        sqlx::Postgres::create_database(&config.database.url)
            .await
            .expect("Failed to create test database");
    }

    // Create database using the Database struct
    let db = Database::new(&config.database)
        .await
        .expect("Failed to connect to database");

    println!("Database connected, running migrations...");

    // Run migrations
    db.run_migrations()
        .await
        .expect("Failed to run database migrations");

    println!("Database setup complete for tests");

    db
}

/// Set up a test Redis pool
pub async fn setup_test_redis_pool() -> Arc<deadpool_redis::Pool> {
    // Use test Redis configuration
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:11022".to_string());

    let redis_config = RedisConfig::from_url(redis_url);
    let redis_pool = redis_config
        .create_pool(Some(Runtime::Tokio1))
        .expect("Failed to create Redis pool");

    Arc::new(redis_pool)
}

/// Set up a test application with all routes and dependencies
///
/// This function creates a complete test application instance
#[allow(dead_code)]
pub async fn setup_test_app() -> Router {
    // Load configuration
    let config = Arc::new(Config::load().unwrap());

    // Initialize database
    let db = setup_test_db(config.clone()).await;

    // Initialize Redis pool for verification store
    let redis_pool = setup_test_redis_pool().await;
    let verification_store = Arc::new(VerificationStore::new(redis_pool));

    // Initialize JWT config
    let jwt_config = Arc::new(create_jwt_config());

    // Initialize AI audit service (use service URL from config)
    let ai_audit_service = AiAuditService::new(None).expect("Failed to create AI audit service");
    let ai_audit_service = Arc::new(ai_audit_service);

    // Create mock Google OAuth service for testing
    let google_oauth_service = Arc::new(GoogleOAuthService::new(
        "test-client-id".to_string(),
        "test-client-secret".to_string(),
        "http://localhost:3000/auth/google/callback".to_string(),
    ));

    // Create application state
    let state = AppState {
        db: db.into(),
        verification_store,
        jwt_config,
        config,
        ai_audit_service,
        proxy_client: None,
        google_oauth_service,
    };

    // Create and return the router
    create_router(state)
}

/// Set up a clean test application (clears test data before setup)
///
/// This function creates a test application with a fresh database state
pub async fn setup_clean_test_app() -> Router {
    println!("Loading configuration for clean test app...");
    // Load configuration once
    let config = Arc::new(Config::load().unwrap());

    println!("Configuration loaded successfully");

    // Initialize database
    let db = setup_test_db(config.clone()).await;
    println!("Test database setup complete");

    // Clear test data - truncate all user-generated data tables
    // Keep system tables like roles, permissions, oauth_providers intact
    let _ = sqlx::query(
        "TRUNCATE TABLE
         users,
         audit_logs,
         committee_wallets,
         ai_audit_reports,
         api_keys,
         user_oauth_accounts
         CASCADE",
    )
    .execute(db.pool())
    .await;
    println!("Test tables truncated successfully");

    // Initialize Redis pool for verification store
    let redis_pool = setup_test_redis_pool().await;
    let verification_store = Arc::new(VerificationStore::new(redis_pool));
    println!("Verification store initialized");

    // Initialize JWT config
    let jwt_config = Arc::new(create_jwt_config());
    println!("JWT config created");

    // Initialize AI audit service (use service URL from config)
    let ai_audit_service = AiAuditService::new(None).expect("Failed to create AI audit service");
    let ai_audit_service = Arc::new(ai_audit_service);
    println!("AI audit service created");

    // Create mock Google OAuth service for testing
    let google_oauth_service = Arc::new(GoogleOAuthService::new(
        "test-client-id".to_string(),
        "test-client-secret".to_string(),
        "http://localhost:3000/auth/google/callback".to_string(),
    ));
    println!("Google OAuth service created");

    // Create proxy client for testing
    println!(
        "SECURE_SERVICE_URL from config: '{}'",
        config.proxy.secure_service_url
    );
    println!(
        "Is URL empty? {}",
        config.proxy.secure_service_url.is_empty()
    );

    let proxy_client = if !config.proxy.secure_service_url.is_empty() {
        match ProxyClient::new(config.proxy.clone(), &config.internal_jwt_secret) {
            Ok(client) => {
                println!("✓ Proxy client created successfully for testing");
                let client_arc = Arc::new(client);
                println!("✓ Proxy client wrapped in Arc");
                Some(client_arc)
            }
            Err(e) => {
                println!("✗ Failed to create proxy client: {}, using None", e);
                None
            }
        }
    } else {
        println!("✗ Proxy service URL not configured (empty), using None");
        None
    };

    println!("Final proxy_client is Some? {}", proxy_client.is_some());

    // Create application state
    let state = AppState {
        db: db.into(),
        verification_store,
        jwt_config,
        config,
        ai_audit_service,
        proxy_client: proxy_client.clone(),
        google_oauth_service,
    };
    println!(
        "Application state created with proxy_client: {}",
        if proxy_client.is_some() {
            "Some"
        } else {
            "None"
        }
    );

    // Create and return the router
    println!(
        "Creating router with state that has proxy_client: {}",
        if state.proxy_client.is_some() {
            "Some"
        } else {
            "None"
        }
    );
    let router = create_router(state);
    println!("Router created successfully");
    router
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
        let config = Arc::new(Config::load().unwrap());
        let db = setup_test_db(config.clone()).await;

        // Test connection with a simple query
        let result = sqlx::query!("SELECT 1 as value").fetch_one(db.pool()).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, Some(1));

        // Test that we can acquire a connection
        assert!(db.pool().acquire().await.is_ok());
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
