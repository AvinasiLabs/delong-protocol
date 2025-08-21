//! Authentication-related models for the DeLong Protocol
//!
//! This module contains database models and operations for authentication functionality.

use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Verification type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum VerificationType {
    Email,
    Phone,
    PasswordReset,
    AccountActivation,
}

impl fmt::Display for VerificationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationType::Email => write!(f, "email"),
            VerificationType::Phone => write!(f, "phone"),
            VerificationType::PasswordReset => write!(f, "password_reset"),
            VerificationType::AccountActivation => write!(f, "account_activation"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_type_display() {
        assert_eq!(VerificationType::Email.to_string(), "email");
        assert_eq!(VerificationType::Phone.to_string(), "phone");
        assert_eq!(
            VerificationType::PasswordReset.to_string(),
            "password_reset"
        );
        assert_eq!(
            VerificationType::AccountActivation.to_string(),
            "account_activation"
        );
    }
}
