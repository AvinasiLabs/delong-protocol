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
    flexible_auth_middleware,
};

// Export cookie authentication middleware
pub use auth::{
    admin_cookie_auth_middleware, clear_auth_cookies, cookie_auth_middleware,
    optional_cookie_auth_middleware, set_auth_cookies,
};

// Export rate limiting middleware
pub use rate_limit::{RateLimitConfig, api_key_rate_limit, rate_limit_middleware};

// Export request ID middleware
pub use request_id::{ClientIp, REQUEST_ID_HEADER, RequestId, request_id_middleware};

// Export response transformer
pub use response_transformer::response_transformer;
