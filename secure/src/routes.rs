//! Routes configuration for the secure service
//!
//! This module sets up all HTTP routes, middleware, and application state
//! for the secure service running in the TEE environment.

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    config::Config,
    handlers,
    infra::{
        contracts::{ContractAddresses, ContractCaller, ContractConfig},
        db::Database,
        tee::{KeyVault, TappdAdapter},
    },
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub db: Database,
    /// Application configuration
    pub config: Config,
    /// IPFS client for decentralized storage
    pub ipfs_client: ipfs_api_backend_hyper::IpfsClient,
    /// Contract caller for blockchain interactions
    pub contract_caller: ContractCaller,
}

impl AppState {
    /// Create new application state
    pub fn new(
        db: Database,
        config: Config,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient,
        contract_caller: ContractCaller,
    ) -> Self {
        Self {
            db,
            config,
            ipfs_client,
            contract_caller,
        }
    }
}

/// Create the main application router with all routes and middleware
pub async fn create_app(db: Database, config: Config) -> Router {
    // Initialize IPFS client
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::default();

    // Initialize contract caller
    let contract_config = ContractConfig {
        http_url: config.chain.rpc_url.clone(),
        ws_url: config
            .chain
            .rpc_url
            .replace("http://", "ws://")
            .replace("https://", "wss://"),
        chain_id: config.chain.chain_id,
        addresses: ContractAddresses {
            data_contribution: config
                .chain
                .contract_address
                .parse()
                .expect("Invalid contract address"),
            algorithm_review: config
                .chain
                .contract_address
                .parse()
                .expect("Invalid contract address"),
        },
        funding_threshold_eth: 0.1,
        top_up_amount_eth: 1.0,
    };

    // Create key vault (using default for now)
    let key_vault = std::sync::Arc::new(KeyVault::new(Box::new(TappdAdapter::new())));

    let contract_caller = ContractCaller::new(
        contract_config,
        key_vault,
        None, // Private key should be configured separately for TEE
    )
    .await
    .expect("Failed to create contract caller");

    let state = Arc::new(AppState::new(db, config, ipfs_client, contract_caller));

    // Health check routes (no authentication)
    let health_routes = Router::new().route("/health", get(handlers::health::health_check));

    // Algorithm management routes
    let algorithm_routes = Router::new()
        .route("/", post(handlers::algorithm::create_algorithm))
        .route("/", get(handlers::algorithm::list_algorithms))
        .route("/{id}", get(handlers::algorithm::get_algorithm))
        .route("/{id}", put(handlers::algorithm::update_algorithm))
        .route("/{id}", delete(handlers::algorithm::delete_algorithm))
        .route(
            "/{id}/execute",
            post(handlers::algorithm::execute_algorithm),
        );

    // Algorithm execution routes
    let algo_exe_routes = Router::new()
        .route("/", post(handlers::submit_algo_exe))
        .route("/", get(handlers::list_algo_exes))
        .route("/{id}", get(handlers::get_algo_exe));

    // Dataset management routes
    let dataset_routes = Router::new()
        .route("/", post(handlers::dataset::create_dataset))
        .route("/", get(handlers::dataset::list_datasets))
        .route("/{id}", get(handlers::dataset::get_dataset))
        .route("/{id}", put(handlers::dataset::update_dataset))
        .route("/{id}", delete(handlers::dataset::delete_dataset))
        .route("/{id}/access", post(handlers::dataset::grant_access))
        .route("/{id}/access", delete(handlers::dataset::revoke_access));

    // Execution and results routes
    let execution_routes = Router::new()
        .route("/", get(handlers::execution::list_executions))
        .route("/{id}", get(handlers::execution::get_execution))
        .route(
            "/{id}/status",
            get(handlers::execution::get_execution_status),
        )
        .route(
            "/{id}/results",
            get(handlers::execution::get_execution_results),
        )
        .route("/{id}/cancel", post(handlers::execution::cancel_execution));

    // Committee and voting routes
    let committee_routes = Router::new()
        .route("/", get(handlers::committee::list_committee_members))
        .route("/", post(handlers::committee::set_committee_member))
        .route("/{id}", get(handlers::committee::get_committee_member))
        .route("/is-member", get(handlers::committee::is_committee_member));

    // Vote routes
    let vote_routes = Router::new()
        .route("/", get(handlers::vote::list_votes))
        .route(
            "/set-voting-duration",
            post(handlers::vote::set_voting_duration),
        );

    // Contract interaction routes
    let contract_routes = Router::new()
        .route("/sync", post(handlers::contract::trigger_sync))
        .route("/status", get(handlers::contract::get_sync_status))
        .route("/events", get(handlers::contract::list_events));

    // TEE attestation routes
    let attestation_routes = Router::new()
        .route(
            "/report",
            get(handlers::attestation::get_attestation_report),
        )
        .route("/verify", post(handlers::attestation::verify_attestation))
        .route("/quote", get(handlers::attestation::get_quote));

    // Protected API routes (require authentication)
    let api_routes = Router::new()
        .nest("/algorithms", algorithm_routes)
        .nest("/algoexes", algo_exe_routes)
        .nest("/datasets", dataset_routes)
        .nest("/executions", execution_routes)
        .nest("/committee", committee_routes)
        .nest("/votes", vote_routes)
        .nest("/contracts", contract_routes)
        .nest("/attestation", attestation_routes);

    // Build the complete application
    Router::new()
        // Health routes (no auth needed)
        .merge(health_routes)
        // API routes (auth required)
        .nest("/api/v1", api_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .fallback(handlers::not_found::handler)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::init_config;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        // Load real configuration from environment variables (.env file)
        let config = init_config().expect("Failed to load test configuration");

        // Create database connection using real config
        let db = Database::new(&config.database)
            .await
            .expect("Failed to connect to test database");

        // Create app with real dependencies
        let app = create_app(db, config).await;

        // Test health endpoint
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Parse response body
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Verify response structure
        assert_eq!(json["status"], "healthy");
        assert!(json["timestamp"].is_string());
    }
}
