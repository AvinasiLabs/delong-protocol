//! Test helper functions and utilities for integration tests
//!
//! This module provides common test setup functionality to ensure
//! consistent test environments across all test suites.

use secure::{config::Config, infra::db::Database};
use tracing::{info, warn};
/// Set up a test database connection
///
/// This function creates a database connection for testing purposes
pub async fn setup_test_db() -> Database {
    // Initialize test environment - load from secure/.env
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Initialize configuration - use Config::load() for consistency
    let config = Config::load().expect("Failed to load config");

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

    // Initialize and return configuration - use Config::load() for consistency
    Config::load().expect("Failed to load config")
}

/// Generate a test Ethereum wallet address
///
/// Creates a valid format Ethereum address for testing
pub fn generate_test_wallet_address(index: u32) -> String {
    format!("0x{:040x}", index)
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
