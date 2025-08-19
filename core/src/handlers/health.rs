//! Health handlers module
//!
//! This module contains a simple health check HTTP request handler
//! for monitoring basic service availability.

use avinapi::prelude::*;
use serde::Serialize;
use tracing::info;

/// Basic health status
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub service: String,
    pub timestamp: String,
}

/// Basic health check endpoint
pub async fn health_check() -> JsonResult<HealthStatus> {
    info!("Health check requested");

    let health_status = HealthStatus {
        status: "healthy".to_string(),
        service: "DeLong Protocol Core Service".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    data!(health_status)
}
