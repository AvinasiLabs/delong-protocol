use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers::runtime::RuntimeState;

/// Create runtime routes
pub fn create_runtime_routes() -> Router<RuntimeState> {
    Router::new()
        // Submit new execution
        .route("/submit", post(crate::handlers::runtime::submit_execution))
        
        // Cancel execution
        .route("/cancel/:execution_id", post(crate::handlers::runtime::cancel_execution))
        
        // Get queue statistics
        .route("/stats", get(crate::handlers::runtime::get_queue_stats))
        
        // List queued executions
        .route("/queued", get(crate::handlers::runtime::list_queued_executions))
        
        // List running executions
        .route("/running", get(crate::handlers::runtime::list_running_executions))
        
        // Get execution status
        .route("/status/:execution_id", get(crate::handlers::runtime::get_execution_status))
        
        // Emergency shutdown
        .route("/emergency-shutdown", post(crate::handlers::runtime::emergency_shutdown))
} 