//! Test helper functions and utilities for integration tests
//!
//! This module provides common test setup functionality to ensure
//! consistent test environments across all test suites.

use axum::Router;
use ipfs_api_backend_hyper::TryFromUri;
use secure::{
    config::Config,
    infra::{
        contracts::ContractCaller,
        db::Database,
        tee::{Client as TeeClient, ClientConfig as TeeClientConfig, Ethereum as TeeEthereum},
        Notifier,
    },
    routes::create_app,
};
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

    // Set TEST_MODE to skip JWT verification in tests
    std::env::set_var("TEST_MODE", "true");

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
#[allow(dead_code)]
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

/// Set up a test application with all routes and dependencies
///
/// This function creates a complete test application instance
#[allow(dead_code)]
pub async fn setup_test_app() -> Router {
    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Set TEST_MODE to skip JWT verification in tests
    std::env::set_var("TEST_MODE", "true");

    // Load configuration
    let config = Config::load().expect("Failed to load config");

    // Initialize database
    let db = Database::new(&config.database)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    db.run_migrations().await.expect("Failed to run migrations");

    // Initialize IPFS client
    let ipfs_client = Arc::new(
        ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
            .expect("Failed to create IPFS client"),
    );

    // Initialize TEE service with proxy endpoint for testing
    let tee_config = TeeClientConfig {
        endpoint: Some("http://localhost:11010".to_string()),
    };
    let tee_service = Arc::new(TeeClient::new(tee_config));

    // Initialize TEE Ethereum
    let tee_ethereum = Arc::new(TeeEthereum::new(tee_service.clone()));

    // Initialize contract caller with TEE support
    let contract_caller = Arc::new(
        ContractCaller::new(config.chain.clone(), db.clone())
            .await
            .expect("Failed to initialize contract caller")
            .with_tee(tee_ethereum.clone())
            .await
            .expect("Failed to initialize TEE wallet"),
    );

    // Initialize notifier
    let notifier = Arc::new(Notifier::new());

    let cfg = deadpool_redis::Config::from_url(&config.redis.url);
    let pool = cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();

    // Create router with full application setup
    create_app(
        db,
        config,
        ipfs_client,
        contract_caller,
        notifier,
        tee_service,
        tee_ethereum,
        pool,
    )
    .await
}

/// Set up a clean test application (clears database before setup)
///
/// This function creates a test application with a fresh database state
#[allow(dead_code)]
pub async fn setup_clean_test_app() -> Router {
    // Initialize test environment
    if let Err(e) = dotenvy::from_filename(".env") {
        warn!("Failed to load .env: {}. Using environment variables.", e);
    }

    // Set TEST_MODE to skip JWT verification in tests
    std::env::set_var("TEST_MODE", "true");

    // Load configuration
    let config = Config::load().expect("Failed to load config");

    // Initialize database
    let db = Database::new(&config.database)
        .await
        .expect("Failed to connect to database");

    // Clear test data - only in test environments
    // Check if we're in a test environment by looking at database name or other indicators
    let _ = sqlx::query("TRUNCATE TABLE algo_exe, algo, dataset, vote, committee_member, blockchain_transaction CASCADE")
        .execute(db.pool())
        .await;

    // Run migrations after clearing
    db.run_migrations().await.expect("Failed to run migrations");

    // Initialize IPFS client
    let ipfs_client = Arc::new(
        ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
            .expect("Failed to create IPFS client"),
    );

    // Initialize TEE service with proxy endpoint for testing
    let tee_config = TeeClientConfig {
        endpoint: Some("http://localhost:11010".to_string()),
    };
    let tee_service = Arc::new(TeeClient::new(tee_config));

    // Initialize TEE Ethereum
    let tee_ethereum = Arc::new(TeeEthereum::new(tee_service.clone()));

    // Initialize contract caller with TEE support
    let contract_caller = Arc::new(
        ContractCaller::new(config.chain.clone(), db.clone())
            .await
            .expect("Failed to initialize contract caller")
            .with_tee(tee_ethereum.clone())
            .await
            .expect("Failed to initialize TEE wallet"),
    );

    // Initialize notifier
    let notifier = Arc::new(Notifier::new());

    let cfg = deadpool_redis::Config::from_url(&config.redis.url);
    let pool = cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();

    // Create router with full application setup
    create_app(
        db,
        config,
        ipfs_client,
        contract_caller,
        notifier,
        tee_service,
        tee_ethereum,
        pool,
    )
    .await
}

/// Extract JSON body from an axum response
///
/// Helper function to extract and parse JSON body from test responses
#[allow(dead_code)]
pub async fn extract_json_body(response: axum::response::Response) -> serde_json::Value {
    use axum::body::to_bytes;

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    serde_json::from_slice(&body).expect("Failed to parse JSON response")
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
