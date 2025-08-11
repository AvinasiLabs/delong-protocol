//! Error handling for the secure service
//!
//! This module uses AppError from avinapi for consistent error handling across services.

use avinapi::prelude::{AppError, AppResult};
use chrono::NaiveDateTime;

// Re-export common error types for convenience
pub use avinapi::prelude::{AppError as SecureError, AppResult as SecureResult};

// Re-export for module usage
pub type Result<T> = AppResult<T>;

/// Extension trait for timestamp conversion
pub trait TimestampExt {
    /// Convert to NaiveDateTime, returning error if invalid
    fn to_naive_datetime(&self) -> AppResult<NaiveDateTime>;
}

impl TimestampExt for i64 {
    fn to_naive_datetime(&self) -> AppResult<NaiveDateTime> {
        chrono::DateTime::from_timestamp(*self, 0)
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| AppError::Internal(format!("Invalid timestamp: {}", self)))
    }
}

impl TimestampExt for alloy::primitives::U256 {
    fn to_naive_datetime(&self) -> AppResult<NaiveDateTime> {
        let timestamp = self.to::<i64>();
        timestamp.to_naive_datetime()
    }
}
