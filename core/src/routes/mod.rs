//! Route definitions for the DeLong Protocol API
//!
//! This module defines all HTTP routes and applies appropriate middleware
//! for authentication, authorization, CORS, and other cross-cutting concerns.

use crate::handlers::root_handler;
use crate::{
    AppState,
    handlers::{admin, ai_audit, auth, health},
    middleware::{admin_middleware, auth_middleware, cors_middleware, logging_middleware},
};
use axum::{
    Router,
    http::Method,
    middleware,
    routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// Create the main application router with all routes and middleware
pub fn create_router(state: Arc<AppState>) -> Router {
    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(Any);

    // Build the router with all routes
    Router::new()
        // Root route
        .route("/", get(root_handler))
        // Health check route (no auth required)
        .route("/health", get(health::health_check))
        // Public authentication routes
        .nest("/auth", create_auth_routes())
        // Protected API routes
        .nest("/api", create_api_routes(state.clone()))
        // Admin routes
        .nest("/admin", create_admin_routes(state.clone()))
        // Apply global middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors)
                .layer(middleware::from_fn(logging_middleware)),
        )
        .with_state(state)
}

/// Create authentication routes (public)
fn create_auth_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(auth::register_user))
        .route("/login", post(auth::login_user))
        .route("/send-code", post(auth::send_verification_code))
        .route("/google", post(auth::google_auth_url))
        .route("/google/callback", get(auth::google_auth_callback))
        .route("/refresh", post(auth::refresh_token))
        .route("/health", get(auth::health_check))
}

/// Create protected API routes
fn create_api_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Protected auth routes
        .route("/auth/me", get(auth::get_current_user))
        .route("/auth/update-wallet", post(auth::update_wallet_address))
        .route("/auth/logout", post(auth::logout_user))
        // AI audit routes
        .route("/ai-audit", post(ai_audit::create_ai_audit))
        .route("/ai-audit", get(ai_audit::get_ai_audit_reports))
        .route("/ai-audit/report", get(ai_audit::get_ai_audit_report))
        .route("/ai-audit/list", get(ai_audit::list_ai_audit_reports))
        // Apply authentication middleware to all API routes
        .layer(middleware::from_fn_with_state(state, auth_middleware))
}

/// Create admin routes (admin only)
fn create_admin_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // User management routes
        .route("/users", get(admin::get_users))
        .route("/users", post(admin::create_user_admin))
        .route("/users/:id", get(admin::get_user_by_id))
        .route("/users/:id", put(admin::update_user))
        .route("/users/:id", delete(admin::delete_user))
        // Role and permission management routes
        .route("/roles", get(admin::get_roles))
        .route("/permissions", get(admin::get_permissions))
        .route(
            "/permissions/:user_id/:permission",
            get(admin::check_permission),
        )
        // API key management routes
        .route("/api-keys", get(admin::list_api_keys))
        .route("/api-keys", post(admin::create_api_key))
        .route("/api-keys/stats", get(admin::get_api_key_stats))
        .route("/api-keys/:id", delete(admin::revoke_api_key))
        // Apply authentication and admin middleware
        .layer(middleware::from_fn(admin_middleware))
        .layer(middleware::from_fn_with_state(state, auth_middleware))
}

/// Route configuration for different environments
pub struct RouteConfig {
    pub enable_debug_routes: bool,
    pub enable_websocket: bool,
    pub enable_public_api: bool,
    pub enable_admin_api: bool,
    pub api_version: String,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            enable_debug_routes: cfg!(debug_assertions),
            enable_websocket: true,
            enable_public_api: true,
            enable_admin_api: true,
            api_version: "v1".to_string(),
        }
    }
}

/// Create router with custom configuration
pub fn create_configured_router(state: Arc<AppState>, route_config: RouteConfig) -> Router {
    let mut router = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health::health_check));

    // Add versioned API routes
    let api_path = format!("/{}", route_config.api_version);
    router = router.nest(&api_path, create_v1_routes(state.clone()));

    // Apply global middleware
    router
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors_middleware(&state.config))
                .layer(middleware::from_fn(logging_middleware)),
        )
        .with_state(state)
}

/// Create v1 API routes
fn create_v1_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .nest("/auth", create_auth_routes())
        .nest("/api", create_api_routes(state.clone()))
        .nest("/admin", create_admin_routes(state))
}

#[cfg(test)]
mod tests {

    // #[tokio::test]
    // #[ignore] // Temporarily disabled due to TestServer issues
    // async fn test_health_route() {
    //     let state = Arc::new(AppState::new_test().await.unwrap());
    //     let app = create_router(state);
    //     let server = TestServer::new(app.into_make_service()).unwrap();

    //     let response = server.get("/health").await;
    //     assert_eq!(response.status_code(), 200);
    // }

    // #[tokio::test]
    // #[ignore] // Temporarily disabled due to TestServer issues
    // async fn test_route_structure() {
    //     let state = Arc::new(AppState::new_test().await.unwrap());
    //     let app = create_router(state);
    //     let server = TestServer::new(app.into_make_service()).unwrap();

    //     // Test that routes are properly mounted
    //     let response = server.get("/").await;
    //     assert_eq!(response.status_code(), 200);

    //     let response = server.get("/health").await;
    //     assert_eq!(response.status_code(), 200);
    // }
}
