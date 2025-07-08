//! Services module
//!
//! This module contains all business logic services and external service integrations.
//! Services are responsible for implementing core business functionality, integrating
//! with external APIs, and providing reusable components across the application.

pub mod ai_audit;
pub mod email;
pub mod verification;

// Re-export commonly used service types and structs
pub use crate::utils::jwt::Claims;
pub use ai_audit::{AiAuditService, AuditFileResult, AuditSummary, SecurityBreach};
pub use email::{EmailConfig, EmailSendResult, EmailService, EmailTemplate};
pub use verification::{VerificationConfig, VerificationStats, VerificationStore};
