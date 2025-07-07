use axum::extract::{Path, Query, State};
use axum::response::Json;
use common::{ApiResponse, PaginationParams};
use std::sync::Arc;

use crate::AppState;
// use crate::services::{DatasetService, dataset::StaticDataset};

// Temporary placeholder for StaticDataset
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StaticDataset {
    pub id: i64,
    pub name: String,
}

/// List static datasets with pagination
pub async fn list_datasets(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<common::PaginatedResponse<StaticDataset>>> {
    tracing::info!(page = %params.page, limit = %params.limit, "Listing static datasets");
    
    // TODO: Implement actual database query
    let result = common::PaginatedResponse::new(
        vec![],
        params.page,
        params.limit,
        0,
    );
    
    Json(ApiResponse::success_with_message(
        result,
        "Static datasets retrieved successfully",
    ))
}

/// Get a specific dataset by ID
pub async fn get_dataset(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<Option<StaticDataset>>> {
    tracing::info!(dataset_id = %id, "Getting dataset by ID");
    
    // TODO: Implement actual database query
    
    Json(ApiResponse::success_with_message(
        None,
        "Dataset not found",
    ))
} 