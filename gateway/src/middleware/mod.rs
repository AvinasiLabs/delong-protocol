//! Middleware modules for the Delong gateway
//!
//! This module provides middleware for gateway functionality:
//! - Authentication: API key validation and user authentication (gateway-specific)
//! - Logging: Request/response logging with structured data (from common)
//! - Request ID: Unique request identifier for tracing (from common)

// Gateway-specific middleware
pub mod auth;

// Re-export middleware from common crate
pub use common::middleware::{
    MiddlewareUtils, REQUEST_ID_HEADER,
    logging::{LoggingConfig, logging_middleware, security_logging_middleware},
    request_id::{
        RequestIdConfig, RequestIdGenerator, generate_request_id, get_current_request_id,
        get_request_id_from_headers, request_id_middleware, request_id_middleware_with_config,
    },
};

// Re-export for backward compatibility
pub use common::middleware::logging::log_large_request;
pub use common::middleware::logging::log_slow_request;
