use alloy::primitives::Address;
use avinapi::prelude::{
    data, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedMultipartForm, ValidatedQuery,
};
use axum::extract::{Path, State};
use ipfs_api_backend_hyper::IpfsApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tracing::{info, warn};
use validator::Validate;

use crate::{
    models::{
        blockchain_transaction::{CreateTransaction, EntityType},
        dataset::{CreateDatasetRequest, Dataset},
        Create, FindById,
    },
    routes::AppState,
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
#[derive(Debug, Serialize)]
pub struct CreateDatasetResponse {
    pub id: i64,
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
    let tx_hash = state
        .contract_caller
        .register_data(
            author_address,
            dataset.ipfs_cid.clone(),
            dataset.name.clone(),
        )
        .await
        .map_err(|e| {
            AppError::Internal(format!("Failed to register dataset on blockchain: {}", e))
        })?;

    info!("Dataset registered on blockchain with tx hash: {}", tx_hash);

    // Create blockchain transaction record
    let create_tx = CreateTransaction {
        tx_hash: tx_hash.clone(),
        entity_id: dataset.id,
        entity_type: EntityType::Dataset,
    };

    let mut db_tx = state.db.pool().begin().await?;

    CreateTransaction::create(&mut db_tx, create_tx).await?;

    // Commit transaction
    db_tx.commit().await?;

    info!("Dataset {} registered successfully", dataset.id);

    avinapi::data!(CreateDatasetResponse {
        id: dataset.id,
        tx_hash
    })
}

/// List datasets with pagination
pub async fn list_datasets(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<DatasetResponse> {
    let (datasets, total) =
        Dataset::find_all_confirmed(state.db.pool(), query.page, query.per_page).await?;

    let items: Vec<DatasetResponse> = datasets
        .into_iter()
        .map(|d| DatasetResponse {
            id: d.id as u64,
            name: d.name,
            file_hash: d.file_hash,
            ipfs_cid: d.ipfs_cid,
            author_wallet: d.author_wallet,
            created_at: d.created_at,
            updated_at: d.updated_at,
        })
        .collect();

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

    // Update dataset
    let updated = sqlx::query_as!(
        Dataset,
        r#"
        UPDATE dataset
        SET name = $1, ui_name = $2, "desc" = $3, updated_at = NOW()
        WHERE id = $4
        RETURNING *
        "#,
        req.name.clone(),
        req.name,
        req.desc,
        id
    )
    .fetch_one(state.db.pool())
    .await?;

    info!("Dataset {} updated successfully", id);

    let response = DatasetResponse {
        id: updated.id as u64,
        name: updated.name,
        file_hash: updated.file_hash,
        ipfs_cid: updated.ipfs_cid,
        author_wallet: updated.author_wallet,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    };

    data!(response)
}

/// Delete a dataset (requires admin)
pub async fn delete_dataset(State(state): State<AppState>, Path(id): Path<i64>) -> JsonResult<()> {
    // TODO: Check admin permission from request headers
    // For now, we'll skip this check in development

    // Check if dataset exists
    let _dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    // Delete dataset (this should cascade to related records based on DB constraints)
    sqlx::query!("DELETE FROM dataset WHERE id = $1", id)
        .execute(state.db.pool())
        .await?;

    info!("Dataset {} deleted successfully", id);

    avinapi::data!(())
}

/// Get a specific dataset by ID
pub async fn get_dataset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> JsonResult<DatasetResponse> {
    let dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    let response = DatasetResponse {
        id: dataset.id as u64,
        name: dataset.name,
        file_hash: dataset.file_hash,
        ipfs_cid: dataset.ipfs_cid,
        author_wallet: dataset.author_wallet,
        created_at: dataset.created_at,
        updated_at: dataset.updated_at,
    };

    data!(response)
}
