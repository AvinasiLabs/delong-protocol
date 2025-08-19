//! Middleware module for Secure service
//!
//! This module contains middleware components for request processing,
//! authentication, and security in the TEE environment.

pub mod internal_jwt;

// Re-export commonly used middleware components
pub use internal_jwt::{internal_jwt_middleware, AuthContext, AuthExt, AuthenticatedUser};
