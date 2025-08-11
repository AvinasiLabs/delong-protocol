//! Test helper functions and utilities for integration tests
//!
//! This module provides common test setup functionality to ensure
//! consistent test environments across all test suites.

use axum::{body::Body, http::Response, Router};
use ipfs_api_backend_hyper::TryFromUri;
use secure::{config::Config, infra::db::Database, routes};
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn};

/// Set up a test database connection
///
/// This function creates a database connection for testing purposes
pub async fn setup_test_db() -> Database {
    // Initialize test environment - load from secure/.env
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Initialize configuration
    let config = Config::from_env().expect("Failed to load config");

    // Initialize database
    let db = Database::new(&config.database)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    info!("Running database migrations for tests");
    db.run_migrations()
        .await
        .expect("Failed to run database migrations");

    db
}

/// Set up a test configuration
///
/// This function loads configuration from environment for testing
pub fn setup_test_config() -> Config {
    // Initialize test environment - load from secure/.env
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Initialize and return configuration
    Config::from_env().expect("Failed to load config")
}

/// Set up a test application with all required dependencies
///
/// This function creates a complete test application with database,
/// configuration, and all routes configured
pub async fn setup_test_app() -> Router {
    // Initialize logging for tests
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("secure=debug".parse().unwrap()),
        )
        .try_init();

    let db = setup_test_db().await;
    let config = setup_test_config();

    routes::create_app(db, config).await
}

/// Set up a test application with a clean database
///
/// This function creates a test application and cleans up any existing test data.
/// Use this when you need to ensure a clean database state for your test.
#[allow(dead_code)]
pub async fn setup_clean_test_app() -> Router {
    // Initialize logging for tests
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("secure=debug".parse().unwrap()),
        )
        .try_init();

    let db = setup_test_db().await;
    let config = setup_test_config();

    // Clean up any existing test data before running tests
    cleanup_test_db(&db).await;

    routes::create_app(db, config).await
}

/// Extract JSON body from response
///
/// Helper function to extract and parse JSON body from an HTTP response
#[allow(dead_code)]
pub async fn extract_json_body(response: Response<Body>) -> Value {
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    serde_json::from_slice(&body_bytes).expect("Failed to parse JSON response")
}

/// Generate a test Ethereum wallet address
///
/// Creates a valid format Ethereum address for testing
pub fn generate_test_wallet_address(index: u32) -> String {
    format!("0x{:040x}", index)
}

/// Create test application state
///
/// Creates an AppState instance for unit testing handlers
#[allow(dead_code)]
pub async fn create_test_state() -> secure::routes::AppState {
    let db = setup_test_db().await;
    let config = setup_test_config();

    // Initialize IPFS client
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
        .expect("Failed to create IPFS client");

    // Initialize TEE service
    let tee_config = secure::infra::tee::TeeConfig {
        endpoint: config
            .tee
            .enabled
            .then(|| "/var/run/dstack.sock".to_string()),
    };
    let tee_service = Arc::new(secure::infra::tee::TeeService::new(tee_config));

    // Initialize contract caller
    let contract_caller = secure::infra::contracts::ContractCaller::new(config.chain.clone())
        .await
        .expect("Failed to create contract caller");

    // Create notifier
    let notifier = Arc::new(secure::infra::Notifier::new());

    secure::routes::AppState::new(
        db,
        config,
        ipfs_client,
        contract_caller,
        notifier,
        tee_service,
    )
}

/// Clean up test database
///
/// Removes test data before or after tests
pub async fn cleanup_test_db(db: &Database) {
    // Clean up test data in reverse dependency order
    let tables = [
        "data_usage",
        "vote",
        "committee_member",
        "blockchain_transaction",
        "algo_exe",
        "algo",
        "dataset",
        "contract_meta",
    ];

    for table in &tables {
        let query = format!("DELETE FROM {}", table);
        sqlx::query(&query)
            .execute(db.pool())
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to clean up table {}: {}", table, e);
                Default::default()
            });
    }
}

/// Create a multipart form body for file upload
///
/// Helper to create multipart/form-data request bodies for testing
pub fn create_multipart_body(
    fields: Vec<(&str, &str)>,
    file_field: Option<(&str, &str, &[u8])>,
) -> (String, Vec<u8>) {
    let boundary = "----WebKitFormBoundaryTest";
    let mut body = Vec::new();

    // Add regular fields
    for (name, value) in fields {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    // Add file field if provided
    if let Some((field_name, file_name, file_content)) = file_field {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                field_name, file_name
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(file_content);
        body.extend_from_slice(b"\r\n");
    }

    // End boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    (format!("multipart/form-data; boundary={}", boundary), body)
}

/// Assert API response success
///
/// Helper to verify successful API responses
#[allow(dead_code)]
pub fn assert_success_response(body: &Value) {
    assert_eq!(
        body["code"].as_str().unwrap(),
        "SUCCESS",
        "Expected success response, got: {}",
        body
    );
}

/// Assert API response error
///
/// Helper to verify error API responses
#[allow(dead_code)]
pub fn assert_error_response(body: &Value, expected_code: &str) {
    assert_eq!(
        body["code"].as_str().unwrap(),
        expected_code,
        "Expected error code {}, got: {}",
        expected_code,
        body
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_test_db() {
        let db = setup_test_db().await;
        assert!(db.pool().acquire().await.is_ok());
    }

    #[test]
    fn test_generate_wallet_address() {
        let addr = generate_test_wallet_address(1);
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
    }

    #[test]
    fn test_create_multipart_body() {
        let fields = vec![("name", "test"), ("value", "123")];
        let (content_type, body) = create_multipart_body(fields, None);

        assert!(content_type.contains("multipart/form-data"));
        assert!(body.len() > 0);

        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("name=\"name\""));
        assert!(body_str.contains("test"));
        assert!(body_str.contains("name=\"value\""));
        assert!(body_str.contains("123"));
    }
}
