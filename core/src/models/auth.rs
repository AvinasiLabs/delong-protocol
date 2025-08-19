//! Authentication-related models for the DeLong Protocol
//!
//! This module contains database models and operations for authentication functionality.

use crate::AppError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Verification type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum VerificationType {
    Email,
    Phone,
    PasswordReset,
    AccountActivation,
}

impl std::fmt::Display for VerificationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationType::Email => write!(f, "email"),
            VerificationType::Phone => write!(f, "phone"),
            VerificationType::PasswordReset => write!(f, "password_reset"),
            VerificationType::AccountActivation => write!(f, "account_activation"),
        }
    }
}

/// Verification code model matching PostgreSQL schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCode {
    pub id: i32,
    pub user_email: String,
    pub code: String,
    pub verification_type: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub is_used: bool,
    pub attempts: i32,
}

impl VerificationCode {
    /// Create a new verification code
    pub async fn create(
        pool: &PgPool,
        email: &str,
        code: &str,
        verification_type: &str,
        expires_minutes: i64,
    ) -> Result<Self, AppError> {
        let expires_at = Utc::now() + Duration::minutes(expires_minutes);

        let record = sqlx::query!(
            r#"
            INSERT INTO verification_codes (user_email, code, verification_type, expires_at, created_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP)
            RETURNING id, user_email, code, verification_type, expires_at, created_at, used_at, is_used, attempts
            "#,
            email,
            code,
            verification_type,
            expires_at
        )
        .fetch_one(pool)
        .await?;

        Ok(VerificationCode {
            id: record.id,
            user_email: record.user_email,
            code: record.code,
            verification_type: record.verification_type,
            expires_at: record.expires_at,
            created_at: record.created_at.unwrap_or_else(|| Utc::now()),
            used_at: record.used_at,
            is_used: record.is_used.unwrap_or(false),
            attempts: record.attempts.unwrap_or(0),
        })
    }

    /// Find a verification code by email and type
    pub async fn find_by_email_and_type(
        pool: &PgPool,
        email: &str,
        verification_type: &str,
    ) -> Result<Option<Self>, AppError> {
        let record = sqlx::query!(
            r#"
            SELECT id, user_email, code, verification_type, expires_at, created_at, used_at, is_used, attempts
            FROM verification_codes
            WHERE user_email = $1 AND verification_type = $2 AND is_used = false AND expires_at > CURRENT_TIMESTAMP
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            email,
            verification_type
        )
        .fetch_optional(pool)
        .await?;

        Ok(record.map(|r| VerificationCode {
            id: r.id,
            user_email: r.user_email,
            code: r.code,
            verification_type: r.verification_type,
            expires_at: r.expires_at,
            created_at: r.created_at.unwrap_or_else(|| Utc::now()),
            used_at: r.used_at,
            is_used: r.is_used.unwrap_or(false),
            attempts: r.attempts.unwrap_or(0),
        }))
    }

    /// Verify a code
    pub async fn verify(
        pool: &PgPool,
        email: &str,
        code: &str,
        verification_type: &str,
    ) -> Result<bool, AppError> {
        // First, find the verification code
        let verification = Self::find_by_email_and_type(pool, email, verification_type).await?;

        match verification {
            Some(mut v) => {
                // Increment attempts
                v.attempts += 1;
                sqlx::query!(
                    "UPDATE verification_codes SET attempts = $1 WHERE id = $2",
                    v.attempts,
                    v.id
                )
                .execute(pool)
                .await?;

                // Check if code matches
                if v.code == code {
                    // Mark as used
                    sqlx::query!(
                        "UPDATE verification_codes SET is_used = true, used_at = CURRENT_TIMESTAMP WHERE id = $1",
                        v.id
                    )
                    .execute(pool)
                    .await?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false),
        }
    }

    /// Mark a verification code as used
    pub async fn mark_as_used(pool: &PgPool, id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE verification_codes SET is_used = true, used_at = CURRENT_TIMESTAMP WHERE id = $1",
            id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete expired codes
    pub async fn delete_expired(pool: &PgPool) -> Result<u64, AppError> {
        let result = sqlx::query!(
            "DELETE FROM verification_codes WHERE expires_at < CURRENT_TIMESTAMP OR is_used = true"
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Check if a code has expired
    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }

    /// Check if a code has exceeded max attempts
    pub fn has_exceeded_attempts(&self, max_attempts: i32) -> bool {
        self.attempts >= max_attempts
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

    #[test]
    fn test_verification_code_expiration() {
        let code = VerificationCode {
            id: 1,
            user_email: "test@example.com".to_string(),
            code: "123456".to_string(),
            verification_type: "email".to_string(),
            expires_at: Utc::now() - Duration::minutes(1),
            created_at: Utc::now() - Duration::minutes(6),
            used_at: None,
            is_used: false,
            attempts: 0,
        };

        assert!(code.is_expired());
    }

    #[test]
    fn test_verification_code_max_attempts() {
        let code = VerificationCode {
            id: 1,
            user_email: "test@example.com".to_string(),
            code: "123456".to_string(),
            verification_type: "email".to_string(),
            expires_at: Utc::now() + Duration::minutes(10),
            created_at: Utc::now(),
            used_at: None,
            is_used: false,
            attempts: 3,
        };

        assert!(code.has_exceeded_attempts(3));
        assert!(!code.has_exceeded_attempts(5));
    }
}
