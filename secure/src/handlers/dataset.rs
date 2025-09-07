use alloy::primitives::{Address, U256};
use avinapi::prelude::{
    data, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedMultipartForm, ValidatedQuery,
};
use axum::extract::{Path, State};
use chrono::Utc;
use ipfs_api_backend_hyper::IpfsApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tracing::{info, warn};
use validator::Validate;

use crate::{
    models::{
        blockchain_transaction::{BlockchainTransaction, TransactionStatus},
        dataset::{CreateDatasetRequest, Dataset},
        Create, FindById,
    },
    routes::AppState,
    workers::chainsync::transaction_monitor::{PendingTransaction, TransactionMonitor},
};

/// Response for dataset
#[derive(Debug, Serialize, Deserialize)]
pub struct DatasetResponse {
    pub id: u64,
    pub name: String,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub author_wallet: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Response for dataset creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDatasetResponse {
    pub tx_hash: String,
}

/// Request for updating dataset
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateDatasetRequest {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: String,
    pub desc: Option<String>,
}

/// Form data for creating dataset with file handling
/// This is specifically designed to work with multipart form data that includes a file
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateDatasetForm {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: String,
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address format. Expected: '0x' followed by 40 hexadecimal characters (e.g., 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb9)"
    ))]
    pub author_wallet: String,
    pub ui_name: String,
    pub desc: Option<String>,
    pub file_format: String,
    pub author: Option<String>,
}

/// Create a new dataset with file upload
pub async fn create_dataset(
    State(state): State<AppState>,
    ValidatedMultipartForm {
        form: form_data,
        files,
    }: ValidatedMultipartForm<CreateDatasetForm>,
) -> JsonResult<CreateDatasetResponse> {
    // Extract file data from the files HashMap
    let file_data = files
        .get("file")
        .ok_or_else(|| AppError::Validation("No file uploaded".to_string()))?;

    let file_path = file_data.filename.clone();
    let file_bytes = &file_data.bytes;

    // Validate that we have actual file content
    if file_bytes.is_empty() {
        return Err(AppError::Validation("Uploaded file is empty".to_string()));
    }

    // Calculate file hash
    let mut hasher = Sha256::new();
    hasher.update(file_bytes);
    let file_hash = format!("0x{}", hex::encode(hasher.finalize()));

    // Check for duplicate file hash
    if let Some(_) = Dataset::find_by_file_hash(state.db.pool(), &file_hash).await? {
        return Err(AppError::Conflict(
            "Dataset with this file hash already exists".to_string(),
        ));
    }

    // Get the author for key derivation (use author field if provided, otherwise use wallet address)
    let key_author = form_data
        .author
        .as_ref()
        .unwrap_or(&form_data.author_wallet);

    // Encrypt the file data using TEE-derived key
    info!("Encrypting dataset for author: {}", key_author);
    let encrypted_data = state
        .tee_crypto
        .encrypt_dataset(file_bytes, key_author)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to encrypt dataset: {}", e)))?;

    // Upload encrypted data to IPFS
    info!("Uploading encrypted dataset to IPFS");
    let ipfs_client = &state.ipfs_client;
    let cursor = std::io::Cursor::new(bytes::Bytes::from(encrypted_data));
    let ipfs_result = ipfs_client
        .add(cursor)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to upload to IPFS: {}", e)))?;

    let ipfs_cid = ipfs_result.hash;
    info!("Dataset uploaded to IPFS with CID: {}", ipfs_cid);

    // Generate sample URL for CSV files
    let mut sample_url: Option<String> = None;
    if form_data.file_format.to_lowercase() == "csv" {
        info!("Generating sample data for CSV file");

        match state
            .sample_generator
            .generate_csv_sample(file_bytes, None)
            .await
        {
            Ok(sample_csv) => {
                // Upload sample to IPFS (without encryption)
                info!("Uploading sample data to IPFS");
                let sample_cursor =
                    std::io::Cursor::new(bytes::Bytes::from(sample_csv.into_bytes()));
                match ipfs_client.add(sample_cursor).await {
                    Ok(sample_result) => {
                        let sample_cid = sample_result.hash;
                        sample_url = Some(format!("/api/sample/{}", sample_cid));
                        info!("Sample uploaded with CID: {}", sample_cid);
                    }
                    Err(e) => {
                        warn!("Failed to upload sample to IPFS: {}", e);
                        // Don't fail the entire operation if sample upload fails
                    }
                }
            }
            Err(e) => {
                warn!("Failed to generate sample CSV: {}", e);
                // Don't fail the entire operation if sample generation fails
            }
        }
    }

    // Check if dataset with same file hash already exists
    let existing = Dataset::find_by_file_hash_with_status(state.db.pool(), &file_hash).await?;

    if let Some(existing_dataset) = existing {
        match existing_dataset.tx_status {
            Some(TransactionStatus::Confirmed) => {
                // Already confirmed, return existing dataset
                info!("Dataset with file_hash {} already confirmed", file_hash);
                return Err(AppError::Conflict(
                    "Dataset already exists and was confirmed on blockchain.".to_string(),
                ));
            }
            Some(TransactionStatus::Pending) => {
                // Pending status, tell user to retry later (timeout check handled by background task)
                return Err(AppError::Conflict(format!(
                    "Dataset is being processed on blockchain (tx: {}). Please try again later.",
                    existing_dataset.tx_hash.unwrap_or_default()
                )));
            }
            Some(TransactionStatus::Failed) | None => {
                // Failed or no transaction, reuse existing dataset and retry submission
                info!(
                    "Retrying blockchain submission for existing dataset {}",
                    existing_dataset.dataset.id
                );

                // Parse author wallet address
                let author_address = Address::from_str(&existing_dataset.dataset.author_wallet)
                    .map_err(|e| AppError::Validation(format!("Invalid wallet address: {}", e)))?;

                // Retry submission to blockchain
                let tx_hash = match state
                    .contract_caller
                    .register_data(
                        author_address,
                        existing_dataset.dataset.ipfs_cid.clone(),
                        U256::from(existing_dataset.dataset.id as u64),
                        existing_dataset.dataset.name.clone(),
                    )
                    .await
                {
                    Ok(hash) => hash,
                    Err(e) => {
                        // Blockchain submission failed, create failed transaction record
                        warn!(
                            "Blockchain submission failed for dataset {}: {}",
                            existing_dataset.dataset.id, e
                        );
                        BlockchainTransaction::create_failed(
                            state.db.pool(),
                            existing_dataset.dataset.id,
                            crate::models::blockchain_transaction::EntityType::Dataset,
                            &e.to_string(),
                        )
                        .await?;

                        return Err(AppError::Internal(format!(
                            "Failed to register dataset on blockchain: {}",
                            e
                        )));
                    }
                };

                info!(
                    "Dataset {} re-registered on blockchain with tx hash: {}",
                    existing_dataset.dataset.id, tx_hash
                );

                // Create/update blockchain_transaction record
                BlockchainTransaction::upsert_for_dataset(
                    state.db.pool(),
                    &tx_hash,
                    existing_dataset.dataset.id,
                )
                .await?;

                // Add to Redis monitoring queue for retry
                let pending_tx = PendingTransaction {
                    tx_hash: tx_hash.clone(),
                    entity_id: existing_dataset.dataset.id,
                    entity_type: "dataset".to_string(),
                    nonce: 0, // TODO: Get actual nonce from transaction
                    created_at: Utc::now(),
                };

                if let Err(e) = TransactionMonitor::add_to_queue(&state.redis, pending_tx).await {
                    warn!(
                        "Failed to add retry transaction {} to monitoring queue: {}",
                        tx_hash, e
                    );
                } else {
                    info!("Retry transaction {} added to monitoring queue", tx_hash);
                }

                return avinapi::data!(CreateDatasetResponse { tx_hash });
            }
        }
    }

    // Create dataset record
    let request = CreateDatasetRequest {
        name: form_data.name.clone(),
        ui_name: form_data.ui_name,
        desc: form_data.desc,
        file_hash: file_hash.clone(),
        ipfs_cid: ipfs_cid.clone(),
        file_size: file_bytes.len() as i64,
        file_format: form_data.file_format,
        author: form_data.author,
        author_wallet: form_data.author_wallet.clone(),
        sample_url,
        file_path,
    };

    let dataset = Dataset::create(state.db.pool(), request).await?;

    // Parse author wallet address
    let author_address = Address::from_str(&dataset.author_wallet)
        .map_err(|e| AppError::Validation(format!("Invalid wallet address: {}", e)))?;

    // Submit to blockchain
    let tx_hash = match state
        .contract_caller
        .register_data(
            author_address,
            dataset.ipfs_cid.clone(),
            U256::from(dataset.id as u64),
            dataset.name.clone(),
        )
        .await
    {
        Ok(hash) => hash,
        Err(e) => {
            // Blockchain submission failed, create failed transaction record
            warn!(
                "Blockchain submission failed for dataset {}: {}",
                dataset.id, e
            );
            BlockchainTransaction::create_failed(
                state.db.pool(),
                dataset.id,
                crate::models::blockchain_transaction::EntityType::Dataset,
                &e.to_string(),
            )
            .await?;

            return Err(AppError::Internal(format!(
                "Failed to register dataset on blockchain: {}",
                e
            )));
        }
    };

    info!("Dataset registered on blockchain with tx hash: {}", tx_hash);

    // Create blockchain transaction record using conditional insert
    // This will not overwrite if a confirmed record already exists
    let was_updated =
        BlockchainTransaction::upsert_for_dataset(state.db.pool(), &tx_hash, dataset.id).await?;

    if was_updated {
        info!(
            "Blockchain transaction record created for dataset {}",
            dataset.id
        );
    } else {
        info!("Blockchain transaction record already exists and is confirmed, skipped update");
    }

    // Add to Redis monitoring queue
    // Get current nonce from contract caller (assuming we track it)
    // For now, use a placeholder nonce - this should be obtained from the actual transaction
    let pending_tx = PendingTransaction {
        tx_hash: tx_hash.clone(),
        entity_id: dataset.id,
        entity_type: "dataset".to_string(),
        nonce: 0, // TODO: Get actual nonce from transaction
        created_at: Utc::now(),
    };

    // Add to Redis queue for monitoring
    if let Err(e) = TransactionMonitor::add_to_queue(&state.redis, pending_tx).await {
        warn!(
            "Failed to add transaction {} to monitoring queue: {}",
            tx_hash, e
        );
        // Continue anyway - monitoring is not critical for success
    } else {
        info!("Transaction {} added to monitoring queue", tx_hash);
    }

    info!("Dataset {} registered successfully", dataset.id);

    avinapi::data!(CreateDatasetResponse { tx_hash })
}

/// List datasets with pagination
pub async fn list_datasets(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<DatasetResponse> {
    let (datasets, total) =
        Dataset::find_all_confirmed(state.db.pool(), query.page, query.per_page).await?;

    let items: Vec<DatasetResponse> = datasets.into_iter().map(|d| d.to_response()).collect();

    avinapi::paginated!(items, total, query.page, query.per_page)
}

/// Update an existing dataset (requires admin)
pub async fn update_dataset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    ValidatedJson(req): ValidatedJson<UpdateDatasetRequest>,
) -> JsonResult<DatasetResponse> {
    // TODO: Check admin permission from request headers
    // For now, we'll skip this check in development

    // Find existing dataset
    let _dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    // Update dataset using model method
    let updated = Dataset::update_metadata(
        state.db.pool(),
        id,
        &req.name, // ui_name
        &req.name, // name
        req.desc.as_deref(),
    )
    .await?;

    info!("Dataset {} updated successfully", id);

    data!(updated.to_response())
}

/// Delete a dataset (requires admin)
pub async fn delete_dataset(State(state): State<AppState>, Path(id): Path<i64>) -> JsonResult<()> {
    // TODO: Check admin permission from request headers
    // For now, we'll skip this check in development

    // Check if dataset exists
    let _dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    // Delete dataset using model method (this should cascade to related records based on DB constraints)
    Dataset::delete(state.db.pool(), id).await?;

    info!("Dataset {} deleted successfully", id);

    avinapi::empty!()
}

/// Get a specific dataset by ID
pub async fn get_dataset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> JsonResult<DatasetResponse> {
    let dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    data!(dataset.to_response())
}

/// Get dataset status including blockchain transaction status
pub async fn get_dataset_status(
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> JsonResult<serde_json::Value> {
    let dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    // Query transaction status
    let tx_status = sqlx::query!(
        r#"
        SELECT status as "status: TransactionStatus", tx_hash, block_number, created_at, updated_at
        FROM blockchain_transaction
        WHERE entity_id = $1 AND entity_type = 'dataset'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(state.db.pool())
    .await?;

    let response = serde_json::json!({
        "dataset": dataset.to_response(),
        "blockchain_status": tx_status.map(|t| {
            serde_json::json!({
                "status": t.status,
                "tx_hash": t.tx_hash,
                "block_number": t.block_number,
                "created_at": t.created_at,
                "updated_at": t.updated_at,
            })
        }),
    });

    data!(response)
}
