//! Proxy infrastructure for forwarding requests to Secure service
//!
//! This module provides the proxy functionality that allows the Core service
//! to act as an authentication gateway, forwarding authenticated requests
//! to the Secure service running in the TEE environment.
//!
//! ## Architecture
//!
//! ```text
//! Client (JWT/API Key) -> Core (Auth Gateway) -> Secure (TEE)
//!                            |
//!                            +-> Generate Internal JWT
//!                            +-> Add Request Signature
//!                            +-> Forward with Auth Context
//! ```
//!
//! ## Authentication Flow
//!
//! 1. **Client Authentication**: Clients authenticate with Core using either:
//!    - JWT tokens (for web users)
//!    - API keys (for developers/programmatic access)
//!
//! 2. **Request Validation**: Core validates the authentication credentials
//!
//! 3. **Internal JWT Generation**: Core generates a short-lived internal JWT containing:
//!    - User identity and context
//!    - Request signature (method, path, body digest)
//!    - Authentication method used
//!    - Permission scopes
//!
//! 4. **Request Forwarding**: Core forwards the request to Secure with the internal JWT
//!
//! 5. **Response Handling**: Core receives and forwards the response back to the client
//!
//! ## Security Features
//!
//! - **Short-lived tokens**: Internal JWTs expire in 60 seconds
//! - **Request integrity**: SHA256 digest of request body included in JWT
//! - **Authentication context**: Preserves original auth method and user context
//! - **Retry logic**: Automatic retries with exponential backoff
//! - **Request tracing**: Request IDs for distributed tracing

pub mod client;

pub use client::{AuthContext, ProxyClient, create_proxy_client};
