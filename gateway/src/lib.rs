//! Delong Protocol Gateway Library
//!
//! This library provides the core functionality for the Delong Protocol Gateway,
//! including middleware, handlers, and utilities for API request processing.

pub mod cache;
pub mod config;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod utils;

// Re-export commonly used items for convenience
pub use config::GatewayConfig;
pub use middleware::MiddlewareUtils;
pub use routes::{create_auth_test_router, create_router, create_test_router};
pub use utils::http_client::{BackendClient, HttpBackendClient};

// Re-export testing utilities
// Note: Specific imports instead of glob to avoid naming conflicts
#[cfg(test)]
pub use handlers::{auth as handlers_auth, dataset, health};
#[cfg(test)]
pub use middleware::{auth as middleware_auth, logging_middleware, request_id_middleware};
