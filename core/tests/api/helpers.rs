
use std::sync::Arc;
use axum::Router;
use sqlx::PgPool;

use crate::common::create_test_router;
use delong_core::{create_app, AppState};

/// Cleans the database by dropping and recreating the public schema.
pub async fn cleanup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // This is a destructive operation, only for test environments.
    // It's often better to use transactions and roll them back,
    // but for simplicity, we'll drop the schema.
    sqlx::query("DROP SCHEMA public CASCADE").execute(pool).await?;
    sqlx::query("CREATE SCHEMA public").execute(pool).await?;
    Ok(())
}

/// Sets up the environment for a new test, creating a default AppState.
pub async fn setup_test_environment() -> (Router, Arc<AppState>) {
    let (router, state) = create_test_router().await;

    // Clean the database to ensure a fresh start.
    cleanup_database(&state.db).await.unwrap();
    
    // Now, run migrations on the clean database.
    sqlx::migrate!("./migrations")
        .run(&state.db)
        .await
        .expect("Failed to run database migrations on clean DB");

    (router, state)
} 