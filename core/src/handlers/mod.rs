//! Core service handlers module
//!
//! This module contains all HTTP request handlers organized by business domain.
//! Each handler module contains functions that process specific API endpoints
//! and return appropriate HTTP responses.

// Handler modules organized by business domain
pub mod admin;
pub mod ai_audit;
pub mod api_key;
pub mod auth;
pub mod health;
pub mod proxy;
pub mod siwe;

// Re-export commonly used handlers for easier access
pub use admin::{
    check_permission, create_user_admin, get_permissions, get_roles, get_user_by_id, get_users,
    update_user,
};
pub use ai_audit::create_ai_audit;
pub use api_key::{
    create_api_key, get_api_key, get_user_api_key_stats, list_user_api_keys, revoke_api_key,
    update_api_key,
};
pub use auth::{
    get_current_user, google_auth_callback, google_auth_url, login_user, register_user,
    send_verification_code,
};
pub use health::health;
pub use proxy::{extract_auth_method, proxy_handler, proxy_routes, secure_health_check};
pub use siwe::{get_siwe_nonce, get_wallet_status, link_wallet, unlink_wallet, verify_signature};

// Import avinapi prelude for response macros and types
pub use avinapi::prelude::*;

/// Root handler for API base path
pub async fn root_handler() -> JsonResult<serde_json::Value> {
    data!(serde_json::json!({
        "service": "DeLong Protocol Core Service",
        "version": crate::VERSION,
        "status": "running"
    }))
}
