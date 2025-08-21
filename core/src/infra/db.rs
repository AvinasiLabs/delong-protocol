//! Database connection and pool management for Core service
//!
//! This module provides centralized database connection management,
//! including connection pool initialization, health checks, and migration support.

use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tracing::{error, info, instrument};

use crate::AppError;
use crate::config::DatabaseConfig;

/// Database connection pool wrapper
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool
    #[instrument(skip(config))]
    pub async fn new(config: &DatabaseConfig) -> Result<Self, AppError> {
        info!("Initializing database connection pool");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .test_before_acquire(true)
            .connect(&config.url)
            .await
            .map_err(|e| {
                error!("Failed to create database pool: {}", e);
                AppError::Config(format!("Failed to connect to database: {}", e))
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

    /// Create a Database instance from an existing pool
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check database connectivity
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database health check failed: {}", e);
                AppError::Database(e)
            })?;

        Ok(())
    }

    /// Run database migrations
    #[instrument(skip(self))]
    pub async fn run_migrations(&self) -> Result<(), AppError> {
        info!("Running database migrations");

        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to run migrations: {}", e);
                AppError::Database(e.into())
            })?;

        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> DatabaseStats {
        DatabaseStats {
            size: self.pool.size(),
            idle: self.pool.num_idle(),
            max_connections: self.pool.options().get_max_connections(),
        }
    }
}

impl AsRef<PgPool> for Database {
    fn as_ref(&self) -> &PgPool {
        &self.pool
    }
}

impl From<Database> for PgPool {
    fn from(db: Database) -> Self {
        db.pool
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    /// Current number of connections in the pool
    pub size: u32,
    /// Number of idle connections
    pub idle: usize,
    /// Maximum number of connections allowed
    pub max_connections: u32,
}
