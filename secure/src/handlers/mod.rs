pub mod algo_exe;
pub mod algorithm;
pub mod attestation;
pub mod committee;
pub mod contract;
pub mod dataset;
pub mod execution;
pub mod health;

pub mod not_found;
pub mod static_dataset;
pub mod vote;
// pub mod vote;
// pub mod contract;

// Re-export handler functions for convenience
pub use algo_exe::{get_algo_exe, list_algo_exes, submit_algo_exe};
pub use static_dataset::{create_static_dataset, list_static_datasets};

// Import common types from avinapi
use avinapi::response::JsonResult;
use serde::{Deserialize, Serialize};

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health check handler
pub async fn health_check() -> JsonResult<HealthResponse> {
    avinapi::data!(HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now(),
    })
}

/// API version response
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
    pub service: String,
}

/// Version handler
pub async fn version() -> avinapi::response::JsonResult<VersionResponse> {
    avinapi::data!(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        service: "delong-protocol-secure".to_string(),
    })
}

/// Handler state containing shared resources
#[derive(Clone)]
pub struct HandlerState {
    pub db_pool: sqlx::PgPool,
    pub ipfs_client: std::sync::Arc<ipfs_api_backend_hyper::IpfsClient>,
    // TODO: Add blockchain client
    // TODO: Add TEE client for encryption
}
