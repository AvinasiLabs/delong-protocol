//! Secure Service - TEE-enabled privacy-preserving computation service
//!
//! This service handles sensitive operations within a Trusted Execution Environment (TEE),
//! including static dataset encryption, algorithm execution, and blockchain synchronization.
//! All operations in this service must be deterministic and use minimal external dependencies
//! to maintain TEE security guarantees.

pub mod config;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod runtime;
pub mod services;
pub mod sync;
pub mod tee;
pub mod utils;

use axum::Router;
use common::ApiResult;
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::SecureConfig;

/// Create the main application router for the secure service
pub fn create_router(
    config: &SecureConfig,
    db_pool: PgPool,
) -> Router {
    let shared_state = Arc::new(AppState {
        config: config.clone(),
        db_pool,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient::default(),
    });

    routes::create_routes(shared_state)
}

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub config: SecureConfig,
    pub db_pool: PgPool,
    pub ipfs_client: ipfs_api_backend_hyper::IpfsClient,
}

/// Health check function for the secure service
/// Health check for database connectivity (currently disabled)
pub async fn health_check(_db_pool: &PgPool) -> ApiResult<()> {
    // TODO: Implement actual database health check when sqlx is properly configured
    tracing::info!("Health check placeholder - database connectivity not yet verified");
    Ok(())
} 