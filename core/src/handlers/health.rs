//! Health handlers module
//!
//! This module contains a simple health check HTTP request handler
//! for monitoring basic service availability.

use crate::handlers::ApiResponse;
use axum::response::Json;
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
pub async fn health_check() -> Json<ApiResponse<HealthStatus>> {
    info!("Health check requested");

    let health_status = HealthStatus {
        status: "healthy".to_string(),
        service: "DeLong Protocol Core Service".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(ApiResponse::success(health_status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        let health_status = response.0;

        assert_eq!(health_status.code, common::ResponseCode::Success);
        assert!(health_status.data.is_some());

        let data = health_status.data.unwrap();
        assert_eq!(data.service, "DeLong Protocol Core Service");
        assert_eq!(data.status, "healthy");
    }
}
