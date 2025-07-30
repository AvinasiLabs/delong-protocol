//! Database connection and pool management utilities
//!
//! This module provides centralized database connection management for the secure service,
//! including connection pool initialization, health checks, and migration support.

use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tracing::{error, info, instrument};

use crate::config::DatabaseConfig;

/// Database connection manager
/// Database connection pool wrapper
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool
    #[instrument(skip(config))]
    pub async fn new(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        info!("Initializing database connection pool");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout))
            .idle_timeout(Duration::from_secs(config.idle_timeout))
            .test_before_acquire(true)
            .connect(&config.url)
            .await
            .map_err(|e| {
                error!("Failed to create database pool: {}", e);
                DatabaseError::ConnectionFailed(e.to_string())
            })?;

        info!(
            "Database pool created with {} max connections",
            config.max_connections
        );

        Ok(Self { pool })
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Check database connectivity
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), DatabaseError> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database health check failed: {}", e);
                DatabaseError::HealthCheckFailed(e.to_string())
            })?;

        Ok(())
    }

    /// Run database migrations
    #[instrument(skip(self))]
    pub async fn run_migrations(&self) -> Result<(), DatabaseError> {
        info!("Running database migrations");

        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to run migrations: {}", e);
                DatabaseError::MigrationFailed(e.to_string())
            })?;

        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> DatabaseStats {
        let pool = &self.pool;
        DatabaseStats {
            size: pool.size(),
            idle: pool.num_idle(),
            max_connections: pool.options().get_max_connections(),
            min_connections: pool.options().get_min_connections(),
        }
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub size: u32,
    pub idle: usize,
    pub max_connections: u32,
    pub min_connections: u32,
}

/// Database errors
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Failed to connect to database: {0}")]
    ConnectionFailed(String),

    #[error("Database health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Migration failed: {0}")]
    MigrationFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
}

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::PoolTimedOut => {
                DatabaseError::ConnectionFailed("Connection pool timed out".to_string())
            }
            sqlx::Error::PoolClosed => {
                DatabaseError::ConnectionFailed("Connection pool is closed".to_string())
            }
            _ => DatabaseError::QueryFailed(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_error_conversion() {
        let sqlx_error = sqlx::Error::PoolClosed;
        let db_error: DatabaseError = sqlx_error.into();
        match db_error {
            DatabaseError::ConnectionFailed(msg) => {
                assert_eq!(msg, "Connection pool is closed");
            }
            _ => panic!("Expected ConnectionFailed error"),
        }
    }

    #[test]
    fn test_database_stats() {
        // This test doesn't require actual database connection
        let stats = DatabaseStats {
            size: 10,
            idle: 5,
            max_connections: 20,
            min_connections: 5,
        };

        assert_eq!(stats.size, 10);
        assert_eq!(stats.idle, 5);
        assert_eq!(stats.max_connections, 20);
        assert_eq!(stats.min_connections, 5);
    }
}
