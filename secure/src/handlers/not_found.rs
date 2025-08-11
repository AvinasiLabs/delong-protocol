use axum::http::StatusCode;
use axum::response::Json;
use serde_json::json;

/// Handler for routes that are not found
pub async fn handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "The requested resource was not found",
            "status": 404,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    )
}
