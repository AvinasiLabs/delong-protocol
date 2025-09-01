//! Services module
//!
//! This module contains all business logic services and external service integrations.
//! Services are responsible for implementing core business functionality, integrating
//! with external APIs, and providing reusable components across the application.

pub mod ai_audit;
pub mod db;
pub mod email;
pub mod google_oauth;
pub mod proxy;
pub mod verification;

// Re-export commonly used service types and structs
// Claims is now exported from utils::jwt module
pub use ai_audit::{AiAuditService, AuditFileResult, AuditSummary, SecurityBreach};
pub use db::{Database, DatabaseStats};
pub use email::{EmailService, EmailTemplate};
pub use google_oauth::{GoogleOAuthService, GoogleUserInfo, OAuthState, OAuthStateStore};
pub use proxy::{AuthContext, ProxyClient, create_proxy_client};
pub use verification::VerificationStore;
