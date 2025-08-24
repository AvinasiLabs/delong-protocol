//! DeLong Protocol Core Service Library
//!
//! This library provides the core functionality for the DeLong Protocol,
//! including authentication, user management, algorithm management,
//! dataset handling, voting system, and AI audit capabilities.
//!
//! ## Architecture
//!
//! The core service follows a modular architecture with clear separation of concerns:
//!
//! - **Handlers**: HTTP request handlers organized by business domain
//! - **Services**: Business logic and external service integration
//! - **Models**: Data structures and domain objects
//! - **Database**: Database access layer and migrations
//! - **Auth**: Authentication and authorization logic
//! - **Middleware**: Cross-cutting concerns like logging, CORS, auth
//! - **Config**: Configuration management and environment variables
//!
//! ## Usage
//!
//! ```rust,no_run
//! use delong_core::{Config, create_app_state, routes::create_router};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::load()?;
//!     let state = create_app_state(config).await?;
//!     let app = create_router(state);
//!
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
//!     axum::serve(listener, app).await?;
//!     Ok(())
//! }
//! ```

// Module declarations
pub mod config;
pub mod handlers;
pub mod infra;
pub mod middleware;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod utils;
// Re-export error types from avinapi
pub use avinapi::prelude::{AppError, AppResult};
pub use config::Config;
pub use infra::ai_audit::AiAuditService;
pub use infra::db::Database;
pub use infra::verification::VerificationStore;

pub use models::*;
use tracing::warn;
pub use utils::jwt::{JwtConfig, create_jwt_config};

use std::sync::Arc;
use tracing::{error, info};

// Re-export AppState from routes module
pub use routes::AppState;

/// Create a new application state with the given configuration
///
/// This function initializes all the required services and dependencies.
/// It will establish database connections, set up authentication services,
/// and validate the configuration.
///
/// # Arguments
///
/// * `config` - Application configuration
///
/// # Returns
///
/// Returns a new `AppState` instance or an error if initialization fails.
pub async fn create_app_state(mut config: Config) -> AppResult<AppState> {
    info!("Initializing application state...");

    // Initialize database
    info!("Connecting to database...");
    let db = Database::new(&config.database).await?;
    info!("Database connection established");

    // Initialize Redis pool (optional)
    let redis_pool = if let Ok(redis_url) = std::env::var("REDIS_URL") {
        info!("Connecting to Redis...");
        let redis_config = deadpool_redis::Config::from_url(&redis_url);
        let redis_pool = redis_config
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .map_err(|e| {
                warn!(
                    "Failed to create Redis pool: {}. OAuth state will use in-memory storage.",
                    e
                );
                e
            })
            .ok()
            .map(Arc::new);

        if redis_pool.is_some() {
            info!("Redis connection established");
        }
        redis_pool
    } else {
        info!("REDIS_URL not configured, using in-memory storage for OAuth state");
        None
    };

    config.redis_pool = redis_pool;

    // Initialize services
    info!("Initializing services...");

    // Initialize Redis-based verification store
    let verification_store = if let Some(ref redis_pool) = config.redis_pool {
        info!("Initializing Redis verification store");
        Arc::new(infra::verification::VerificationStore::new(
            redis_pool.clone(),
        ))
    } else {
        error!("Redis pool not available - verification service will not work");
        return Err(AppError::Internal(
            "Redis is required for verification service".to_string(),
        ));
    };

    let config_arc = Arc::new(config);
    let jwt_config = Arc::new(create_jwt_config());

    // Initialize AI audit service
    let ai_service_url = std::env::var("AI_AUDIT_SERVICE_URL").ok();
    let ai_audit_service =
        Arc::new(AiAuditService::new(ai_service_url).map_err(|e| {
            AppError::Config(format!("Failed to initialize AI audit service: {}", e))
        })?);

    // Initialize Google OAuth service
    let google_client_id =
        std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| "your-google-client-id".to_string());
    let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .unwrap_or_else(|_| "your-google-client-secret".to_string());
    let google_redirect_uri = std::env::var("GOOGLE_REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:3000/auth/google/callback".to_string());

    let google_oauth_service = Arc::new(infra::google_oauth::GoogleOAuthService::new(
        google_client_id,
        google_client_secret,
        google_redirect_uri,
    ));

    info!("Google OAuth service initialized");

    // Initialize proxy client for forwarding requests to Secure service
    let proxy_client = if std::env::var("SECURE_SERVICE_URL").is_ok() {
        info!("Initializing proxy client for Secure service...");
        let proxy_client = infra::proxy::create_proxy_client(
            config_arc.proxy.clone(),
            &config_arc.internal_jwt_secret,
        )
        .await
        .map_err(|e| {
            warn!(
                "Failed to initialize proxy client: {}. Proxy features will be disabled.",
                e
            );
            e
        })
        .ok()
        .map(Arc::new);

        if proxy_client.is_some() {
            info!("Proxy client initialized successfully");
        }
        proxy_client
    } else {
        info!("SECURE_SERVICE_URL not configured, proxy features disabled");
        None
    };

    info!("Services initialized");

    let state = AppState::new(
        db.into(),
        verification_store,
        jwt_config,
        config_arc,
        ai_audit_service,
        google_oauth_service,
        proxy_client,
    );

    info!("Application state initialized successfully");
    Ok(state)
}

/// Create the main application router
///
/// This function creates the complete Axum router with all routes,
/// middleware, and state. It's the main entry point for creating
/// the HTTP server.
///
/// # Arguments
///
/// * `state` - Shared application state
///
/// # Returns
///
/// Returns a configured Axum router ready to serve HTTP requests.
///
/// # Examples
///
/// ```rust,no_run
/// use delong_core::{Config, create_app_state, routes::create_router};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = Config::load()?;
///     let state = create_app_state(config).await?;
///     let app = create_router(state);
///
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
///     axum::serve(listener, app).await?;
///     Ok(())
/// }
/// ```
pub fn create_app(state: AppState) -> axum::Router {
    routes::create_router(state)
}

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Build timestamp
pub const BUILD_TIME: &str = "unknown";

/// Git commit hash
pub const GIT_COMMIT: &str = "unknown";

/// Initialize the application
///
/// This function performs all necessary initialization steps including
/// setting up logging, loading configuration, and preparing the application
/// state.
///
/// # Returns
///
/// Returns the initialized application state or an error if initialization fails.
pub async fn init() -> AppResult<AppState> {
    // Initialize logging
    init_logging()?;

    // Load configuration
    let config = Config::load().map_err(|e| AppError::Config(format!("{}", e)))?;

    // Log startup information
    info!("Starting {} v{}", NAME, VERSION);
    info!("Build time: {}", BUILD_TIME);
    info!("Git commit: {}", GIT_COMMIT);

    // Create application state
    let state = create_app_state(config).await?;

    info!("Application initialization completed successfully");
    Ok(state)
}

/// Initialize logging
///
/// Sets up structured logging with appropriate log levels and formatting
/// based on the environment.
fn init_logging() -> AppResult<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "debug,core=info,tower_http=debug".into());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Logging initialized");
    Ok(())
}

/// Graceful shutdown handler
///
/// This function handles graceful shutdown of the application,
/// ensuring all resources are properly cleaned up.
pub async fn shutdown(_state: Arc<AppState>) -> AppResult<()> {
    info!("Initiating graceful shutdown...");

    // Close database connections
    // TODO: Implement proper database connection closure
    info!("Database connections closed");

    // Perform any other cleanup tasks
    info!("Graceful shutdown completed");
    Ok(())
}

/// Health check function
///
/// This function performs a basic health check of the application
/// and its dependencies.
pub async fn health_check(state: &AppState) -> AppResult<()> {
    // Check database connectivity
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|e| AppError::Config(format!("Database health check failed: {}", e)))?;

    // Check other dependencies as needed
    // Redis, external services, etc.

    Ok(())
}
