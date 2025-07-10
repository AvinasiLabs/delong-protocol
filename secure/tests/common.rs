//! Common test utilities for Secure Service integration tests
pub mod test_utils {
    use anyhow::Result;
    use common::prelude::*;
    use sqlx::PgPool;
    use std::sync::Once;
    use tracing_subscriber;

    static INIT: Once = Once::new();

    /// Initialize test logging (call once per test suite)
    pub fn init_test_logging() {
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_test_writer()
                .init();
        });
    }

    /// Create a test database connection pool
    pub async fn create_test_pool() -> anyhow::Result<PgPool> {
        dotenvy::dotenv().ok();
        let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://delong:delong_test_2025@localhost:5433/delong".to_string()
        });

        PgPool::connect(&database_url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to test database: {}", e))
    }

    /// Setup test environment
    pub async fn setup_test_env() -> Result<()> {
        init_test_logging();
        
        // Load test environment variables
        dotenvy::from_filename(".env.test").map_err(|e| anyhow::anyhow!("Failed to load .env.test: {}", e))?;
        
        // Initialize any other test-specific setup
        Ok(())
    }

    /// Cleanup test environment
    pub async fn cleanup_test_env() -> Result<()> {
        // Cleanup test data, close connections, etc.
        Ok(())
    }

    /// Generate test data for algorithms
    pub fn create_test_algorithm_request() -> AlgoExeData {
        AlgoExeData {
            id: 1,
            algo_id: "test_algo_001".to_string(),
            used_dataset: "dataset_001".to_string(),
            scientist_wallet: "0x1234567890123456789012345678901234567890".to_string(),
            review_status: "pending".to_string(),
            vote_start_time: None,
            vote_end_time: None,
            status: "queued".to_string(),
            start_time: None,
            end_time: None,
            result: None,
            error_msg: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            algo_name: Some("Test Algorithm".to_string()),
            algo_link: Some("https://github.com/test/repo".to_string()),
            cid: Some("QmTestCID123".to_string()),
        }
    }

    /// Generate test dataset metadata
    pub fn create_test_dataset() -> TestDataset {
        TestDataset {
            id: 1,
            name: "test_genomics_dataset".to_string(),
            ui_name: "Test Genomics Dataset".to_string(),
            description: Some("Test dataset for genomics research".to_string()),
            file_hash: "sha256:abcdef1234567890abcdef1234567890abcdef12".to_string(),
            ipfs_cid: "QmTestCID1234567890abcdef".to_string(),
            file_size: 1024000,
            file_format: "csv".to_string(),
            author: Some("Dr. Test".to_string()),
            author_wallet: "0x1234567890123456789012345678901234567890".to_string(),
            sample_url: Some("https://ipfs.io/ipfs/QmSampleCID".to_string()),
            file_path: Some("/secure/datasets/test_genomics.csv".to_string()),
            created_at: std::time::SystemTime::now(),
            updated_at: std::time::SystemTime::now(),
        }
    }

    /// Test dataset structure
    #[derive(Debug, Clone)]
    pub struct TestDataset {
        pub id: i32,
        pub name: String,
        pub ui_name: String,
        pub description: Option<String>,
        pub file_hash: String,
        pub ipfs_cid: String,
        pub file_size: i64,
        pub file_format: String,
        pub author: Option<String>,
        pub author_wallet: String,
        pub sample_url: Option<String>,
        pub file_path: Option<String>,
        pub created_at: std::time::SystemTime,
        pub updated_at: std::time::SystemTime,
    }

    /// Mock TEE environment for testing
    pub struct MockTeeEnvironment {
        pub attestation_valid: bool,
        pub key_vault_initialized: bool,
        pub enclave_id: String,
    }

    impl MockTeeEnvironment {
        pub fn new() -> Self {
            Self {
                attestation_valid: true,
                key_vault_initialized: true,
                enclave_id: "mock_enclave_123".to_string(),
            }
        }

        pub async fn mock_encrypt(&self, data: &[u8]) -> std::result::Result<Vec<u8>, String> {
            if !self.attestation_valid {
                return Err("TEE attestation failed".to_string());
            }
            
            // Mock encryption (XOR with simple key for testing)
            let key = 0x42u8;
            let encrypted: Vec<u8> = data.iter().map(|b| b ^ key).collect();
            Ok(encrypted)
        }

        pub async fn mock_decrypt(&self, encrypted_data: &[u8]) -> std::result::Result<Vec<u8>, String> {
            if !self.attestation_valid {
                return Err("TEE attestation failed".to_string());
            }
            
            // Mock decryption (same as encryption for XOR)
            self.mock_encrypt(encrypted_data).await
        }

        pub async fn derive_key(&self, dataset_hash: &str) -> std::result::Result<Vec<u8>, String> {
            if !self.key_vault_initialized {
                return Err("Key vault not initialized".to_string());
            }
            
            // Mock key derivation
            let key = format!("key_for_{}", dataset_hash);
            Ok(key.as_bytes().to_vec())
        }
    }

    /// Mock blockchain event for testing
    #[derive(Debug, Clone)]
    pub enum MockBlockchainEvent {
        AlgorithmSubmitted {
            algorithm_id: String,
            submitter: String,
            block_number: u64,
            transaction_hash: String,
            timestamp: u64,
        },
        DatasetRegistered {
            dataset_id: u64,
            author_wallet: String,
            ipfs_cid: String,
            file_hash: String,
        },
    }

    /// Mock blockchain client for testing
    pub struct MockBlockchainClient {
        pub connected: bool,
        pub mock_events: Vec<MockBlockchainEvent>,
    }

    impl MockBlockchainClient {
        pub fn new() -> Self {
            Self {
                connected: true,
                mock_events: vec![
                    MockBlockchainEvent::AlgorithmSubmitted {
                        algorithm_id: "algo_001".to_string(),
                        submitter: "0x1111111111111111111111111111111111111111".to_string(),
                        block_number: 12345,
                        transaction_hash: "0x1234567890abcdef".to_string(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    },
                ],
            }
        }

        pub async fn get_events(
            &self,
            from_block: u64,
            to_block: u64,
        ) -> std::result::Result<Vec<MockBlockchainEvent>, String> {
            if !self.connected {
                return Err("Blockchain client not connected".to_string());
            }
            
            // Filter mock events by block range
            let filtered_events: Vec<_> = self.mock_events
                .iter()
                .filter(|event| {
                    let event_block = match event {
                        MockBlockchainEvent::AlgorithmSubmitted { block_number, .. } => *block_number,
                        MockBlockchainEvent::DatasetRegistered { .. } => 12345, // Mock block number
                    };
                    event_block >= from_block && event_block <= to_block
                })
                .cloned()
                .collect();
            
            Ok(filtered_events)
        }

        pub async fn submit_result(&self, job_id: &str, result_hash: &str) -> std::result::Result<String, String> {
            if !self.connected {
                return Err("Blockchain client not connected".to_string());
            }
            
            // Mock transaction hash
            let tx_hash = format!("0x_mock_tx_{}_{}", job_id, result_hash);
            Ok(tx_hash)
        }
    }

    /// Test database setup helpers
    pub mod db {
        use super::*;
        use sqlx::Executor;

        pub async fn setup_test_database(pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
            pool.execute("SELECT 1").await?;
            Ok(())
        }

        pub async fn cleanup_test_database(_pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
            Ok(())
        }

        pub async fn seed_test_data(_pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
            Ok(())
        }
    }
} 