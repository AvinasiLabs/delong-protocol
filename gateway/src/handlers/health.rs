use axum::{http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Simple health status for the gateway
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "status": "healthy",
    "service": "delong-gateway",
    "version": "0.2.0",
    "timestamp": "2024-01-01T00:00:00Z"
}))]
pub struct HealthResponse {
    /// Health status of the service
    #[schema(example = "healthy")]
    pub status: String,
    /// Name of the service
    #[schema(example = "delong-gateway")]
    pub service: String,
    /// Version of the service
    #[schema(example = "0.2.0")]
    pub version: String,
    /// Current timestamp in RFC3339 format
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub timestamp: String,
}

/// Basic health check handler
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    summary = "Health check",
    description = "Check if the gateway service is healthy and operational",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 500, description = "Service is unhealthy")
    )
)]
pub async fn health_handler() -> Result<Json<HealthResponse>, StatusCode> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        service: "delong-gateway".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Ok(Json(response))
}
