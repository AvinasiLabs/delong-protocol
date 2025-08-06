//! Routes configuration for the secure service
//!
//! This module sets up all HTTP routes, middleware, and application state
//! for the secure service running in the TEE environment.

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    routing::{get, post},
    Router,
};
use ipfs_api_backend_hyper::TryFromUri;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    config::Config,
    handlers,
    infra::ws::{ws_handler, Hub},
    infra::{
        contracts::ContractCaller,
        db::Database,
        tee::{TeeConfig as InfraTeeConfig, TeeService},
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
    /// WebSocket hub for real-time notifications
    pub ws_hub: Arc<Hub>,
    /// TEE service for secure operations
    pub tee_service: Arc<TeeService>,
}

impl AppState {
    /// Create new application state
    pub fn new(
        db: Database,
        config: Config,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient,
        contract_caller: ContractCaller,
        ws_hub: Arc<Hub>,
        tee_service: Arc<TeeService>,
    ) -> Self {
        Self {
            db,
            config,
            ipfs_client,
            contract_caller,
            ws_hub,
            tee_service,
        }
    }
}

/// Create the main application router with all routes and middleware
pub async fn create_app(db: Database, config: Config, ws_hub: Arc<Hub>) -> Router {
    // Initialize IPFS client
    let ipfs_client = ipfs_api_backend_hyper::IpfsClient::from_str(&config.ipfs.api_url)
        .expect("Failed to create IPFS client");

    // Initialize contract caller
    // Initialize TEE service
    let tee_config = InfraTeeConfig {
        endpoint: config
            .tee
            .enabled
            .then(|| "/var/run/dstack.sock".to_string()),
    };
    let tee_service = Arc::new(TeeService::new(tee_config));

    let contract_caller = ContractCaller::new(config.chain.clone())
        .await
        .expect("Failed to create contract caller");

    let state = AppState::new(
        db,
        config,
        ipfs_client,
        contract_caller,
        ws_hub,
        tee_service,
    );

    // Health check routes (no authentication)
    let health_routes = Router::new().route("/health", get(handlers::health::health_check));

    // Algorithm execution routes
    let algo_exe_routes = Router::new()
        .route("/", post(handlers::submit_algo_exe))
        .route("/", get(handlers::list_algo_exes))
        .route("/{id}", get(handlers::get_algo_exe));

    // Static dataset routes
    let static_dataset_routes = Router::new()
        .route("/", post(handlers::create_static_dataset))
        .route("/", get(handlers::list_static_datasets));

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

    // Contract metadata routes
    let contract_routes = Router::new().route("/", get(handlers::contract::list_contracts));

    // WebSocket route handler
    let ws_route_handler = |Path(task_id): Path<String>,
                            ws: WebSocketUpgrade,
                            State(state): State<AppState>| async move {
        ws_handler(ws, task_id, state.ws_hub.clone()).await
    };

    // Protected API routes (require authentication)
    let api_routes = Router::new()
        .nest("/algoexes", algo_exe_routes)
        .nest("/static-datasets", static_dataset_routes)
        .nest("/committee", committee_routes)
        .nest("/votes", vote_routes)
        .nest("/contracts", contract_routes);

    // Build the complete application
    Router::new()
        // Health routes (no auth needed)
        .merge(health_routes)
        // API routes (auth required)
        .nest("/api", api_routes)
        // WebSocket endpoint
        .route("/ws/{task_id}", get(ws_route_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .fallback(handlers::not_found_handler)
        .with_state(state)
}
