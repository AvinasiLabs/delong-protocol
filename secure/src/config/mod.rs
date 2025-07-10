use serde::{Deserialize, Serialize};
use std::env;

/// Configuration for the secure service running in TEE environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureConfig {
    /// Server configuration
    pub server: ServerConfig,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// TEE configuration
    pub tee: TeeConfig,
    
    /// IPFS configuration
    pub ipfs: IpfsConfig,
    
    /// Blockchain configuration
    pub blockchain: BlockchainConfig,
    
    /// Authentication configuration
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    pub client_type: String,
    pub master_key_path: String,
    pub attestation_required: bool,
    pub key_vault_master_key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpfsConfig {
    pub api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    pub client_type: String,
    pub http_url: String,
    pub ws_url: String,
    pub chain_id: u64,
    pub private_key: String,
    pub sync_interval_seconds: u64,
    pub sync_enabled: bool,
    pub data_contribution_address: String,
    pub algorithm_review_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub use_jwt: bool,
    pub jwt_secret: String,
}

impl SecureConfig {
    /// Create a mock configuration for testing
    pub fn mock() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8082,
            },
            database: DatabaseConfig {
                url: "postgresql://test:test@localhost:5432/test".to_string(),
                max_connections: 5,
            },
            tee: TeeConfig {
                client_type: "mock".to_string(),
                master_key_path: "/tmp/mock_master.key".to_string(),
                attestation_required: false,
                key_vault_master_key_path: "/tmp/key_vault_master.key".to_string(),
            },
            ipfs: IpfsConfig {
                api_url: "http://localhost:5001".to_string(),
            },
            blockchain: BlockchainConfig {
                client_type: "mock".to_string(),
                http_url: "http://localhost:8545".to_string(),
                ws_url: "ws://localhost:8545".to_string(),
                chain_id: 1337,
                private_key: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
                sync_interval_seconds: 60,
                sync_enabled: false,
                data_contribution_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
                algorithm_review_address: "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512".to_string(),
            },
            auth: AuthConfig {
                use_jwt: false,
                jwt_secret: "test_secret".to_string(),
            },
        }
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerConfig {
                host: env::var("SECURE_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("SECURE_PORT")
                    .unwrap_or_else(|_| "8082".to_string())
                    .parse()
                    .map_err(|_| ConfigError::InvalidPort)?,
            },
            database: DatabaseConfig {
                url: env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?,
                max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            },
            tee: TeeConfig {
                client_type: env::var("TEE_CLIENT_TYPE").unwrap_or_else(|_| "mock".to_string()),
                master_key_path: env::var("TEE_MASTER_KEY_PATH")
                    .unwrap_or_else(|_| "/secure/master.key".to_string()),
                attestation_required: env::var("TEE_ATTESTATION_REQUIRED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                key_vault_master_key_path: env::var("KEY_VAULT_MASTER_KEY_PATH")
                    .unwrap_or_else(|_| "/secure/key_vault.key".to_string()),
            },
            ipfs: IpfsConfig {
                api_url: env::var("IPFS_API_URL")
                    .unwrap_or_else(|_| "http://localhost:5001".to_string()),
            },
            blockchain: BlockchainConfig {
                client_type: env::var("BLOCKCHAIN_CLIENT_TYPE").unwrap_or_else(|_| "mock".to_string()),
                http_url: env::var("ETH_HTTP_URL").unwrap_or_else(|_| "https://mainnet.infura.io/v3/YOUR-PROJECT-ID".to_string()),
                ws_url: env::var("ETH_WS_URL").unwrap_or_else(|_| "wss://mainnet.infura.io/ws/v3/YOUR-PROJECT-ID".to_string()),
                chain_id: env::var("CHAIN_ID")
                    .unwrap_or_else(|_| "1".to_string())
                    .parse()
                    .map_err(|_| ConfigError::InvalidChainId)?,
                private_key: env::var("OFFICIAL_ACCOUNT_PRIVATE_KEY")
                    .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()),
                sync_interval_seconds: env::var("BLOCKCHAIN_SYNC_INTERVAL")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
                sync_enabled: env::var("BLOCKCHAIN_SYNC_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                data_contribution_address: env::var("DATA_CONTRIBUTION_ADDRESS")
                    .map_err(|_| ConfigError::MissingDataContributionAddress)?,
                algorithm_review_address: env::var("ALGORITHM_REVIEW_ADDRESS")
                    .map_err(|_| ConfigError::MissingAlgorithmReviewAddress)?,
            },
            auth: AuthConfig {
                use_jwt: env::var("USE_JWT")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "default_secret".to_string()),
            },
        })
    }
    
    /// Get server socket address
    pub fn socket_addr(&self) -> Result<std::net::SocketAddr, ConfigError> {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .map_err(|_| ConfigError::InvalidAddress)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid port number")]
    InvalidPort,
    #[error("Missing DATABASE_URL environment variable")]
    MissingDatabaseUrl,
    #[error("Missing ETH_HTTP_URL environment variable")]
    MissingEthUrl,
    #[error("Missing ETH_WS_URL environment variable")]
    MissingEthWsUrl,
    #[error("Missing CHAIN_ID environment variable")]
    MissingChainId,
    #[error("Invalid CHAIN_ID format")]
    InvalidChainId,
    #[error("Missing OFFICIAL_ACCOUNT_PRIVATE_KEY environment variable")]
    MissingPrivateKey,
    #[error("Invalid address format")]
    InvalidAddress,
    #[error("Missing DATA_CONTRIBUTION_ADDRESS environment variable")]
    MissingDataContributionAddress,
    #[error("Missing ALGORITHM_REVIEW_ADDRESS environment variable")]
    MissingAlgorithmReviewAddress,
} 