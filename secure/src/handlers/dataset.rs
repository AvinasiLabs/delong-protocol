use crate::{
    models::{
        BlockchainTransaction, CreateStcDatasetReq, StaticDataset, UpdateStaticDatasetRequest,
        ENTITY_TYPE_STATIC_DATASET,
    },
    services::key_ctx::{KeyContext, KeyKind},
    AppState,
};
use axum::{
    extract::{Multipart, Path, Query, State},
    response::Json,
};
use common::{ApiError, ApiResult, ApiResponse, PaginatedResponse, PaginationParams};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::SeekFrom;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tracing::{error, info, instrument};
use uuid::Uuid;

/// List static datasets with pagination
#[instrument(skip(state), fields(page = %params.page, limit = %params.limit))]
pub async fn list_datasets(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> ApiResult<Json<ApiResponse<PaginatedResponse<StaticDataset>>>> {
    info!("Listing static datasets");

    let (datasets, total) =
        StaticDataset::get_paginated(&state.db, params.page as i64, params.limit as i64).await?;
    let response = PaginatedResponse::new(datasets, params.page, params.limit, total as u64);
    Ok(Json(ApiResponse::success(response)))
}

/// Get a specific dataset by ID
#[instrument(skip(state), fields(id = %id))]
pub async fn get_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ApiResponse<StaticDataset>>> {
    info!("Getting dataset by ID");

    let dataset = StaticDataset::get_by_id(&state.db, id).await?;
    Ok(Json(ApiResponse::success(dataset)))
}

/// Create a new static dataset by uploading a file
#[instrument(skip_all)]
pub async fn create_dataset(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> ApiResult<Json<ApiResponse<StaticDataset>>> {
    info!("Starting static dataset creation process");

    // 1. Setup temporary file storage
    let temp_file_name = format!("/tmp/{}", Uuid::new_v4());
    let mut temp_file = File::create(&temp_file_name)
        .await
        .map_err(|e| {
            error!("Failed to create temp file: {}", e);
            ApiError::InternalServerError
        })?;

    // 2. Parse multipart form and stream file to temp location
    let mut ui_name: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut author: Option<String> = None;
    let mut author_wallet: Option<String> = None;
    let mut file_format: Option<String> = None;
    let mut file_size: i64 = 0;
    // In a real scenario, this would be extracted from a JWT token.
    let user_wallet = "mock_user_wallet".to_string();

    while let Some(field) = multipart.next_field().await? {
        let name = if let Some(name) = field.name() {
            name.to_string()
        } else {
            continue;
        };

        if name == "file" {
            if let Some(file_name) = field.file_name() {
                file_format = Some(file_name.split('.').last().unwrap_or("").to_string());
            }
            let mut field_contents = field;
            while let Some(chunk) = field_contents.chunk().await? {
                temp_file.write_all(&chunk).await.map_err(|e| {
                    error!("Failed to write to temp file: {}", e);
                    ApiError::FileProcessingError
                })?;
            }
            file_size = temp_file.metadata().await?.len() as i64;
        } else {
            let data = field.bytes().await?;
            match name.as_str() {
                "ui_name" => ui_name = Some(String::from_utf8_lossy(&data).to_string()),
                "desc" => desc = Some(String::from_utf8_lossy(&data).to_string()),
                "author" => author = Some(String::from_utf8_lossy(&data).to_string()),
                "author_wallet" => author_wallet = Some(String::from_utf8_lossy(&data).to_string()),
                _ => (),
            }
        }
    }
    temp_file.flush().await?; // Ensure all bytes are written

    // 3. Calculate file hash
    let mut hasher = Sha256::new();
    temp_file.seek(SeekFrom::Start(0)).await?; // Rewind file
    let mut buffer = [0; 1024];
    loop {
        let n = temp_file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let file_hash = format!("{:x}", hasher.finalize());

    // 4. Start transaction and orchestrate services
    let mut tx = state.db.begin().await.map_err(|e| {
        error!("Failed to begin transaction: {}", e);
        ApiError::DatabaseError(e.to_string())
    })?;

    let new_dataset = match async {
        // 4a. (Simulated) Upload to IPFS
        info!("Uploading file to IPFS...");
        let ipfs_cid = state.ipfs_service.upload_file(&temp_file_name).await?;

        // 4b. (Simulated) Register on blockchain
        info!("Registering dataset on blockchain...");
        let key_context = KeyContext::new(
            KeyKind::EthAccount,
            &user_wallet, // Use user wallet as the context name for derivation
            "Registering a static dataset",
        );
        let tx_hash = state
            .ctr_caller_service
            .register_static_dataset(
                &user_wallet,
                &ipfs_cid,
                &ui_name.clone().unwrap_or_default(),
                &key_context,
            )
            .await?;

        // 4c. Create DB entry for the dataset
        info!("Creating dataset record in database...");
        let req = CreateStcDatasetReq {
            name: ui_name.clone().unwrap_or_default(),
            ui_name: ui_name.unwrap_or_default(),
            desc: desc.unwrap_or_default(),
            file_hash,
            ipfs_cid: ipfs_cid.clone(),
            file_size,
            file_format: file_format.unwrap_or_default(),
            author: author.unwrap_or_default(),
            author_wallet: author_wallet.unwrap_or_default(),
            sample_url: format!("/api/sample/{}", ipfs_cid),
            file_path: temp_file_name.clone(),
        };
        let dataset = StaticDataset::create_in_tx(&mut tx, &user_wallet, &req).await?;

        // 4d. Create corresponding blockchain transaction record
        info!("Logging blockchain transaction...");
        let args = json!({
            "name": req.name,
            "file_hash": req.file_hash,
            "ipfs_cid": req.ipfs_cid,
        });
        BlockchainTransaction::create(&mut tx, &tx_hash, dataset.id, ENTITY_TYPE_STATIC_DATASET, &args)
            .await?;

        Ok::<_, ApiError>(dataset)
    }
    .await
    {
        Ok(dataset) => dataset,
        Err(e) => {
            error!("Error during dataset creation transaction, rolling back: {}", e);
            tx.rollback().await?;
            return Err(e);
        }
    };

    // 5. Commit transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        ApiError::DatabaseError(e.to_string())
    })?;

    // 6. Cleanup temporary file
    fs::remove_file(&temp_file_name).await.map_err(|e| {
        error!("Failed to remove temp file '{}': {}", temp_file_name, e);
        // Don't fail the whole request for this, just log it.
        ApiError::FileProcessingError
    })?;

    info!("Successfully created static dataset with ID: {}", new_dataset.id);
    Ok(Json(ApiResponse::success(new_dataset)))
}

/// Update an existing static dataset
#[instrument(skip(state), fields(id = %id))]
pub async fn update_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateStaticDatasetRequest>,
) -> ApiResult<Json<ApiResponse<StaticDataset>>> {
    info!("Updating dataset");
    let dataset = StaticDataset::update(&state.db, id, req).await?;
    Ok(Json(ApiResponse::success(dataset)))
}

/// Get a sample for a static dataset by its CID
#[instrument(skip(_state), fields(cid = %cid))]
pub async fn sample_dataset(
    State(_state): State<Arc<AppState>>,
    Path(cid): Path<String>,
) -> ApiResult<Json<ApiResponse<String>>> {
    info!("Getting dataset sample");
    // In a real scenario, this would involve fetching from a specific service or file path.
    let sample_data = format!("This is a sample for dataset with CID: {}", cid);
    Ok(Json(ApiResponse::success(sample_data)))
} 