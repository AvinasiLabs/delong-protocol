use axum::{http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};

/// Simple health status for the gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: String,
}

/// Basic health check handler
pub async fn health_handler() -> Result<Json<HealthResponse>, StatusCode> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        service: "delong-gateway".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Ok(Json(response))
}
