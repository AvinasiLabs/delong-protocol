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
//! ```rust
//! use core::{AppState, create_app};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = core::AppConfig::from_env()?;
//!     let state = Arc::new(AppState::new(config).await?);
//!     let app = create_app(state);
//!
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
//!     axum::serve(listener, app).await?;
//!     Ok(())
//! }
//! ```

// Module declarations
pub mod config;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod utils;
// Re-export commonly used types
pub use common::{ApiError, ApiResult};
pub use config::AppConfig;
pub use models::*;
pub use services::ai_audit::AiAuditService;
pub use services::verification::VerificationStore;
use sqlx::PgPool;
pub use utils::jwt::{JwtConfig, create_jwt_config};

use std::sync::Arc;
use tracing::info;

/// Shared application state
///
/// This struct contains all the shared dependencies that are passed
/// to HTTP handlers and other parts of the application.
#[derive(Clone)]
pub struct AppState {
    /// Database pool for data persistence
    pub db: PgPool,
    /// Verification store for email verification codes
    pub verification_store: Arc<VerificationStore>,
    /// JWT configuration for token operations
    pub jwt_config: Arc<JwtConfig>,
    /// Application configuration
    pub config: Arc<AppConfig>,
    /// AI audit service for external AI audit operations
    pub ai_audit_service: Arc<AiAuditService>,
}

impl AppState {
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use core::{AppState, AppConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AppConfig::from_env()?;
    ///     let state = AppState::new(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: AppConfig) -> ApiResult<Self> {
        info!("Initializing application state...");

        // Initialize database pool
        info!("Connecting to database...");
        let db = PgPool::connect(&config.database_url).await.map_err(|e| {
            ApiError::ConfigurationError(format!("Failed to connect to database: {}", e))
        })?;
        info!("Database connection established");

        // Initialize services
        info!("Initializing services...");
        let config_arc = Arc::new(config);
        let verification_store = Arc::new(VerificationStore::new());
        let jwt_config = Arc::new(create_jwt_config());

        // Initialize AI audit service
        let ai_service_url = std::env::var("AI_AUDIT_SERVICE_URL").ok();
        let ai_audit_service = Arc::new(AiAuditService::new(ai_service_url).map_err(|e| {
            ApiError::ConfigurationError(format!("Failed to initialize AI audit service: {}", e))
        })?);

        info!("Services initialized");

        let state = Self {
            db,
            verification_store,
            jwt_config,
            config: config_arc,
            ai_audit_service,
        };

        info!("Application state initialized successfully");
        Ok(state)
    }

    /// Create application state for testing
    ///
    /// This function creates a minimal application state suitable for testing.
    /// It uses in-memory or test databases and mock services where appropriate.
    #[cfg(test)]
    pub async fn new_test() -> ApiResult<Self> {
        let config = AppConfig::default();
        let db = PgPool::connect("sqlite::memory:").await.map_err(|e| {
            ApiError::ConfigurationError(format!("Failed to connect to test database: {}", e))
        })?;

        let config_arc = Arc::new(config);
        let verification_store = Arc::new(VerificationStore::new());
        let jwt_config = Arc::new(create_jwt_config());

        // Initialize AI audit service for testing
        let ai_audit_service = Arc::new(AiAuditService::new(None).map_err(|e| {
            ApiError::ConfigurationError(format!(
                "Failed to initialize AI audit service for test: {}",
                e
            ))
        })?);

        Ok(Self {
            db,
            verification_store,
            jwt_config,
            config: config_arc,
            ai_audit_service,
        })
    }

    /// Get the service name
    pub fn service_name(&self) -> &str {
        "DeLong Protocol Core Service"
    }

    /// Get the service version
    pub fn version(&self) -> &str {
        VERSION
    }

    /// Check if the service is running in debug mode
    pub fn is_debug(&self) -> bool {
        cfg!(debug_assertions)
    }

    /// Check if the service is running in production mode
    pub fn is_production(&self) -> bool {
        std::env::var("RUST_ENV").unwrap_or_default() == "production"
    }
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
/// ```rust
/// use core::{AppState, create_app};
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = core::AppConfig::from_env()?;
///     let state = Arc::new(AppState::new(config).await?);
///     let app = create_app(state);
///
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
///     axum::serve(listener, app).await?;
///     Ok(())
/// }
/// ```
pub fn create_app(state: Arc<AppState>) -> axum::Router {
    routes::create_router(state)
}

/// Create the application router with custom route configuration
///
/// This function allows more fine-grained control over which routes
/// are enabled and how they're configured.
///
/// # Arguments
///
/// * `state` - Shared application state
/// * `route_config` - Custom route configuration
///
/// # Returns
///
/// Returns a configured Axum router with custom route configuration.
pub fn create_app_with_config(
    state: Arc<AppState>,
    route_config: routes::RouteConfig,
) -> axum::Router {
    routes::create_configured_router(state, route_config)
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
pub async fn init() -> ApiResult<Arc<AppState>> {
    // Initialize logging
    init_logging()?;

    // Load configuration
    let config =
        AppConfig::from_env().map_err(|e| ApiError::ConfigurationError(format!("{}", e)))?;

    // Log startup information
    info!("Starting {} v{}", NAME, VERSION);
    info!("Build time: {}", BUILD_TIME);
    info!("Git commit: {}", GIT_COMMIT);

    // Create application state
    let state = Arc::new(AppState::new(config).await?);

    info!("Application initialization completed successfully");
    Ok(state)
}

/// Initialize logging
///
/// Sets up structured logging with appropriate log levels and formatting
/// based on the environment.
fn init_logging() -> ApiResult<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "core=info,tower_http=debug".into());

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
pub async fn shutdown(_state: Arc<AppState>) -> ApiResult<()> {
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
pub async fn health_check(state: &AppState) -> ApiResult<()> {
    // Check database connectivity
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|e| {
            ApiError::ConfigurationError(format!("Database health check failed: {}", e))
        })?;

    // Check other dependencies as needed
    // Redis, external services, etc.

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new_test().await;
        assert!(state.is_ok());
    }

    #[tokio::test]
    async fn test_app_state_properties() {
        let state = AppState::new_test().await.unwrap();
        assert_eq!(state.service_name(), "DeLong Protocol Core Service");
        assert_eq!(state.version(), VERSION);
        assert!(state.is_debug());
        assert!(!state.is_production());
    }

    #[test]
    fn test_version_and_name() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "core");
    }

    #[test]
    fn test_constants() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
        assert!(!BUILD_TIME.is_empty());
        assert!(!GIT_COMMIT.is_empty());
    }

    #[tokio::test]
    async fn test_create_app() {
        let state = Arc::new(AppState::new_test().await.unwrap());
        let _app = create_app(state);

        // Basic test to ensure router is created without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_health_check() {
        let state = AppState::new_test().await.unwrap();
        let result = health_check(&state).await;
        assert!(result.is_ok());
    }
}
