use axum::extract::{Path, Query, State};
use axum::response::Json;
use common::{ApiResponse, PaginationParams};
use std::sync::Arc;
use common::models::dataset::StaticDatasetInfo;

use crate::AppState;

/// List static datasets with pagination
pub async fn list_datasets(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<common::PaginatedResponse<StaticDatasetInfo>>> {
    tracing::info!(page = %params.page, limit = %params.limit, "Listing static datasets");
    
    // TODO: Implement actual database query
    let result = common::PaginatedResponse::new(
        vec![],
        params.page,
        params.limit,
        0,
    );
    
    Json(ApiResponse::success(result))
}

/// Get a specific dataset by ID
pub async fn get_dataset(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<Option<StaticDatasetInfo>>> {
    tracing::info!(dataset_id = %id, "Getting dataset by ID");
    
    // TODO: Implement actual database query
    
    Json(ApiResponse::success(None))
} 