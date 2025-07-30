//! Error handling module that uses avinapi for consistent error types.

pub use avinapi::error::AppResult as Result;
pub use avinapi::error::{AppError, AppResult};

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
            sqlx::Error::RowNotFound => AppError::not_found(format!("{} not found", entity)),
            _ => AppError::database(e.to_string()),
        })
    }

    fn conflict_msg(self, msg: &str) -> Result<T> {
        self.map_err(|e| match &e {
            sqlx::Error::Database(db_err) => {
                // MySQL error code 1062 is for duplicate entry
                if db_err.code().map(|c| c == "1062").unwrap_or(false) {
                    AppError::conflict(msg)
                } else {
                    AppError::database(e.to_string())
                }
            }
            _ => AppError::database(e.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_constructors() {
        let error = AppError::validation("test message");
        assert_eq!(format!("{}", error), "Validation error: test message");

        let error = AppError::not_found("resource not found");
        assert_eq!(format!("{}", error), "Not found: resource not found");

        let error = AppError::internal("internal error");
        assert_eq!(format!("{}", error), "Internal error: internal error");
    }

    #[test]
    fn test_project_specific_errors() {
        let error = AppError::ipfs("upload failed");
        assert_eq!(format!("{}", error), "IPFS error: upload failed");

        let error = AppError::blockchain("transaction failed");
        assert_eq!(format!("{}", error), "Blockchain error: transaction failed");

        let error = AppError::tee("enclave error");
        assert_eq!(format!("{}", error), "TEE error: enclave error");

        let error = AppError::configuration("missing config");
        assert_eq!(format!("{}", error), "Configuration error: missing config");
    }
}
