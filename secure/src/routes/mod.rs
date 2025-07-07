use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::handlers;

/// Create all routes for the secure service
pub fn create_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/api/static-datasets", get(handlers::dataset::list_datasets))
        .route("/api/static-datasets/:id", get(handlers::dataset::get_dataset))
        .route("/api/algorithm-executions", get(handlers::algo_exe::list_executions))
        .route("/api/algorithm-executions/:id", get(handlers::algo_exe::get_execution))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
} 