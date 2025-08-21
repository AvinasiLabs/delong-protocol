//! Routes configuration for the core service
//!
//! This module sets up all HTTP routes, middleware, and application state
//! for the core service.

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_scalar::Scalar;

use sqlx::PgPool;
use std::sync::Arc;

use crate::{
    Config, handlers,
    infra::{
        ai_audit::AiAuditService, google_oauth::GoogleOAuthService, proxy::ProxyClient,
        verification::VerificationStore,
    },
    middleware::{
        combined_auth_middleware, jwt_auth_middleware, rate_limit_middleware,
        request_id_middleware, response_transformer,
    },
    openapi::ApiDoc,
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
    /// Google OAuth service for authentication
    pub google_oauth_service: Arc<GoogleOAuthService>,
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
        google_oauth_service: Arc<GoogleOAuthService>,
        proxy_client: Option<Arc<ProxyClient>>,
    ) -> Self {
        Self {
            db,
            verification_store,
            jwt_config,
            config,
            ai_audit_service,
            google_oauth_service,
            proxy_client,
        }
    }
}

/// Create the main application router with all routes and middleware
pub fn create_router(state: AppState) -> Router {
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

    // Protected AI audit routes
    let ai_audit_routes = Router::new()
        .route("/", post(handlers::ai_audit::create_ai_audit))
        .route("/", get(handlers::ai_audit::get_ai_audit_reports))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // Protected API key routes
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

    // Protected admin routes
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

    // Protected proxy routes to Secure service (if configured)
    let secure_proxy_routes = if state.proxy_client.is_some() {
        Router::new()
            .nest("/algoexes", handlers::proxy::proxy_routes())
            .nest("/datasets", handlers::proxy::proxy_routes())
            .nest("/committee", handlers::proxy::proxy_routes())
            .nest("/votes", handlers::proxy::proxy_routes())
            .nest("/contracts", handlers::proxy::proxy_routes())
            .route("/secure/health", get(handlers::proxy::secure_health_check))
            .layer(axum::middleware::from_fn(
                handlers::proxy::extract_auth_method,
            ))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                combined_auth_middleware,
            ))
    } else {
        Router::new()
    };

    // Build the complete application
    Router::new()
        // Public routes (no authentication required)
        .route("/", get(handlers::root_handler))
        .route("/health", get(handlers::health::health))
        // Public auth routes
        .route("/auth/register", post(handlers::auth::register_user))
        .route("/auth/login", post(handlers::auth::login_user))
        .route(
            "/auth/send-code",
            post(handlers::auth::send_verification_code),
        )
        .route("/auth/google", post(handlers::auth::google_auth_url))
        .route(
            "/auth/google/callback",
            get(handlers::auth::google_auth_callback),
        )
        // Public API key validation
        .route(
            "/api/api-keys/validate",
            post(handlers::api_key::validate_api_key),
        )
        // API Documentation routes
        .route(
            "/api-docs/openapi.json",
            get(|| async { axum::Json(ApiDoc::openapi()) }),
        )
        // Protected routes
        .nest("/api/user", user_routes)
        .nest("/api/ai-audit", ai_audit_routes)
        .nest("/api/api-keys", api_key_routes)
        .nest("/api", secure_proxy_routes)
        .nest("/admin", admin_routes)
        // Serve Scalar API documentation UI
        .route(
            "/scalar",
            get(|| async {
                use axum::response::Html;
                let spec = ApiDoc::openapi();
                let html = Scalar::new(serde_json::to_value(spec).unwrap()).to_html();
                Html(html)
            }),
        )
        .layer(axum::middleware::from_fn(response_transformer))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
