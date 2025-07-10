use crate::models::ContractMeta;
use crate::AppState;
use axum::extract::State;
use axum::response::Json;
use std::sync::Arc;
use tracing::{error, info, instrument};
use common::ApiResponse;

/// List all contract metadata
#[instrument(skip(state))]
pub async fn list_contracts(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ContractMeta>>> {
    let contracts = ContractMeta::get_all(&state.db)
        .await
        .map_err(|e| {
            error!("Failed to list contracts: {}", e);
            ApiError::InternalServerError
        })?;
    Ok(Json(contracts))
} 