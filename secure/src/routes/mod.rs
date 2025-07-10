use crate::{handlers, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Create all routes for the secure service
pub fn create_router(state: Arc<AppState>) -> Router {
    let protected_routes = Router::new()
        // --- Static Datasets (/static-datasets) ---
        .route(
            "/api/static-datasets",
            post(handlers::dataset::create_dataset).get(handlers::dataset::list_datasets),
        )
        .route(
            "/api/static-datasets/:id",
            get(handlers::dataset::get_dataset).put(handlers::dataset::update_dataset),
        )
        // --- Algorithm Executions (/algo-exes) ---
        .route(
            "/api/algo-exes",
            post(handlers::algo_exe::submit_algorithm_execution)
                .get(handlers::algo_exe::list_executions),
        )
        .route("/api/algo-exes/:id", get(handlers::algo_exe::get_execution))
        // --- Committee Members (/committee) ---
        .route(
            "/api/committee",
            post(handlers::committee::upsert_committee_member)
                .get(handlers::committee::list_committee_members),
        )
        // --- Votes (/votes) ---
        .route(
            "/api/votes",
            get(handlers::votes::list_votes).post(handlers::votes::cast_vote),
        )
        .route("/api/votes/:id", get(handlers::votes::get_vote));

    // Combine public and protected routes
    Router::new()
        .merge(protected_routes)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
} 