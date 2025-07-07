use axum::{routing::{get, post}, Router};
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
        .route("/api/algo-exes", post(handlers::algo_exe::submit_algorithm_execution))
        .route("/api/algo-exes", get(handlers::algo_exe::list_executions))
        .route("/api/algo-exes/:id", get(handlers::algo_exe::get_execution))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
} 