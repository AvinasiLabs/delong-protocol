use alloy::primitives::Address;
use avinapi::prelude::{AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedQuery};
use axum::extract::{Multipart, State};
use ipfs_api_backend_hyper::IpfsApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tracing::info;
use validator::Validate;

use crate::{
    models::{
        blockchain_transaction::{CreateTransaction, EntityType},
        dataset::{CreateDatasetRequest, Dataset},
        Create,
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
    pub tx_hash: String,
}

/// Form data for creating dataset (for validation after multipart parsing)
#[derive(Debug, Validate)]
struct CreateDatasetForm {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: String,
    #[validate(regex(
        path = "crate::ETHEREUM_ADDRESS_REGEX",
        message = "Invalid Ethereum address"
    ))]
    pub author_wallet: String,
    pub ui_name: String,
    pub desc: Option<String>,
    pub file_format: String,
    pub author: Option<String>,
    pub sample_url: Option<String>,
}

/// Create a new dataset
pub async fn create_dataset(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> JsonResult<CreateDatasetResponse> {
    // Parse multipart form data
    let mut form_data = CreateDatasetForm {
        name: String::new(),
        author_wallet: String::new(),
        ui_name: String::new(),
        desc: None,
        file_format: String::new(),
        author: None,
        sample_url: None,
    };

    let mut file_data: Vec<u8> = Vec::new();
    let mut file_path: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("Failed to read multipart field: {}", e)))?
    {
        let field_name = field
            .name()
            .ok_or_else(|| AppError::Validation("Field name is missing".to_string()))?
            .to_string();

        match field_name.as_str() {
            "file" => {
                file_path = field.file_name().map(|s| s.to_string());
                file_data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(format!("Failed to read file: {}", e)))?
                    .to_vec();
            }
            "name" => {
                form_data.name = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("Failed to read name: {}", e)))?;
            }
            "author_wallet" => {
                form_data.author_wallet = field.text().await.map_err(|e| {
                    AppError::Validation(format!("Failed to read author_wallet: {}", e))
                })?;
            }
            "ui_name" => {
                form_data.ui_name = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("Failed to read ui_name: {}", e)))?;
            }
            "desc" => {
                form_data.desc =
                    Some(field.text().await.map_err(|e| {
                        AppError::Validation(format!("Failed to read desc: {}", e))
                    })?);
            }
            "file_format" => {
                form_data.file_format = field.text().await.map_err(|e| {
                    AppError::Validation(format!("Failed to read file_format: {}", e))
                })?;
            }
            "author" => {
                form_data.author =
                    Some(field.text().await.map_err(|e| {
                        AppError::Validation(format!("Failed to read author: {}", e))
                    })?);
            }
            "sample_url" => {
                form_data.sample_url = Some(field.text().await.map_err(|e| {
                    AppError::Validation(format!("Failed to read sample_url: {}", e))
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate that we have a file
    if file_data.is_empty() {
        return Err(AppError::Validation("No file uploaded".to_string()));
    }

    // Validate form data
    form_data
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    // Calculate file hash
    let mut hasher = Sha256::new();
    hasher.update(&file_data);
    let file_hash = format!("0x{}", hex::encode(hasher.finalize()));

    // Check for duplicate file hash
    if let Some(_) = Dataset::find_by_file_hash(state.db.pool(), &file_hash).await? {
        return Err(AppError::Conflict(
            "Dataset with this file hash already exists".to_string(),
        ));
    }

    // Upload to IPFS
    let ipfs_client = &state.ipfs_client;

    let cursor = std::io::Cursor::new(bytes::Bytes::from(file_data.clone()));
    let ipfs_result = ipfs_client
        .add(cursor)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to upload to IPFS: {}", e)))?;

    let ipfs_cid = ipfs_result.hash;

    // Create dataset record
    let request = CreateDatasetRequest {
        name: form_data.name.clone(),
        ui_name: form_data.ui_name,
        desc: form_data.desc,
        file_hash: file_hash.clone(),
        ipfs_cid: ipfs_cid.clone(),
        file_size: file_data.len() as i64,
        file_format: form_data.file_format,
        author: form_data.author,
        author_wallet: form_data.author_wallet.clone(),
        sample_url: form_data.sample_url,
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

    // Start database transaction for blockchain transaction record
    let mut tx = state.db.pool().begin().await?;

    CreateTransaction::create(&mut tx, create_tx).await?;

    // Commit transaction
    tx.commit().await?;

    info!("Dataset {} registered successfully", dataset.id);

    avinapi::data!(CreateDatasetResponse { tx_hash })
}

/// List datasets with pagination
pub async fn list_datasets(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<DatasetResponse> {
    let page = query.page;
    let per_page = query.per_page;

    // Validate pagination
    if page == 0 {
        return Err(AppError::Validation(
            "Page must be greater than 0".to_string(),
        ));
    }
    if per_page == 0 || per_page > 100 {
        return Err(AppError::Validation(
            "Per page must be between 1 and 100".to_string(),
        ));
    }

    let (datasets, total) = Dataset::find_all_confirmed(state.db.pool(), page, per_page).await?;

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

    avinapi::paginated!(items, total, page, per_page)
}
