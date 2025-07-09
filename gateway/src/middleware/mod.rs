//! Middleware modules for the Delong gateway
//!
//! This module provides middleware for gateway functionality:
//! - Authentication: API key validation and user authentication (gateway-specific)
//! - Logging: Request/response logging with structured data (from common)
//! - Request ID: Unique request identifier for tracing (from common)

// Gateway-specific middleware
pub mod auth;
