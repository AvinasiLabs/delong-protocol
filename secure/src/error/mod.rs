//! Error handling module that uses avinapi for consistent error types.

pub use avinapi::prelude::AppResult as Result;
pub use avinapi::prelude::{AppError, AppResult};

/// Extension trait for converting database errors with context
pub trait DbErrorExt<T> {
    /// Convert to AppError with not found handling
    fn not_found_msg(self, entity: &str) -> Result<T>;

    /// Convert to AppError with conflict handling for unique constraint violations
    fn conflict_msg(self, msg: &str) -> Result<T>;
}

impl<T> DbErrorExt<T> for std::result::Result<T, sqlx::Error> {
    fn not_found_msg(self, entity: &str) -> Result<T> {
        self.map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound(format!("{} not found", entity)),
            _ => AppError::Database(e),
        })
    }

    fn conflict_msg(self, msg: &str) -> Result<T> {
        self.map_err(|e| match &e {
            sqlx::Error::Database(db_err) => {
                // MySQL error code 1062 is for duplicate entry
                if db_err.code().map(|c| c == "1062").unwrap_or(false) {
                    AppError::Conflict(msg.to_string())
                } else {
                    AppError::Database(e)
                }
            }
            _ => AppError::Database(e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_constructors() {
        let error = AppError::Validation("test message".to_string());
        assert_eq!(format!("{}", error), "Validation error: test message");

        let error = AppError::NotFound("resource not found".to_string());
        assert_eq!(
            format!("{}", error),
            "Resource not found: resource not found"
        );

        let error = AppError::Internal("internal error".to_string());
        assert_eq!(
            format!("{}", error),
            "Internal server error: internal error"
        );
    }

    #[test]
    fn test_project_specific_errors() {
        let error = AppError::Internal("ipfs upload failed".to_string());
        assert_eq!(
            format!("{}", error),
            "Internal server error: ipfs upload failed"
        );

        let error = AppError::Internal("blockchain transaction failed".to_string());
        assert_eq!(
            format!("{}", error),
            "Internal server error: blockchain transaction failed"
        );

        let error = AppError::Internal("tee enclave error".to_string());
        assert_eq!(
            format!("{}", error),
            "Internal server error: tee enclave error"
        );

        let error = AppError::Config("missing config".to_string());
        assert_eq!(format!("{}", error), "Configuration error: missing config");
    }
}
