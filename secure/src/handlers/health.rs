use axum::response::Json;
use serde_json::json;

/// Basic health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "secure",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
