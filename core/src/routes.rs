//! Routes configuration for the core service
//!
//! This module sets up all HTTP routes, middleware, and application state
//! for the core service.

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use tower_http::cors::{Any, CorsLayer};

use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    Config, handlers,
    infra::{ai_audit::AiAuditService, proxy::ProxyClient, verification::VerificationStore},
    middleware::{
        combined_auth_middleware, jwt_auth_middleware, request_id_middleware, response_transformer,
    },
    utils::jwt::JwtConfig,
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Database pool for data persistence
    pub db: PgPool,
    /// Verification store for email verification codes
    pub verification_store: Arc<VerificationStore>,
    /// JWT configuration for token operations
    pub jwt_config: Arc<JwtConfig>,
    /// Application configuration
    pub config: Arc<Config>,
    /// AI audit service for external AI audit operations
    pub ai_audit_service: Arc<AiAuditService>,
    /// Proxy client for forwarding requests to Secure service
    pub proxy_client: Option<Arc<ProxyClient>>,
}

impl AppState {
    /// Create new application state
    pub fn new(
        db: PgPool,
        verification_store: Arc<VerificationStore>,
        jwt_config: Arc<JwtConfig>,
        config: Arc<Config>,
        ai_audit_service: Arc<AiAuditService>,
        proxy_client: Option<Arc<ProxyClient>>,
    ) -> Self {
        Self {
            db,
            verification_store,
            jwt_config,
            config,
            ai_audit_service,
            proxy_client,
        }
    }
}

/// Create the main application router with all routes and middleware
pub fn create_router(state: AppState) -> Router {
    // Health check routes (no authentication)
    let health_routes = Router::new()
        .route("/", get(handlers::root_handler))
        .route("/health", get(handlers::health::health_check));

    // Public authentication routes
    let auth_routes = Router::new()
        .route("/register", post(handlers::auth::register_user))
        .route("/login", post(handlers::auth::login_user))
        .route("/send-code", post(handlers::auth::send_verification_code))
        .route("/google", post(handlers::auth::google_auth_url))
        .route(
            "/google/callback",
            get(handlers::auth::google_auth_callback),
        );

    // Protected user routes
    let user_routes = Router::new()
        .route(
            "/update-wallet",
            post(handlers::auth::update_wallet_address),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // AI audit routes (protected)
    let ai_audit_routes = Router::new()
        .route("/", post(handlers::ai_audit::create_ai_audit))
        .route("/", get(handlers::ai_audit::get_ai_audit_reports))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // Admin routes (protected)
    let admin_routes = Router::new()
        .route("/users", get(handlers::admin::get_users))
        .route("/users", post(handlers::admin::create_user_admin))
        .route("/users/{id}", get(handlers::admin::get_user_by_id))
        .route("/users/{id}", put(handlers::admin::update_user))
        .route("/roles", get(handlers::admin::get_roles))
        .route("/permissions", get(handlers::admin::get_permissions))
        .route(
            "/permissions/{user_id}/{permission}",
            get(handlers::admin::check_permission),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // API key routes (protected)
    let api_key_routes = Router::new()
        .route("/", get(handlers::api_key::list_user_api_keys))
        .route("/", post(handlers::api_key::create_api_key))
        .route("/stats", get(handlers::api_key::get_user_api_key_stats))
        .route("/{id}", get(handlers::api_key::get_api_key))
        .route("/{id}", put(handlers::api_key::update_api_key))
        .route("/{id}", delete(handlers::api_key::revoke_api_key))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // Public API key validation route (no auth needed for secure module)
    let api_key_validate_routes =
        Router::new().route("/validate", post(handlers::api_key::validate_api_key));

    // Protected API routes
    let api_routes = Router::new()
        .nest("/user", user_routes)
        .nest("/ai-audit", ai_audit_routes)
        .nest("/api-keys", api_key_routes);

    // Proxy routes to Secure service (protected)
    // These routes are forwarded to the Secure service running in TEE
    let secure_proxy_routes = if state.proxy_client.is_some() {
        Router::new()
            // Forward all algorithm execution requests
            .nest("/algoexes", handlers::proxy::proxy_routes())
            // Forward all dataset-related requests
            .nest("/datasets", handlers::proxy::proxy_routes())
            // Forward all committee-related requests
            .nest("/committee", handlers::proxy::proxy_routes())
            // Forward all vote-related requests
            .nest("/votes", handlers::proxy::proxy_routes())
            // Forward all contract-related requests
            .nest("/contracts", handlers::proxy::proxy_routes())
            // Health check for Secure service
            .route("/secure/health", get(handlers::proxy::secure_health_check))
            // Apply combined authentication middleware (JWT + API Key)
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                combined_auth_middleware,
            ))
            // Apply auth method extraction middleware
            .layer(axum::middleware::from_fn(
                handlers::proxy::extract_auth_method,
            ))
    } else {
        // If proxy client is not configured, return empty router
        Router::new()
    };

    // Build the complete application
    Router::new()
        // Health routes (no auth needed)
        .merge(health_routes)
        // Public auth routes
        .nest("/auth", auth_routes)
        // Public API key validation route
        .nest("/api/api-keys", api_key_validate_routes)
        // Protected API routes
        .nest("/api", api_routes)
        // Proxy routes to Secure service (if configured)
        .nest("/api", secure_proxy_routes)
        // Admin routes
        .nest("/admin", admin_routes)
        // Apply global middleware
        .layer(axum::middleware::from_fn(response_transformer))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
