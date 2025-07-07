use std::time::Duration;
use tokio::time::sleep;
use secure::config::SecureConfig;
use secure::sync::BlockchainSyncService;

#[tokio::test]
async fn test_blockchain_sync_service() {
    // Initialize tracing for test
    tracing_subscriber::fmt::init();

    // Create test configuration
    let config = SecureConfig {
        server: secure::config::ServerConfig {
            host: "localhost".to_string(),
            port: 8082,
        },
        database: secure::config::DatabaseConfig {
            url: "postgresql://test:test@localhost/test".to_string(),
            max_connections: 5,
        },
        tee: secure::config::TeeConfig {
            client_type: "mock".to_string(),
            master_key_path: "/tmp/test_key".to_string(),
            attestation_required: false,
        },
        ipfs: secure::config::IpfsConfig {
            api_url: "http://localhost:5001".to_string(),
        },
        blockchain: secure::config::BlockchainConfig {
            client_type: "mock".to_string(),
            http_url: "http://localhost:8545".to_string(),
            ws_url: "ws://localhost:8545".to_string(),
            chain_id: 1,
            private_key: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            sync_interval_seconds: 1, // Fast sync for testing
        },
        auth: secure::config::AuthConfig {
            use_jwt: false,
            jwt_secret: "test_secret".to_string(),
        },
    };

    // Create and start the blockchain sync service
    let mut sync_service = BlockchainSyncService::new(config);
    
    // Start the service in a background task
    let sync_handle = tokio::spawn(async move {
        sync_service.start().await
    });

    // Let it run for a few seconds
    sleep(Duration::from_secs(5)).await;

    // Stop the service
    sync_handle.abort();

    // The test passes if we get here without panicking
    println!("Blockchain sync service test completed successfully");
}

#[tokio::test]
async fn test_blockchain_sync_service_lifecycle() {
    // Create test configuration
    let config = SecureConfig {
        server: secure::config::ServerConfig {
            host: "localhost".to_string(),
            port: 8082,
        },
        database: secure::config::DatabaseConfig {
            url: "postgresql://test:test@localhost/test".to_string(),
            max_connections: 5,
        },
        tee: secure::config::TeeConfig {
            client_type: "mock".to_string(),
            master_key_path: "/tmp/test_key".to_string(),
            attestation_required: false,
        },
        ipfs: secure::config::IpfsConfig {
            api_url: "http://localhost:5001".to_string(),
        },
        blockchain: secure::config::BlockchainConfig {
            client_type: "mock".to_string(),
            http_url: "http://localhost:8545".to_string(),
            ws_url: "ws://localhost:8545".to_string(),
            chain_id: 1,
            private_key: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            sync_interval_seconds: 1,
        },
        auth: secure::config::AuthConfig {
            use_jwt: false,
            jwt_secret: "test_secret".to_string(),
        },
    };

    // Create sync service
    let sync_service = BlockchainSyncService::new(config);

    // Test initial state
    assert!(!sync_service.is_running().await);

    // Test stop without start
    sync_service.stop().await;
    assert!(!sync_service.is_running().await);

    println!("Blockchain sync service lifecycle test completed successfully");
} 