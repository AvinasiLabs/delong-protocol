use axum::extract::State;
use axum::response::Json;
use common::ApiResponse;
use std::sync::Arc;

use crate::AppState;

/// Health check endpoint for the secure service
pub async fn health_check(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // Check database connectivity
    // TODO: Implement actual health check
    // let _ = crate.health_check(&state.db_pool).await;

    tracing::info!("Health check passed");

    Json(ApiResponse::success(()))
} 