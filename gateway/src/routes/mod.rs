use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::{
    config::GatewayConfig,
    handlers::{
        algo_exe::{get_algo_exe_handler, get_algo_exes_handler, submit_algo_exe_handler},
        auth::{
            create_api_key_handler, list_api_keys_handler, revoke_api_key_handler,
            validate_api_key_handler,
        },
        committee::{
            check_committee_membership_handler, get_committee_member_handler,
            get_committee_members_handler, set_committee_member_handler,
        },
        contract::get_contracts_handler,
        dataset::{
            // Dynamic datasets handlers
            create_dataset_handler,
            delete_dataset_handler,
            // Static datasets handlers
            delete_static_dataset_handler,
            get_datasets_handler,
            get_sample_data_handler,
            get_static_dataset_handler,
            get_static_datasets_handler,
            update_dataset_handler,
            update_static_dataset_handler,
            upload_static_dataset_handler,
        },
        health::health_handler,
        report::upload_report_handler,
        vote::{get_votes_handler, set_vote_duration_handler},
        websocket::websocket_handler,
    },
    middleware::{
        auth::auth_middleware, logging::logging_middleware, request_id::request_id_middleware,
    },
    utils::http_client::BackendClient,
};

/// Application state containing configuration and HTTP client
#[derive(Clone)]
pub struct AppState {
    pub config: GatewayConfig,
    pub http_client: Arc<dyn BackendClient>,
}

/// Create the main application router
pub fn create_router(config: &GatewayConfig, http_client: impl BackendClient + 'static) -> Router {
    let app_state = AppState {
        config: config.clone(),
        http_client: Arc::new(http_client),
    };

    let api_routes = create_api_routes(&app_state);

    Router::new()
        // Health check routes (no auth required)
        .route("/health", get(health_handler))
        // API routes with authentication
        .nest("/api", api_routes)
        // Global middleware
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn(logging_middleware))
        .layer(CorsLayer::permissive()) // Configure CORS as needed
        // Add application state
        .with_state(app_state)
}

/// Create API routes with authentication
fn create_api_routes(state: &AppState) -> Router<AppState> {
    // Apply authentication middleware if enabled
    if state.config.api.enable_api_key_validation {
        // Apply auth middleware to all routes except sample data access
        let protected_routes = Router::new()
            .nest("/static-datasets", create_static_dataset_routes(state))
            .nest("/datasets", create_dynamic_dataset_routes(state))
            .nest("/algo-exes", create_algo_exe_routes(state))
            .nest("/committee", create_committee_routes(state))
            .nest("/votes", create_vote_routes(state))
            .nest("/contracts", create_contract_routes(state))
            .nest("/reports", create_report_routes(state))
            .nest("/auth", create_auth_routes(state))
            .layer(middleware::from_fn(auth_middleware));

        Router::new()
            .merge(protected_routes)
            .route("/sample/{cid}", get(get_sample_data_handler))
            .route("/ws", get(websocket_handler))
    } else {
        Router::new()
            .nest("/static-datasets", create_static_dataset_routes(state))
            .nest("/datasets", create_dynamic_dataset_routes(state))
            .nest("/algo-exes", create_algo_exe_routes(state))
            .nest("/committee", create_committee_routes(state))
            .nest("/votes", create_vote_routes(state))
            .nest("/contracts", create_contract_routes(state))
            .nest("/reports", create_report_routes(state))
            .nest("/auth", create_auth_routes(state))
            .route("/sample/{cid}", get(get_sample_data_handler))
            .route("/ws", get(websocket_handler))
    }
}

/// Create static dataset management routes
/// These routes handle encrypted datasets stored on IPFS with blockchain records
fn create_static_dataset_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // Upload new static dataset (multipart/form-data)
        .route("/", post(upload_static_dataset_handler))
        // List static datasets with pagination
        .route("/", get(get_static_datasets_handler))
        // Get specific static dataset info
        .route("/{id}", get(get_static_dataset_handler))
        // Update static dataset metadata
        .route("/{id}", put(update_static_dataset_handler))
        // Delete static dataset
        .route("/{id}", delete(delete_static_dataset_handler))
}

/// Create dynamic dataset management routes
/// These routes handle mutable datasets stored locally with version management
fn create_dynamic_dataset_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // Create new dynamic dataset (admin only)
        .route("/", post(create_dataset_handler))
        // List dynamic datasets with pagination
        .route("/", get(get_datasets_handler))
        // Update dynamic dataset
        .route("/{id}", put(update_dataset_handler))
        // Delete dynamic dataset
        .route("/{id}", delete(delete_dataset_handler))
}

/// Create authentication/API key management routes
fn create_auth_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // Create new API key
        .route("/keys", post(create_api_key_handler))
        // List API keys
        .route("/keys", get(list_api_keys_handler))
        // Validate API key
        .route("/keys/validate", post(validate_api_key_handler))
        // Revoke API key
        .route("/keys/{key_id}", delete(revoke_api_key_handler))
}

/// Create algorithm execution management routes
/// These routes handle algorithm submission, execution tracking, and results
fn create_algo_exe_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // Submit new algorithm execution
        .route("/", post(submit_algo_exe_handler))
        // List algorithm executions with pagination
        .route("/", get(get_algo_exes_handler))
        // Get specific algorithm execution details
        .route("/{id}", get(get_algo_exe_handler))
}

/// Create committee management routes
/// These routes handle committee member operations and membership checks
fn create_committee_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // Set committee member (add/update)
        .route("/", post(set_committee_member_handler))
        // List committee members with pagination
        .route("/", get(get_committee_members_handler))
        // Get specific committee member details
        .route("/{id}", get(get_committee_member_handler))
        // Check committee membership by wallet address
        .route("/check/{wallet}", get(check_committee_membership_handler))
}

/// Create voting management routes
/// These routes handle voting operations and configuration
fn create_vote_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // List votes with pagination
        .route("/", get(get_votes_handler))
        // Set vote duration configuration
        .route("/duration", post(set_vote_duration_handler))
}

/// Create contract metadata routes
/// These routes handle smart contract information
fn create_contract_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // List contracts with pagination
        .route("/", get(get_contracts_handler))
}

/// Create test report management routes
/// These routes handle test report uploads and processing
fn create_report_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        // Upload test report
        .route("/", post(upload_report_handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::GatewayConfig, utils::http_client::HttpBackendClient};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = GatewayConfig::default();
        let client = HttpBackendClient::new(30);
        let app = create_router(&config, client);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_sample_data_public_access() {
        let config = GatewayConfig::default();
        let client = HttpBackendClient::new(30);
        let app = create_router(&config, client);

        let request = Request::builder()
            .uri("/api/sample/QmSampleTestCID")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        // This should not return 401 since sample data is public
        let response = app.oneshot(request).await.unwrap();
        // Note: Will likely return 500 or other error in tests due to missing backend,
        // but should not be 401 Unauthorized
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_static_datasets_requires_auth_when_enabled() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = true;
        let client = HttpBackendClient::new(30);
        let app = create_router(&config, client);

        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should return 401 when auth is enabled and no API key provided
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_disabled_allows_access() {
        let mut config = GatewayConfig::default();
        config.api.enable_api_key_validation = false;
        let client = HttpBackendClient::new(30);
        let app = create_router(&config, client);

        let request = Request::builder()
            .uri("/api/static-datasets")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should not return 401 when auth is disabled
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
