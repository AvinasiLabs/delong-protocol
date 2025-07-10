//! Runtime execution data models
//!
//! This module contains shared data structures for TEE runtime execution management.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request to submit an algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitExecutionRequest {
    pub algorithm_cid: String,
    pub dataset_id: String,
    pub scientist_wallet: String,
    pub priority: Option<u32>,
    pub parameters: Option<serde_json::Value>,
}

/// Response for submitting an execution
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitExecutionResponse {
    pub execution_id: u64,
    pub status: String,
}

/// Query parameters for listing executions
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListExecutionsQuery {
    pub status: Option<String>,
    pub scientist_wallet: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Represents an algorithm execution in an API response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionResponse {
    pub execution_id: u64,
    pub algorithm_cid: String,
    pub dataset_id: String,
    pub scientist_wallet: String,
    pub priority: u32,
    pub created_at: String,
    pub status: String,
    pub parameters: serde_json::Value,
}

/// Statistics about the execution queue
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionStats {
    pub queued_count: usize,
    pub running_count: usize,
    pub max_concurrent: usize,
    pub available_slots: usize,
} 