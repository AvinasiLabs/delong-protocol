//! Test helper functions and utilities
//!
//! This module provides common test setup functionality to ensure
//! consistent test environments across all test suites.

use crate::{
    infra::{db::Database, Hub},
    routes, Config,
};
use axum::Router;
use std::sync::Arc;
use tracing::{info, warn};

/// Set up a test database connection
///
/// This function creates a database connection for testing purposes
pub async fn setup_test_db() -> Database {
    // Initialize test environment
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
    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Initialize and return configuration
    Config::from_env().expect("Failed to load config")
}

/// Set up a test application with all required dependencies
///
/// This function:
/// 1. Loads test environment variables from .env.dev
/// 2. Initializes configuration
/// 3. Creates database connection
/// 4. Runs database migrations
/// 5. Initializes WebSocket hub
/// 6. Creates and returns the application router
pub async fn setup_test_app() -> Router {
    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!(
            "Failed to load .env.dev: {}. Using environment variables.",
            e
        );
    }

    // Initialize configuration
    let config = Config::from_env().expect("Failed to load config");

    // Initialize database
    let db = Database::new(&config.database)
        .await
        .expect("Failed to connect to database");

    // Run migrations - IMPORTANT for tests
    info!("Running database migrations for tests");
    db.run_migrations()
        .await
        .expect("Failed to run database migrations");

    // Initialize WebSocket hub
    let ws_hub = Arc::new(Hub::new());

    // Create app
    routes::create_app(db, config, ws_hub).await
}

/// Create a multipart form data body for file upload tests
///
/// This is useful for testing endpoints that accept file uploads
pub fn create_multipart_body(
    fields: Vec<(&str, &str)>,
    file_field: &str,
    file_name: &str,
    file_content: &[u8],
) -> (String, Vec<u8>) {
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let mut body = Vec::new();

    // Add regular fields
    for (name, value) in fields {
        body.extend_from_slice(format!("------{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    // Add file field
    body.extend_from_slice(format!("------{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
            file_field, file_name
        )
        .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(file_content);
    body.extend_from_slice(b"\r\n");

    // End boundary
    body.extend_from_slice(format!("------{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (content_type, body)
}

/// Helper to extract JSON response body from axum response
pub async fn extract_json_body(
    response: axum::http::Response<axum::body::Body>,
) -> serde_json::Value {
    use axum::body::to_bytes;

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    serde_json::from_slice(&body).expect("Failed to parse JSON response")
}

/// Helper to create a test dataset name
///
/// Generates a unique dataset name using timestamp to avoid conflicts
pub fn generate_test_dataset_name(prefix: &str) -> String {
    format!("{}-{}", prefix, chrono::Utc::now().timestamp_nanos())
}

/// Helper to wait for async operations to complete
///
/// Useful when testing operations that trigger background tasks
pub async fn wait_for_async_operations(duration_ms: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
}

/// Helper to generate unique test wallet addresses
///
/// Generates a deterministic but unique Ethereum address for testing
pub fn generate_test_wallet_address(prefix: &str) -> String {
    use sha2::{Digest, Sha256};

    let unique_string = format!(
        "{}-{}",
        prefix,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let mut hasher = Sha256::new();
    hasher.update(unique_string.as_bytes());
    let hash = hasher.finalize();

    // Take first 20 bytes of hash for Ethereum address
    let address_bytes = &hash[..20];
    format!("0x{}", hex::encode(address_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_test_dataset_name() {
        let name1 = generate_test_dataset_name("test");
        let name2 = generate_test_dataset_name("test");

        // Names should be unique
        assert_ne!(name1, name2);

        // Names should start with prefix
        assert!(name1.starts_with("test-"));
        assert!(name2.starts_with("test-"));
    }

    #[test]
    fn test_create_multipart_body() {
        let fields = vec![("name", "test-dataset"), ("author", "0x123")];
        let (content_type, body) =
            create_multipart_body(fields, "file", "test.csv", b"col1,col2\nval1,val2");

        assert!(content_type.contains("multipart/form-data"));
        assert!(content_type.contains("boundary="));

        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("name=\"name\""));
        assert!(body_str.contains("test-dataset"));
        assert!(body_str.contains("name=\"author\""));
        assert!(body_str.contains("0x123"));
        assert!(body_str.contains("filename=\"test.csv\""));
        assert!(body_str.contains("col1,col2"));
    }
}
