//! Routes configuration for the secure service
//!
//! This module sets up all HTTP routes, middleware, and application state
//! for the secure service running in the TEE environment.

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    config::Config,
    handlers,
    infra::{
        contracts::ContractCaller, db::Database, Notifier, SampleGenerator, TeeClient,
        TeeCryptoService, TeeEthereum,
    },
    middleware::internal_jwt_middleware,
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
    pub contract_caller: Arc<ContractCaller>,
    /// Notifier for WebSocket notifications
    pub notifier: Arc<Notifier>,
    /// Redis connection pool for queue management
    pub redis: deadpool_redis::Pool,
    /// TEE service for secure operations
    pub tee_service: Arc<TeeClient>,
    /// TEE Ethereum manager for TEE-based Ethereum operations
    pub tee_ethereum: Arc<TeeEthereum>,
    /// TEE cryptographic service for data encryption
    pub tee_crypto: Arc<TeeCryptoService>,
    /// Sample generator service for dataset sampling
    pub sample_generator: Arc<SampleGenerator>,
}

impl AppState {
    /// Create new application state
    pub fn new(
        db: Database,
        config: Config,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient,
        contract_caller: Arc<ContractCaller>,
        notifier: Arc<Notifier>,
        redis: deadpool_redis::Pool,
        tee_service: Arc<TeeClient>,
        tee_ethereum: Arc<TeeEthereum>,
        tee_crypto: Arc<TeeCryptoService>,
        sample_generator: Arc<SampleGenerator>,
    ) -> Self {
        Self {
            db,
            config,
            ipfs_client,
            contract_caller,
            notifier,
            redis,
            tee_service,
            tee_ethereum,
            tee_crypto,
            sample_generator,
        }
    }
}

/// Create the main application router with all routes and middleware
pub async fn create_app(
    db: Database,
    config: Config,
    ipfs_client: Arc<ipfs_api_backend_hyper::IpfsClient>,
    contract_caller: Arc<ContractCaller>,
    notifier: Arc<Notifier>,
    tee_service: Arc<TeeClient>,
    tee_ethereum: Arc<TeeEthereum>,
    redis_pool: deadpool_redis::Pool,
) -> Router {
    // Create the TEE crypto service
    let tee_crypto = Arc::new(TeeCryptoService::new(tee_service.clone()));

    // Create the sample generator service
    let sample_generator = Arc::new(SampleGenerator::new(config.dataset.sample_api_url.clone()));

    // Save max upload size before moving config
    let max_upload_size_bytes = config.dataset.max_upload_size_mb * 1024 * 1024;

    let state = AppState::new(
        db,
        config,
        ipfs_client.as_ref().clone(),
        contract_caller,
        notifier.clone(),
        redis_pool,
        tee_service,
        tee_ethereum,
        tee_crypto,
        sample_generator,
    );

    // Health check routes (no authentication)
    let health_routes = Router::new().route("/health", get(handlers::health::health_check));

    // Algorithm execution routes
    let execution_routes = Router::new()
        .route("/", post(handlers::submit_execution))
        .route("/", get(handlers::list_executions))
        .route("/{id}", get(handlers::get_execution));

    // Dataset routes
    let dataset_routes = Router::new()
        .route("/", post(handlers::create_dataset))
        .route("/", get(handlers::list_datasets))
        .route("/search", get(handlers::dataset::search_datasets))
        .route("/featured", get(handlers::dataset::get_featured_datasets))
        .route("/trending", get(handlers::dataset::get_trending_datasets))
        .route("/tags", get(handlers::dataset::get_all_tags))
        .route("/tags/popular", get(handlers::dataset::get_popular_tags))
        .route("/by-tags", get(handlers::dataset::get_datasets_by_tags))
        .route("/slug/{slug}", get(handlers::dataset::get_dataset_by_slug))
        .route("/{id}", get(handlers::get_dataset))
        .route("/{id}/status", get(handlers::dataset::get_dataset_status))
        .route("/{id}", axum::routing::put(handlers::update_dataset))
        .route("/{id}", axum::routing::delete(handlers::delete_dataset))
        .route("/{id}/tags", get(handlers::dataset::get_dataset_tags))
        .route("/{id}/tags", post(handlers::dataset::add_dataset_tags))
        .route(
            "/{id}/tags",
            axum::routing::delete(handlers::dataset::remove_dataset_tags),
        )
        .route("/{id}/schema", get(handlers::dataset::get_dataset_schema))
        .route("/{id}/schema", post(handlers::dataset::set_dataset_schema))
        .route("/{id}/usage", get(handlers::dataset::get_dataset_usage));

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

    // Create WebSocket state
    let ws_state = handlers::WsState { notifier };

    // WebSocket routes
    let ws_routes = handlers::ws_routes().with_state(ws_state);

    // Protected API routes (require authentication)
    let api_routes = Router::new()
        .nest("/executions", execution_routes)
        .nest("/datasets", dataset_routes)
        .nest("/committee", committee_routes)
        .nest("/votes", vote_routes)
        .nest("/contracts", contract_routes)
        // Apply internal JWT verification middleware from Core service
        .layer(middleware::from_fn(internal_jwt_middleware));

    // Build the complete application
    Router::new()
        // Health routes (no auth needed)
        .merge(health_routes)
        // API routes (auth required)
        .nest("/api", api_routes)
        // WebSocket routes
        .merge(ws_routes)
        // Set request body limit from configuration for large CSV file uploads
        .layer(DefaultBodyLimit::max(max_upload_size_bytes))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .fallback(handlers::not_found_handler)
        .with_state(state)
}
