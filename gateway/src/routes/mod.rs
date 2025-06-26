use axum::{
    Router, middleware,
    routing::{delete, get, post},
};
use tower_http::cors::CorsLayer;

use crate::{
    config::GatewayConfig,
    handlers::{
        algorithm::{
            get_algorithm_handler, get_algorithm_result_handler, get_algorithm_status_handler,
            submit_algorithm_handler,
        },
        auth::{
            create_api_key_handler, list_api_keys_handler, revoke_api_key_handler,
            validate_api_key_handler,
        },
        data::{
            delete_dataset_handler, get_dataset_handler, get_dataset_list_handler,
            upload_dataset_handler,
        },
        health::{health_handler, liveness_handler, readiness_handler},
        // metrics::{metrics_handler, prometheus_metrics_handler},
    },
    middleware::{
        auth::auth_middleware, logging::logging_middleware, request_id::request_id_middleware,
    },
};

/// Create the main application router
pub fn create_router(config: &GatewayConfig) -> Router {
    let api_routes = create_api_routes(config);

    Router::new()
        // Health check routes (no auth required)
        .route("/health", get(health_handler))
        .route("/health/live", get(liveness_handler))
        .route("/health/ready", get(readiness_handler))
        // Metrics route (no auth required for monitoring)
        // .route("/metrics", get(metrics_handler))
        // Prometheus metrics endpoint
        // .route("/metrics/prometheus", get(prometheus_metrics_handler))
        // API routes with authentication
        .nest("/api/v1", api_routes)
        // Global middleware
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn(logging_middleware))
        .layer(CorsLayer::permissive()) // Configure CORS as needed
}

/// Create API routes with authentication
fn create_api_routes(config: &GatewayConfig) -> Router {
    let mut api_router = Router::new();

    // Data management routes
    api_router = api_router.nest("/datasets", create_dataset_routes());

    // Algorithm management routes
    api_router = api_router.nest("/algorithms", create_algorithm_routes());

    // API key management routes (admin only)
    api_router = api_router.nest("/auth", create_auth_routes());

    // Apply authentication middleware if enabled
    if config.api.enable_api_key_validation {
        api_router = api_router.layer(middleware::from_fn(auth_middleware));
    }

    api_router
}

/// Create dataset management routes
fn create_dataset_routes() -> Router {
    Router::new()
        // Upload new dataset
        .route("/", post(upload_dataset_handler))
        // List user's datasets
        .route("/", get(get_dataset_list_handler))
        // Get specific dataset info
        .route("/{dataset_id}", get(get_dataset_handler))
        // Delete dataset
        .route("/{dataset_id}", delete(delete_dataset_handler))
}

/// Create algorithm management routes
fn create_algorithm_routes() -> Router {
    Router::new()
        // Submit new algorithm for execution
        .route("/submit", post(submit_algorithm_handler))
        // Get algorithm execution status
        .route("/{algorithm_id}/status", get(get_algorithm_status_handler))
        // Get algorithm execution result
        .route("/{algorithm_id}/result", get(get_algorithm_result_handler))
        // Get algorithm details
        .route("/{algorithm_id}", get(get_algorithm_handler))
}

/// Create authentication/API key management routes
fn create_auth_routes() -> Router {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GatewayConfig;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = GatewayConfig::default();
        let app = create_router(&config);

        let request = Request::builder()
            .uri("/health")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // #[tokio::test]
    // async fn test_metrics_endpoint() {
    //     let config = GatewayConfig::default();
    //     let app = create_router(&config);

    //     let request = Request::builder()
    //         .uri("/metrics")
    //         .method(Method::GET)
    //         .body(Body::empty())
    //         .unwrap();

    //     let response = app.oneshot(request).await.unwrap();
    //     assert_eq!(response.status(), StatusCode::OK);
    // }
}
