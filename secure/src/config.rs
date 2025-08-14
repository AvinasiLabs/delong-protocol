//! Configuration module for the secure service

use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub runtime: RuntimeConfig,
    pub ipfs: IpfsConfig,
    pub chain: ChainConfig,
    pub tee: TeeConfig,
    pub committee: CommitteeConfig,
    pub dataset: DatasetConfig,
}

/// Dataset configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetConfig {
    pub sample_size: usize,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: u64,
    pub idle_timeout: u64,
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub max_execution_time: u64,
    pub max_memory: usize,
    pub worker_threads: usize,
    pub queue_size: usize,
    pub max_concurrent_executions: usize,
    pub working_directory: PathBuf,
    pub python_path: String,
    pub poll_interval: u64,
    pub dataset_base_path: PathBuf,
    pub execution_timeout_secs: u64,
}

/// IPFS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpfsConfig {
    pub api_url: String,
    pub gateway_url: String,
    pub timeout: u64,
}

/// Blockchain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub chain_id: u64,
    pub contract_address: Option<String>,
    pub private_key: Option<String>,
    pub confirmations: u64,
    pub gas_price_multiplier: f64,
    pub max_gas_limit: u64,
    pub funding_threshold_eth: f64,
    pub funding_amount_eth: f64,
    pub sync_interval: u64,
    pub sync_batch_size: usize,
    pub block_batch_size: usize,
}

/// TEE configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    pub enabled: bool,
    pub client_kind: String,
}

/// Committee configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitteeConfig {
    pub size: i64,
}

impl Config {
    /// Load configuration from environment variables
    /// This will try to load from .env file first if it exists
    pub fn load() -> Result<Self, String> {
        // Try to load .env file from current directory or parent directories
        // This is non-fatal - if .env doesn't exist, we'll use system env vars
        dotenvy::dotenv().ok();

        Ok(Config {
            server: ServerConfig {
                host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("SERVER_PORT")
                    .unwrap_or_else(|_| "11000".to_string())
                    .parse()
                    .map_err(|_| "Invalid SERVER_PORT")?,
                workers: env::var("SERVER_WORKERS")
                    .unwrap_or_else(|_| "4".to_string())
                    .parse()
                    .map_err(|_| "Invalid SERVER_WORKERS")?,
            },
            database: DatabaseConfig {
                url: env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set")?,
                max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "32".to_string())
                    .parse()
                    .map_err(|_| "Invalid DATABASE_MAX_CONNECTIONS")?,
                min_connections: env::var("DATABASE_MIN_CONNECTIONS")
                    .unwrap_or_else(|_| "1".to_string())
                    .parse()
                    .map_err(|_| "Invalid DATABASE_MIN_CONNECTIONS")?,
                connect_timeout: env::var("DATABASE_CONNECT_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .map_err(|_| "Invalid DATABASE_CONNECT_TIMEOUT")?,
                idle_timeout: env::var("DATABASE_IDLE_TIMEOUT")
                    .unwrap_or_else(|_| "600".to_string())
                    .parse()
                    .map_err(|_| "Invalid DATABASE_IDLE_TIMEOUT")?,
            },
            runtime: RuntimeConfig {
                max_execution_time: env::var("RUNTIME_MAX_EXECUTION_TIME")
                    .unwrap_or_else(|_| "3600".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_MAX_EXECUTION_TIME")?,
                max_memory: env::var("RUNTIME_MAX_MEMORY")
                    .unwrap_or_else(|_| "1024".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_MAX_MEMORY")?,
                worker_threads: env::var("RUNTIME_WORKER_THREADS")
                    .unwrap_or_else(|_| "4".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_WORKER_THREADS")?,
                queue_size: env::var("RUNTIME_QUEUE_SIZE")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_QUEUE_SIZE")?,
                max_concurrent_executions: env::var("RUNTIME_MAX_CONCURRENT_EXECUTIONS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_MAX_CONCURRENT_EXECUTIONS")?,
                working_directory: PathBuf::from(
                    env::var("RUNTIME_WORKING_DIRECTORY")
                        .unwrap_or_else(|_| "/tmp/delong-runtime".to_string()),
                ),
                python_path: env::var("RUNTIME_PYTHON_PATH")
                    .unwrap_or_else(|_| "python3".to_string()),
                poll_interval: env::var("RUNTIME_POLL_INTERVAL")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_POLL_INTERVAL")?,
                dataset_base_path: PathBuf::from(
                    env::var("RUNTIME_DATASET_BASE_PATH")
                        .unwrap_or_else(|_| "/tmp/datasets".to_string()),
                ),
                execution_timeout_secs: env::var("RUNTIME_EXECUTION_TIMEOUT")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .map_err(|_| "Invalid RUNTIME_EXECUTION_TIMEOUT")?,
            },
            ipfs: IpfsConfig {
                api_url: env::var("IPFS_API_URL")
                    .unwrap_or_else(|_| "http://localhost:5001".to_string()),
                gateway_url: env::var("IPFS_GATEWAY_URL")
                    .unwrap_or_else(|_| "http://localhost:8080".to_string()),
                timeout: env::var("IPFS_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .map_err(|_| "Invalid IPFS_TIMEOUT")?,
            },
            chain: ChainConfig {
                rpc_url: env::var("CHAIN_RPC_URL")
                    .unwrap_or_else(|_| "http://localhost:11003".to_string()),
                ws_url: env::var("CHAIN_WS_URL").ok(),
                chain_id: env::var("CHAIN_ID")
                    .unwrap_or_else(|_| "31337".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_ID")?,
                contract_address: env::var("CHAIN_CONTRACT_ADDRESS").ok(),
                private_key: env::var("OFFICIAL_ACCOUNT_PRIVATE_KEY").ok(),
                confirmations: env::var("CHAIN_CONFIRMATIONS")
                    .unwrap_or_else(|_| "1".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_CONFIRMATIONS")?,
                gas_price_multiplier: env::var("CHAIN_GAS_PRICE_MULTIPLIER")
                    .unwrap_or_else(|_| "1.1".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_GAS_PRICE_MULTIPLIER")?,
                max_gas_limit: env::var("CHAIN_MAX_GAS_LIMIT")
                    .unwrap_or_else(|_| "10000000".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_MAX_GAS_LIMIT")?,
                funding_threshold_eth: env::var("CHAIN_FUNDING_THRESHOLD_ETH")
                    .unwrap_or_else(|_| "0.5".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_FUNDING_THRESHOLD_ETH")?,
                funding_amount_eth: env::var("CHAIN_FUNDING_AMOUNT_ETH")
                    .unwrap_or_else(|_| "1.0".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_FUNDING_AMOUNT_ETH")?,
                sync_interval: env::var("CHAIN_SYNC_INTERVAL")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_SYNC_INTERVAL")?,
                sync_batch_size: env::var("CHAIN_SYNC_BATCH_SIZE")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_SYNC_BATCH_SIZE")?,
                block_batch_size: env::var("CHAIN_BLOCK_BATCH_SIZE")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .map_err(|_| "Invalid CHAIN_BLOCK_BATCH_SIZE")?,
            },
            tee: TeeConfig {
                enabled: env::var("TEE_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .map_err(|_| "Invalid TEE_ENABLED")?,
                client_kind: env::var("TEE_CLIENT_KIND").unwrap_or_else(|_| "SGX".to_string()),
            },
            committee: CommitteeConfig {
                size: env::var("COMMITTEE_SIZE")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()
                    .map_err(|_| "Invalid COMMITTEE_SIZE")?,
            },
            dataset: DatasetConfig {
                sample_size: env::var("DATASET_SAMPLE_SIZE")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .map_err(|_| "Invalid DATASET_SAMPLE_SIZE")?,
            },
        })
    }

    /// Alias for load() to maintain compatibility
    pub fn from_env() -> Result<Self, String> {
        Self::load()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 11000,
                workers: 4,
            },
            database: DatabaseConfig {
                url: "postgres://secure_user:secure_dev_password@localhost:11001/secure_db"
                    .to_string(),
                max_connections: 32,
                min_connections: 1,
                connect_timeout: 30,
                idle_timeout: 600,
            },
            runtime: RuntimeConfig {
                max_execution_time: 3600,
                max_memory: 1024,
                worker_threads: 4,
                queue_size: 100,
                max_concurrent_executions: 10,
                working_directory: PathBuf::from("/tmp/delong-runtime"),
                python_path: "python3".to_string(),
                poll_interval: 5,
                dataset_base_path: PathBuf::from("/tmp/datasets"),
                execution_timeout_secs: 300,
            },
            ipfs: IpfsConfig {
                api_url: "http://localhost:5001".to_string(),
                gateway_url: "http://localhost:8080".to_string(),
                timeout: 30,
            },
            chain: ChainConfig {
                rpc_url: "http://localhost:11003".to_string(),
                ws_url: None,
                chain_id: 31337,
                contract_address: None,
                private_key: None,
                confirmations: 1,
                gas_price_multiplier: 1.1,
                max_gas_limit: 10_000_000,
                funding_threshold_eth: 0.5,
                funding_amount_eth: 1.0,
                sync_interval: 10,
                sync_batch_size: 1000,
                block_batch_size: 100,
            },
            tee: TeeConfig {
                enabled: false,
                client_kind: "SGX".to_string(),
            },
            committee: CommitteeConfig {
                size: 3,
            },
            dataset: DatasetConfig {
                sample_size: 100,
            },
        }
    }
}

/// Initialize configuration (deprecated, use Config::load() instead)
#[deprecated(note = "Use Config::load() instead")]
pub fn init_config() -> Config {
    Config::load().expect("Failed to load configuration")
}

/// Get database URL from environment
pub fn get_database_url() -> String {
    dotenvy::dotenv().ok();
    env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://secure_user:secure_dev_password@localhost:11001/secure_db".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.port, 11000);
        assert_eq!(config.chain.chain_id, 31337);
    }

    #[test]
    fn test_load_with_env() {
        // Set some test environment variables
        env::set_var("SERVER_PORT", "8080");
        env::set_var("DATABASE_URL", "postgres://test@localhost/test");

        let config = Config::load().expect("Failed to load config");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.url, "postgres://test@localhost/test");

        // Clean up
        env::remove_var("SERVER_PORT");
        env::remove_var("DATABASE_URL");
    }
}
