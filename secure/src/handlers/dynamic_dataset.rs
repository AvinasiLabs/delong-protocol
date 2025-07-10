use crate::models::{
    CreateDynamicDatasetRequest, DynamicDataset, UpdateDynamicDatasetRequest,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use common::{ApiResponse, PaginatedResponse, PaginationParams};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[instrument(skip_all)]
pub async fn create_dynamic_dataset(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateDynamicDatasetRequest>,
) -> Result<Json<ApiResponse<DynamicDataset>>, Json<ApiResponse<()>>> {
    info!(name = %payload.name, "Creating new dynamic dataset");
    
    // In the Go project, ui_name is passed but not used, and file_path is constructed.
    let file_path = format!("/app/data/datasets/dynamic/{}", payload.name);
    
    match DynamicDataset::create(&state.db, &payload.name, &payload.ui_name, &payload.description, &file_path).await {
        Ok(dataset) => Ok(Json(ApiResponse::success(dataset))),
        Err(e) => {
            error!("Failed to create dynamic dataset: {}", e);
            Err(Json(ApiResponse::internal_error()))
        }
    }
}

#[instrument(skip(state))]
pub async fn list_dynamic_datasets(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ApiResponse<PaginatedResponse<DynamicDataset>>>, Json<ApiResponse<()>>> {
    let page = params.page;
    let limit = params.limit;
    info!(page, limit, "Listing dynamic datasets");

    match DynamicDataset::get_paginated(&state.db, page as i32, limit as i32).await {
        Ok((datasets, total)) => {
            let paginated_response = PaginatedResponse::new(datasets, page, limit, total as u64);
            Ok(Json(ApiResponse::success(paginated_response)))
        }
        Err(e) => {
            error!("Failed to list dynamic datasets: {}", e);
            Err(Json(ApiResponse::internal_error()))
        }
    }
}

#[instrument(skip(state), fields(id = %id))]
pub async fn get_dynamic_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<DynamicDataset>>, Json<ApiResponse<()>>> {
    info!("Getting dynamic dataset by ID");
    match DynamicDataset::get_by_id(&state.db, id as i32).await {
        Ok(dataset) => Ok(Json(ApiResponse::success(dataset))),
        Err(sqlx::Error::RowNotFound) => {
            Err(Json(ApiResponse::not_found()))
        }
        Err(e) => {
            error!("Failed to get dynamic dataset: {}", e);
            Err(Json(ApiResponse::internal_error()))
        }
    }
}

#[instrument(skip_all)]
pub async fn update_dynamic_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateDynamicDatasetRequest>,
) -> Result<Json<ApiResponse<DynamicDataset>>, Json<ApiResponse<()>>> {
    info!(id, "Updating dynamic dataset");
    match DynamicDataset::update(&state.db, id as i32, &req.description).await {
        Ok(dataset) => Ok(Json(ApiResponse::success(dataset))),
        Err(sqlx::Error::RowNotFound) => {
            Err(Json(ApiResponse::not_found()))
        }
        Err(e) => {
            error!("Failed to update dynamic dataset: {}", e);
            Err(Json(ApiResponse::internal_error()))
        }
    }
}

#[instrument(skip(state), fields(id = %id))]
pub async fn delete_dynamic_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<()>>, Json<ApiResponse<()>>> {
    info!("Deleting dynamic dataset by ID");
    match DynamicDataset::delete(&state.db, id as i32).await {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(sqlx::Error::RowNotFound) => {
            Err(Json(ApiResponse::not_found()))
        }
        Err(e) => {
            error!("Failed to delete dynamic dataset: {}", e);
            Err(Json(ApiResponse::internal_error()))
        }
    }
} 