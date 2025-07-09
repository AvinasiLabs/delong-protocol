use axum::{routing::{get, post}, Router, middleware, Extension};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::handlers;
use crate::middleware::JwtMiddleware;

/// Create all routes for the secure service
pub fn create_routes(state: Arc<AppState>) -> Router {
    // Create JWT middleware
    let jwt_middleware = JwtMiddleware::new(
        state.config.auth.jwt_secret.clone(),
        state.config.auth.use_jwt,
    );

    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/health", get(handlers::health::health_check));

    // Protected routes (require JWT authentication)
    let protected_routes = Router::new()
        .route("/api/static-datasets", get(handlers::dataset::list_datasets))
        .route("/api/static-datasets/{id}", get(handlers::dataset::get_dataset))
        .route("/api/algo-exes", post(handlers::algo_exe::submit_algorithm_execution))
        .route("/api/algo-exes", get(handlers::algo_exe::list_executions))
        .route("/api/algo-exes/{id}", get(handlers::algo_exe::get_execution))
        // Committee management routes
        .route("/api/committee", get(handlers::committee::list_committee_members))
        .route("/api/committee", post(handlers::committee::upsert_committee_member))
        .route("/api/committee/{wallet_address}", get(handlers::committee::get_committee_member))
        .route("/api/committee/stats", get(handlers::committee::get_committee_stats))
        // Voting routes
        .route("/api/votes", get(handlers::votes::list_votes))
        .route("/api/votes/{execution_id}", post(handlers::votes::cast_vote))
        .route("/api/votes/{execution_id}/tally", get(handlers::votes::get_vote_tally))
        .route("/api/votes/{execution_id}/votes", get(handlers::votes::get_execution_votes))
        .route("/api/votes/{execution_id}/voters/{voter_wallet}", get(handlers::votes::check_voter_voted))
        // Blockchain contract routes
        .route("/api/contracts/transactions", post(handlers::contracts::submit_transaction))
        .route("/api/contracts/transactions", get(handlers::contracts::list_transactions))
        .route("/api/contracts/transactions/{tx_hash}", get(handlers::contracts::get_transaction_status))
        .route("/api/contracts/status", get(handlers::contracts::get_sync_status))
        .route("/api/contracts/stats", get(handlers::contracts::get_contract_stats))
        .route("/api/contracts/events", get(handlers::contracts::get_blockchain_events))
        // Reports routes
        .route("/api/reports/committee-activity", get(handlers::reports::get_committee_activity_report))
        .route("/api/reports/voting", get(handlers::reports::get_voting_report))
        .route("/api/reports/system-activity", get(handlers::reports::get_system_activity_report))
        .route("/api/reports/governance-summary", get(handlers::reports::get_governance_summary))
        .layer(middleware::from_fn_with_state(
            Extension(jwt_middleware.clone()),
            JwtMiddleware::auth_middleware,
        ));

    // Combine public and protected routes
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state)
        .layer(Extension(jwt_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
} 