pub mod auth;
pub mod rate_limit;
pub mod request_id;
pub mod response_transformer;

// Export authentication middleware and types
pub use auth::{
    // Extractors
    AdminUser,
    AuthMethod,
    // Core types
    AuthUser,
    admin_only_middleware,
    api_key_only_middleware,
    flexible_auth_middleware,
    // Middleware functions
    jwt_only_middleware,
};

// Export rate limiting middleware
pub use rate_limit::{RateLimitConfig, api_key_rate_limit, rate_limit_middleware};

// Export request ID middleware
pub use request_id::{ClientIp, REQUEST_ID_HEADER, RequestId, request_id_middleware};

// Export response transformer
pub use response_transformer::response_transformer;
