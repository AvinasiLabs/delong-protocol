use serde::Deserialize;
use std::env;

/// Main configuration structure for the secure service
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub runtime: RuntimeConfig,
    pub ipfs: IpfsConfig,
    pub chain: ChainConfig,
    pub tee: TeeConfig,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
}

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: u64,
    pub idle_timeout: u64,
}

/// Runtime configuration for algorithm execution
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    pub max_execution_time: u64, // in seconds
    pub max_memory: u64,         // in MB
    pub worker_threads: usize,
    pub queue_size: usize,
    pub max_concurrent_executions: usize,
    pub working_directory: String,
    pub python_path: String,
    pub poll_interval: u64, // in seconds
}

/// IPFS configuration
#[derive(Debug, Clone, Deserialize)]
pub struct IpfsConfig {
    pub api_url: String,
    pub gateway_url: String,
    pub timeout: u64, // in seconds
}

/// Blockchain configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub contract_address: String,
    pub sync_interval: u64, // in seconds
    pub sync_batch_size: u64,
    pub block_batch_size: u64,
}

/// TEE (Trusted Execution Environment) configuration
#[derive(Debug, Clone, Deserialize)]
pub struct TeeConfig {
    pub enabled: bool,
    pub attestation_provider: String,
    pub measurement_file: Option<String>,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, config::ConfigError> {
        let server = ServerConfig {
            host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            workers: env::var("SERVER_WORKERS").ok().and_then(|s| s.parse().ok()),
        };

        let database = DatabaseConfig {
            url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            min_connections: env::var("DATABASE_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .unwrap_or(1),
            connect_timeout: env::var("DATABASE_CONNECT_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            idle_timeout: env::var("DATABASE_IDLE_TIMEOUT")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .unwrap_or(600),
        };

        let runtime = RuntimeConfig {
            max_execution_time: env::var("RUNTIME_MAX_EXECUTION_TIME")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .unwrap_or(3600),
            max_memory: env::var("RUNTIME_MAX_MEMORY")
                .unwrap_or_else(|_| "512".to_string())
                .parse()
                .unwrap_or(512),
            worker_threads: env::var("RUNTIME_WORKER_THREADS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .unwrap_or(4),
            queue_size: env::var("RUNTIME_QUEUE_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
            max_concurrent_executions: env::var("RUNTIME_MAX_CONCURRENT_EXECUTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            working_directory: env::var("RUNTIME_WORKING_DIRECTORY")
                .unwrap_or_else(|_| "/tmp/delong".to_string()),
            python_path: env::var("PYTHON_PATH").unwrap_or_else(|_| "python3".to_string()),
            poll_interval: env::var("RUNTIME_POLL_INTERVAL")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
        };

        let ipfs = IpfsConfig {
            api_url: env::var("IPFS_API_URL")
                .unwrap_or_else(|_| "http://localhost:5001".to_string()),
            gateway_url: env::var("IPFS_GATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            timeout: env::var("IPFS_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
        };

        let chain = ChainConfig {
            rpc_url: env::var("CHAIN_RPC_URL")
                .unwrap_or_else(|_| "ws://localhost:8545".to_string()),
            chain_id: env::var("CHAIN_ID")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .unwrap_or(1),
            contract_address: env::var("CONTRACT_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            sync_interval: env::var("CHAIN_SYNC_INTERVAL")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            sync_batch_size: env::var("CHAIN_SYNC_BATCH_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
            block_batch_size: env::var("CHAIN_BLOCK_BATCH_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
        };

        let tee = TeeConfig {
            enabled: env::var("TEE_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            attestation_provider: env::var("TEE_ATTESTATION_PROVIDER")
                .unwrap_or_else(|_| "sgx".to_string()),
            measurement_file: env::var("TEE_MEASUREMENT_FILE").ok(),
        };

        Ok(Config {
            server,
            database,
            runtime,
            ipfs,
            chain,
            tee,
        })
    }
}

/// Initialize configuration from environment variables
pub fn init_config() -> Result<Config, env::VarError> {
    // Load .env file if it exists (ignore errors if it doesn't)
    let _ = dotenvy::dotenv();

    // Try to load from CONFIG_PATH environment variable first
    if let Ok(config_path) = env::var("CONFIG_PATH") {
        if let Ok(config_str) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<Config>(&config_str) {
                return Ok(config);
            }
        }
    }

    // Otherwise, build config from individual environment variables
    Ok(Config {
        server: ServerConfig {
            host: env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string())
                .parse()
                .expect("Invalid SERVER_HOST"),
            port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("Invalid SERVER_PORT"),
            workers: env::var("SERVER_WORKERS").ok().and_then(|s| s.parse().ok()),
        },
        database: DatabaseConfig {
            url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:password@localhost/delong".to_string()),
            max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "32".to_string())
                .parse()
                .expect("Invalid DATABASE_MAX_CONNECTIONS"),
            min_connections: env::var("DATABASE_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .expect("Invalid DATABASE_MIN_CONNECTIONS"),
            connect_timeout: env::var("DATABASE_CONNECT_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("Invalid DATABASE_CONNECT_TIMEOUT"),
            idle_timeout: env::var("DATABASE_IDLE_TIMEOUT")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .expect("Invalid DATABASE_IDLE_TIMEOUT"),
        },
        runtime: RuntimeConfig {
            max_execution_time: env::var("RUNTIME_MAX_EXECUTION_TIME")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .expect("Invalid RUNTIME_MAX_EXECUTION_TIME"),
            max_memory: env::var("RUNTIME_MAX_MEMORY")
                .unwrap_or_else(|_| "1024".to_string())
                .parse()
                .expect("Invalid RUNTIME_MAX_MEMORY"),
            worker_threads: env::var("RUNTIME_WORKER_THREADS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .expect("Invalid RUNTIME_WORKER_THREADS"),
            queue_size: env::var("RUNTIME_QUEUE_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("Invalid RUNTIME_QUEUE_SIZE"),
            max_concurrent_executions: env::var("RUNTIME_MAX_CONCURRENT_EXECUTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("Invalid RUNTIME_MAX_CONCURRENT_EXECUTIONS"),
            working_directory: env::var("RUNTIME_WORKING_DIRECTORY")
                .unwrap_or_else(|_| "/tmp/delong-runtime".to_string()),
            python_path: env::var("RUNTIME_PYTHON_PATH").unwrap_or_else(|_| "python3".to_string()),
            poll_interval: env::var("RUNTIME_POLL_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("Invalid RUNTIME_POLL_INTERVAL"),
        },
        ipfs: IpfsConfig {
            api_url: env::var("IPFS_API_URL")
                .unwrap_or_else(|_| "http://localhost:5001".to_string()),
            gateway_url: env::var("IPFS_GATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            timeout: env::var("IPFS_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("Invalid IPFS_TIMEOUT"),
        },
        chain: ChainConfig {
            rpc_url: env::var("CHAIN_RPC_URL")
                .unwrap_or_else(|_| "ws://localhost:8545".to_string()),
            chain_id: env::var("CHAIN_ID")
                .unwrap_or_else(|_| "1337".to_string())
                .parse()
                .expect("Invalid CHAIN_ID"),
            contract_address: env::var("CHAIN_CONTRACT_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            sync_interval: env::var("CHAIN_SYNC_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .expect("Invalid CHAIN_SYNC_INTERVAL"),
            sync_batch_size: env::var("CHAIN_SYNC_BATCH_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .expect("Invalid CHAIN_SYNC_BATCH_SIZE"),
            block_batch_size: env::var("CHAIN_BLOCK_BATCH_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .expect("Invalid CHAIN_BLOCK_BATCH_SIZE"),
        },
        tee: TeeConfig {
            enabled: env::var("TEE_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            attestation_provider: env::var("TEE_ATTESTATION_PROVIDER")
                .unwrap_or_else(|_| "sgx".to_string()),
            measurement_file: env::var("TEE_MEASUREMENT_FILE").ok(),
        },
    })
}

/// Default implementation for Config
impl Default for Config {
    fn default() -> Self {
        // Try to load from environment first
        init_config().unwrap_or_else(|_| {
            // Fall back to hardcoded defaults
            Self {
                server: ServerConfig {
                    host: "0.0.0.0".to_string(),
                    port: 8080,
                    workers: None,
                },
                database: DatabaseConfig {
                    url: "postgres://postgres:password@localhost/delong".to_string(),
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
                    working_directory: "/tmp/delong-runtime".to_string(),
                    python_path: "python3".to_string(),
                    poll_interval: 60,
                },
                ipfs: IpfsConfig {
                    api_url: "http://localhost:5001".to_string(),
                    gateway_url: "http://localhost:8080".to_string(),
                    timeout: 30,
                },
                chain: ChainConfig {
                    rpc_url: "ws://localhost:8545".to_string(),
                    chain_id: 1337,
                    contract_address: "0x0000000000000000000000000000000000000000".to_string(),
                    sync_interval: 60,
                    sync_batch_size: 1000,
                    block_batch_size: 100,
                },
                tee: TeeConfig {
                    enabled: env::var("TEE_ENABLED")
                        .unwrap_or_else(|_| "false".to_string())
                        .parse()
                        .unwrap_or(false),
                    attestation_provider: env::var("TEE_ATTESTATION_PROVIDER")
                        .unwrap_or_else(|_| "sgx".to_string()),
                    measurement_file: env::var("TEE_MEASUREMENT_FILE").ok(),
                },
            }
        })
    }
}
