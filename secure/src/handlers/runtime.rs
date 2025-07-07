use axum::{
    extract::{Path, Query, State},
    response::Json,
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, error};

use common::ApiResponse;
use crate::runtime::{
    ExecutionScheduler, 
    execution_queue::{ExecutionRequest, ExecutionStatus},
    scheduler::ExecutionStats,
};

/// Request to submit an algorithm execution
#[derive(Debug, Deserialize)]
pub struct SubmitExecutionRequest {
    pub algorithm_cid: String,
    pub dataset_id: String,
    pub scientist_wallet: String,
    pub priority: Option<u32>,
    pub parameters: Option<serde_json::Value>,
}

/// Query parameters for listing executions
#[derive(Debug, Deserialize)]
pub struct ListExecutionsQuery {
    pub status: Option<ExecutionStatus>,
    pub scientist_wallet: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Application state for runtime handlers
#[derive(Clone)]
pub struct RuntimeState {
    pub scheduler: Arc<ExecutionScheduler>,
}

/// Submit a new algorithm execution request
pub async fn submit_execution(
    State(state): State<RuntimeState>,
    Json(request): Json<SubmitExecutionRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    info!(
        algorithm_cid = %request.algorithm_cid,
        dataset_id = %request.dataset_id,
        scientist_wallet = %request.scientist_wallet,
        "Submitting algorithm execution request"
    );

    // Generate unique execution ID
    let execution_id = chrono::Utc::now().timestamp_millis() as u64;

    // Create execution request
    let execution_request = ExecutionRequest {
        execution_id,
        algorithm_cid: request.algorithm_cid,
        dataset_id: request.dataset_id,
        scientist_wallet: request.scientist_wallet,
        priority: request.priority.unwrap_or(0),
        created_at: chrono::Utc::now(),
        status: ExecutionStatus::Queued,
        parameters: request.parameters.unwrap_or(serde_json::Value::Null),
    };

    // Submit to scheduler
    match state.scheduler.submit_execution(execution_request).await {
        Ok(()) => {
            info!(execution_id = %execution_id, "Algorithm execution submitted successfully");
            Ok(Json(ApiResponse::success(serde_json::json!({
                "execution_id": execution_id,
                "status": "queued",
                "message": "Algorithm execution request submitted successfully"
            }))))
        }
        Err(e) => {
            error!(
                execution_id = %execution_id,
                error = %e,
                "Failed to submit algorithm execution"
            );
            Ok(Json(ApiResponse::success(serde_json::json!({
                "error": true,
                "message": format!("Failed to submit execution: {}", e)
            }))))
        }
    }
}

/// Cancel an algorithm execution
pub async fn cancel_execution(
    State(state): State<RuntimeState>,
    Path(execution_id): Path<u64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    info!(execution_id = %execution_id, "Cancelling algorithm execution");

    match state.scheduler.cancel_execution(execution_id).await {
        Ok(()) => {
            info!(execution_id = %execution_id, "Algorithm execution cancelled successfully");
            Ok(Json(ApiResponse::success(serde_json::json!({
                "execution_id": execution_id,
                "status": "cancelled",
                "message": "Algorithm execution cancelled successfully"
            }))))
        }
        Err(e) => {
            error!(
                execution_id = %execution_id,
                error = %e,
                "Failed to cancel algorithm execution"
            );
            Ok(Json(ApiResponse::success(serde_json::json!({
                "error": true,
                "message": format!("Failed to cancel execution: {}", e)
            }))))
        }
    }
}

/// Get execution queue statistics
pub async fn get_queue_stats(
    State(state): State<RuntimeState>,
) -> Result<Json<ApiResponse<ExecutionStats>>, StatusCode> {
    info!("Getting execution queue statistics");

    let stats = state.scheduler.get_execution_stats().await;
    Ok(Json(ApiResponse::success(stats)))
}

/// List queued executions
pub async fn list_queued_executions(
    State(state): State<RuntimeState>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<Json<ApiResponse<Vec<ExecutionRequest>>>, StatusCode> {
    info!("Listing queued executions");

    let mut executions = state.scheduler.get_queued_requests().await;

    // Apply filters
    if let Some(scientist_wallet) = query.scientist_wallet {
        executions.retain(|req| req.scientist_wallet == scientist_wallet);
    }

    // Apply pagination
    let offset = query.offset.unwrap_or(0) as usize;
    let limit = query.limit.unwrap_or(100) as usize;
    
    let total_count = executions.len();
    let paginated_executions = executions
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();

    info!(
        total_count = %total_count,
        returned_count = %paginated_executions.len(),
        "Retrieved queued executions"
    );

    Ok(Json(ApiResponse::success(paginated_executions)))
}

/// List running executions
pub async fn list_running_executions(
    State(state): State<RuntimeState>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<Json<ApiResponse<Vec<ExecutionRequest>>>, StatusCode> {
    info!("Listing running executions");

    let mut executions = state.scheduler.get_running_requests().await;

    // Apply filters
    if let Some(scientist_wallet) = query.scientist_wallet {
        executions.retain(|req| req.scientist_wallet == scientist_wallet);
    }

    // Apply pagination
    let offset = query.offset.unwrap_or(0) as usize;
    let limit = query.limit.unwrap_or(100) as usize;
    
    let total_count = executions.len();
    let paginated_executions = executions
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();

    info!(
        total_count = %total_count,
        returned_count = %paginated_executions.len(),
        "Retrieved running executions"
    );

    Ok(Json(ApiResponse::success(paginated_executions)))
}

/// Get execution status by ID
pub async fn get_execution_status(
    State(state): State<RuntimeState>,
    Path(execution_id): Path<u64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    info!(execution_id = %execution_id, "Getting execution status");

    // Check in queued executions
    let queued_executions = state.scheduler.get_queued_requests().await;
    if let Some(request) = queued_executions.iter().find(|req| req.execution_id == execution_id) {
        return Ok(Json(ApiResponse::success(serde_json::json!({
            "execution_id": execution_id,
            "status": request.status,
            "created_at": request.created_at,
            "algorithm_cid": request.algorithm_cid,
            "dataset_id": request.dataset_id,
            "scientist_wallet": request.scientist_wallet,
            "priority": request.priority,
            "parameters": request.parameters
        }))));
    }

    // Check in running executions
    let running_executions = state.scheduler.get_running_requests().await;
    if let Some(request) = running_executions.iter().find(|req| req.execution_id == execution_id) {
        return Ok(Json(ApiResponse::success(serde_json::json!({
            "execution_id": execution_id,
            "status": request.status,
            "created_at": request.created_at,
            "algorithm_cid": request.algorithm_cid,
            "dataset_id": request.dataset_id,
            "scientist_wallet": request.scientist_wallet,
            "priority": request.priority,
            "parameters": request.parameters
        }))));
    }

    // TODO: Check in database for completed executions

    Ok(Json(ApiResponse::success(serde_json::json!({
        "execution_id": execution_id,
        "status": "not_found",
        "message": format!("Execution {} not found", execution_id)
    }))))
}

/// Emergency shutdown - cancel all executions
pub async fn emergency_shutdown(
    State(state): State<RuntimeState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    info!("Emergency shutdown requested");

    match state.scheduler.emergency_shutdown().await {
        Ok(()) => {
            info!("Emergency shutdown completed successfully");
            Ok(Json(ApiResponse::success(serde_json::json!({
                "message": "Emergency shutdown completed successfully",
                "timestamp": chrono::Utc::now()
            }))))
        }
        Err(e) => {
            error!(error = %e, "Failed to perform emergency shutdown");
            Ok(Json(ApiResponse::success(serde_json::json!({
                "error": true,
                "message": format!("Failed to perform emergency shutdown: {}", e)
            }))))
        }
    }
} 