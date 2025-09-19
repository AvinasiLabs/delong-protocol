use alloy::primitives::{Address, U256};
use avinapi::prelude::{
    data, AppError, JsonResult, PaginatedResult, PaginationQuery, ValidatedJson,
    ValidatedMultipartForm, ValidatedQuery,
};
use axum::extract::{Extension, Path, Query, State};
use chrono::Utc;
use ipfs_api_backend_hyper::IpfsApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tracing::{info, warn};
use validator::Validate;

use crate::{
    middleware::AuthenticatedUser,
    models::{
        blockchain_transaction::{BlockchainTransaction, EntityType, TransactionStatus},
        dataset::{CreateDatasetRequest, Dataset},
        dataset_tag::DatasetTag,
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
    pub ui_name: String,
    pub desc: Option<String>,
    pub file_hash: String,
    pub ipfs_cid: String,
    pub file_size: u64,
    pub file_format: String,
    pub author: Option<String>,
    pub wallet: String,
    pub author_avatar: Option<String>, // ADD avatar URL
    pub sample_url: Option<String>,
    pub file_path: Option<String>,
    // New fields from migration
    pub author_id: Option<u64>,
    pub is_encrypted: bool,
    pub slug: Option<String>,
    pub license: Option<String>,
    pub thumbnail_url: Option<String>,
    pub version: Option<String>,
    pub detailed_desc: Option<String>,
    pub views_count: u64,
    pub tags: Vec<String>,
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
    pub ui_name: Option<String>,
    pub desc: Option<String>,
    pub detailed_desc: Option<String>,
    pub category: Option<String>,
    pub license: Option<String>,
    pub thumbnail_url: Option<String>,
    pub version: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Query parameters for searching datasets
#[derive(Debug, Deserialize, Validate)]
pub struct SearchDatasetQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub tag: Option<String>,        // Single tag parameter
    pub tags: Option<Vec<String>>,  // Multiple tags parameter
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
    pub wallet: String,
    pub ui_name: String,
    pub desc: Option<String>,
    pub file_format: String,
    pub author: Option<String>,
    // New fields from migration
    pub author_id: Option<i64>,
    pub is_encrypted: Option<bool>,
    pub slug: Option<String>,
    pub license: Option<String>,
    pub thumbnail_url: Option<String>,
    pub version: Option<String>,
    pub detailed_desc: Option<String>,
    pub tags: Option<String>, // JSON array of tags as string
}

/// Create a new dataset with file upload
use crate::models::{
    AddTagsRequest, DatasetSchema, RemoveTagsRequest, SetDatasetSchemaRequest, TagWithCount,
};

pub async fn create_dataset(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedUser>, // Extract user from JWT
    ValidatedMultipartForm {
        form: mut form_data,
        files,
    }: ValidatedMultipartForm<CreateDatasetForm>,
) -> JsonResult<DatasetResponse> {
    // Return full response
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

    // Parse user_id from string
    let user_id: i64 = auth
        .user_id
        .parse()
        .map_err(|_| AppError::Validation("Invalid user ID".to_string()))?;

    // Update form data with user information
    form_data.author_id = Some(user_id);
    form_data.author = Some(auth.username.clone());

    // Use a universal identifier for dataset encryption
    // All datasets use the same TEE-derived key so any authorized user can decrypt
    let dataset_key_id = "UNIVERSAL_DATASET_KEY";

    // Encrypt the file data using TEE-derived key
    info!("Encrypting dataset with universal TEE key");
    let encrypted_data = state
        .tee_crypto
        .encrypt_dataset(file_bytes, dataset_key_id)
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
                let author_address = Address::from_str(&existing_dataset.dataset.wallet)
                    .map_err(|e| AppError::Validation(format!("Invalid wallet address: {}", e)))?;

                // Retry submission to blockchain
                let tx_hash = match state
                    .contract_caller
                    .register_dataset(
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

                // Create/update transaction record
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
                    entity_type: EntityType::Dataset,
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

                // For existing datasets, we should return the full dataset response
                // but we need to fetch it first
                let existing_data = existing_dataset.dataset;
                let dataset_response = DatasetResponse {
                    id: existing_data.id as u64,
                    name: existing_data.name,
                    ui_name: existing_data.ui_name,
                    desc: existing_data.desc,
                    file_hash: existing_data.file_hash,
                    ipfs_cid: existing_data.ipfs_cid,
                    file_size: existing_data.file_size as u64,
                    file_format: existing_data.file_format,
                    author: existing_data.author,
                    wallet: existing_data.wallet,
                    sample_url: existing_data.sample_url,
                    file_path: existing_data.file_path,
                    author_id: existing_data.author_id.map(|id| id as u64),
                    is_encrypted: existing_data.is_encrypted,
                    slug: existing_data.slug,
                    license: existing_data.license,
                    thumbnail_url: existing_data.thumbnail_url,
                    version: existing_data.version,
                    detailed_desc: existing_data.detailed_desc,
                    views_count: existing_data.views_count as u64,
                    tags: vec![],
                    author_avatar: auth.avatar_url.clone(),
                    created_at: existing_data.created_at,
                    updated_at: existing_data.updated_at,
                };
                return avinapi::data!(dataset_response);
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
        author: Some(auth.username.clone()), // Use username from auth context
        wallet: form_data.wallet.clone(),
        sample_url,
        file_path,
        // New fields from migration
        author_id: auth.user_id.parse::<i64>().ok(), // Parse user_id from auth context
        is_encrypted: form_data.is_encrypted,
        slug: form_data.slug,
        license: form_data.license,
        thumbnail_url: form_data.thumbnail_url,
        version: form_data.version,
        detailed_desc: form_data.detailed_desc,
    };

    let dataset = Dataset::create(state.db.pool(), request).await?;

    // Process tags if provided
    let mut tags = Vec::new();
    if let Some(tags_json) = &form_data.tags {
        if let Ok(parsed_tags) = serde_json::from_str::<Vec<String>>(tags_json) {
            use crate::models::DatasetTag;
            for tag in &parsed_tags {
                DatasetTag::add_tag(state.db.pool(), dataset.id as i64, &tag).await?;
            }
            tags = parsed_tags;
        }
    }

    // Parse author wallet address
    let author_address = Address::from_str(&dataset.wallet)
        .map_err(|e| AppError::Validation(format!("Invalid wallet address: {}", e)))?;

    // Submit to blockchain
    let tx_hash = match state
        .contract_caller
        .register_dataset(
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
        entity_type: EntityType::Dataset,
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

    // Return complete dataset response with user information
    avinapi::data!(DatasetResponse {
        id: dataset.id as u64,
        name: dataset.name,
        ui_name: dataset.ui_name,
        desc: dataset.desc,
        file_hash: dataset.file_hash,
        ipfs_cid: dataset.ipfs_cid,
        file_size: dataset.file_size as u64,
        file_format: dataset.file_format,
        author: dataset.author,
        wallet: dataset.wallet,
        author_avatar: auth.avatar_url.clone(),
        sample_url: dataset.sample_url,
        file_path: dataset.file_path,
        author_id: dataset.author_id.map(|id| id as u64),
        is_encrypted: dataset.is_encrypted,
        slug: dataset.slug,
        license: dataset.license,
        thumbnail_url: dataset.thumbnail_url,
        version: dataset.version,
        detailed_desc: dataset.detailed_desc,
        views_count: dataset.views_count as u64,
        tags,
        created_at: dataset.created_at,
        updated_at: dataset.updated_at,
    })
}

/// List datasets with pagination
pub async fn list_datasets(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<DatasetResponse> {
    let (datasets, total) =
        Dataset::find_all_confirmed(state.db.pool(), query.page, query.per_page).await?;

    let mut items = Vec::new();
    for dataset in datasets {
        items.push(dataset.to_response(state.db.pool()).await?);
    }

    avinapi::paginated!(items, total, query.page, query.per_page)
}

/// Update an existing dataset (requires admin)
pub async fn update_dataset(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<i64>,
    ValidatedJson(req): ValidatedJson<UpdateDatasetRequest>,
) -> JsonResult<DatasetResponse> {
    // Check if user owns the dataset or is admin
    // For now, we'll just log the user making the update
    info!("User {} updating dataset {}", auth.username, id);

    // Find existing dataset
    let _dataset = Dataset::find_by_id(state.db.pool(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", id)))?;

    // Update dataset using model method
    let ui_name = req.ui_name.as_deref().unwrap_or(&req.name);
    let updated =
        Dataset::update_metadata(state.db.pool(), id, ui_name, &req.name, req.desc.as_deref())
            .await?;

    // Update tags if provided
    if let Some(tags) = req.tags {
        DatasetTag::update_dataset_tags(state.db.pool(), id, &tags).await?;
    }

    info!("Dataset {} updated successfully", id);

    data!(updated.to_response(state.db.pool()).await?)
}

/// Delete a dataset (requires admin)
pub async fn delete_dataset(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<i64>,
) -> JsonResult<()> {
    info!("User {} deleting dataset {}", auth.username, id);
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

    // Increment view count
    Dataset::increment_views(state.db.pool(), id).await?;

    data!(dataset.to_response(state.db.pool()).await?)
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
        FROM transaction
        WHERE entity_id = $1 AND entity_type = 'dataset'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(state.db.pool())
    .await?;

    let dataset_response = dataset.to_response(state.db.pool()).await?;
    let response = serde_json::json!({
        "dataset": dataset_response,
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

/// Get dataset by slug
pub async fn get_dataset_by_slug(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> JsonResult<DatasetResponse> {
    let dataset = Dataset::find_by_slug(state.db.pool(), &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Dataset with slug '{}' not found", slug)))?;

    // Increment view count
    Dataset::increment_views(state.db.pool(), dataset.id).await?;

    data!(dataset.to_response(state.db.pool()).await?)
}

/// Get featured datasets (based on usage)
pub async fn get_featured_datasets(
    State(state): State<AppState>,
) -> JsonResult<Vec<DatasetResponse>> {
    let datasets = Dataset::get_featured(state.db.pool(), 6).await?;

    let mut items = Vec::new();
    for dataset in datasets {
        items.push(dataset.to_response(state.db.pool()).await?);
    }

    data!(items)
}

/// Get trending datasets (based on recent views)
pub async fn get_trending_datasets(
    State(state): State<AppState>,
) -> JsonResult<Vec<DatasetResponse>> {
    let datasets = Dataset::get_trending(state.db.pool(), 6).await?;

    let mut items = Vec::new();
    for dataset in datasets {
        items.push(dataset.to_response(state.db.pool()).await?);
    }

    data!(items)
}

/// Search datasets
pub async fn search_datasets(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<SearchDatasetQuery>,
    ValidatedQuery(pagination): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<DatasetResponse> {
    // Combine tag and tags parameters
    let all_tags = match (&query.tag, &query.tags) {
        (Some(single_tag), Some(multi_tags)) => {
            // If both are provided, combine them
            let mut combined = vec![single_tag.clone()];
            combined.extend(multi_tags.clone());
            Some(combined)
        }
        (Some(single_tag), None) => {
            // Only single tag provided
            Some(vec![single_tag.clone()])
        }
        (None, Some(multi_tags)) => {
            // Only multiple tags provided
            Some(multi_tags.clone())
        }
        (None, None) => None,
    };

    let (datasets, total) = if let Some(keyword) = &query.q {
        Dataset::search(
            state.db.pool(),
            keyword,
            pagination.page,
            pagination.per_page,
        )
        .await?
    } else if let Some(_category) = &query.category {
        // Category field is deprecated, return empty result
        (vec![], 0)
    } else if let Some(tags) = all_tags {
        // Find datasets by tags
        let (dataset_ids, count) = DatasetTag::find_datasets_by_tags_all(
            state.db.pool(),
            &tags,
            pagination.page,
            pagination.per_page,
        )
        .await?;

        let mut datasets = Vec::new();
        for id in dataset_ids {
            if let Some(dataset) = Dataset::find_by_id(state.db.pool(), id).await? {
                datasets.push(dataset);
            }
        }
        (datasets, count)
    } else {
        Dataset::find_all_confirmed(
            state.db.pool(),
            pagination.page,
            pagination.per_page,
        )
        .await?
    };

    let mut items = Vec::new();
    for dataset in datasets {
        items.push(dataset.to_response(state.db.pool()).await?);
    }

    avinapi::paginated!(
        items,
        total,
        pagination.page,
        pagination.per_page
    )
}

/// Get datasets by category
// Category-based filtering is deprecated - use tags instead
// Keeping this function for backward compatibility but returning empty results
pub async fn get_datasets_by_category(
    State(_state): State<AppState>,
    Path(_category): Path<String>,
    ValidatedQuery(query): ValidatedQuery<PaginationQuery>,
) -> PaginatedResult<DatasetResponse> {
    // Return empty result - category field is deprecated
    avinapi::paginated!(Vec::<DatasetResponse>::new(), 0, query.page, query.per_page)
}

/// Add tags to a dataset
pub async fn add_dataset_tags(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<i64>,
    ValidatedJson(req): ValidatedJson<AddTagsRequest>,
) -> JsonResult<Vec<String>> {
    info!("User {} adding tags to dataset {}", auth.username, id);
    // Check if dataset exists
    Dataset::find_by_id_required(state.db.pool(), id).await?;

    DatasetTag::add_tags(state.db.pool(), id, &req.tags).await?;

    let tags = DatasetTag::get_dataset_tags(state.db.pool(), id).await?;

    data!(tags)
}

/// Remove tags from a dataset
pub async fn remove_dataset_tags(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<i64>,
    ValidatedJson(req): ValidatedJson<RemoveTagsRequest>,
) -> JsonResult<Vec<String>> {
    info!("User {} removing tags from dataset {}", auth.username, id);
    // Check if dataset exists
    Dataset::find_by_id_required(state.db.pool(), id).await?;

    DatasetTag::remove_tags(state.db.pool(), id, &req.tags).await?;

    let tags = DatasetTag::get_dataset_tags(state.db.pool(), id).await?;

    data!(tags)
}

/// Get dataset tags
pub async fn get_dataset_tags(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> JsonResult<Vec<String>> {
    // Check if dataset exists
    Dataset::find_by_id_required(state.db.pool(), id).await?;

    let tags = DatasetTag::get_dataset_tags(state.db.pool(), id).await?;

    data!(tags)
}

/// Get all tags with counts
pub async fn get_all_tags(State(state): State<AppState>) -> JsonResult<Vec<TagWithCount>> {
    let tags = DatasetTag::get_all_tags_with_count(state.db.pool()).await?;

    data!(tags)
}

/// Get popular tags
pub async fn get_popular_tags(State(state): State<AppState>) -> JsonResult<Vec<TagWithCount>> {
    let tags = DatasetTag::get_popular_tags(state.db.pool(), 20).await?;

    data!(tags)
}

/// Get dataset schema
pub async fn get_dataset_schema(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> JsonResult<Vec<DatasetSchema>> {
    // Check if dataset exists
    Dataset::find_by_id_required(state.db.pool(), id).await?;

    let schema = DatasetSchema::get_by_dataset_id(state.db.pool(), id).await?;

    data!(schema)
}

/// Set dataset schema
pub async fn set_dataset_schema(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<i64>,
    ValidatedJson(req): ValidatedJson<SetDatasetSchemaRequest>,
) -> JsonResult<Vec<DatasetSchema>> {
    info!("User {} setting schema for dataset {}", auth.username, id);
    // Check if dataset exists
    Dataset::find_by_id_required(state.db.pool(), id).await?;

    // Validate field types
    for field in &req.fields {
        if !DatasetSchema::validate_field_type(&field.field_type) {
            return Err(AppError::Validation(format!(
                "Invalid field type: {}",
                field.field_type
            )));
        }
    }

    let request = crate::models::dataset_schema::SetDatasetSchemaRequest { fields: req.fields };

    let schema = DatasetSchema::set_dataset_schema(state.db.pool(), id, request).await?;

    data!(schema)
}

/// Get dataset usage history
pub async fn get_dataset_usage(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> JsonResult<Vec<serde_json::Value>> {
    // Check if dataset exists
    let _dataset = Dataset::find_by_id_required(state.db.pool(), id).await?;

    // Get usage history (note: users table is in Core service, so we can't join here)
    let usage_history = sqlx::query!(
        r#"
        SELECT
            ae.id,
            ae.dataset_name as dataset,
            ae.algo_name,
            ae.wallet as scientist_wallet,
            ae.used_at,
            ae.execution_status,
            ae.runtime_seconds,
            ae.records_processed,
            ae.user_id
        FROM algorithm_execution ae
        LEFT JOIN dataset d ON d.name = ae.dataset_name
        WHERE d.id = $1
        ORDER BY ae.used_at DESC
        LIMIT 100
        "#,
        id
    )
    .fetch_all(state.db.pool())
    .await?;

    let mut results = Vec::new();
    for row in usage_history {
        results.push(serde_json::json!({
            "id": row.id,
            "algo_name": row.algo_name,
            "user_id": row.user_id,
            "scientist_wallet": row.scientist_wallet,
            "used_at": row.used_at,
            "execution_status": row.execution_status,
            "runtime_seconds": row.runtime_seconds,
            "records_processed": row.records_processed,
        }));
    }

    data!(results)
}

/// Query parameters for filtering datasets by tags
#[derive(Debug, Deserialize)]
pub struct TagFilterQuery {
    pub tags: Vec<String>,
    pub tag_mode: Option<String>, // "any" or "all"
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Get datasets by tags (replacement for category filtering)
pub async fn get_datasets_by_tags(
    State(state): State<AppState>,
    Query(params): Query<TagFilterQuery>,
) -> JsonResult<Vec<DatasetResponse>> {
    let tag_mode = params.tag_mode.unwrap_or_else(|| "any".to_string());

    // Get datasets that match the tags
    let datasets = if tag_mode == "all" {
        // Get datasets that have ALL specified tags
        Dataset::find_by_all_tags(state.db.pool(), &params.tags).await?
    } else {
        // Get datasets that have ANY of the specified tags
        Dataset::find_by_any_tags(state.db.pool(), &params.tags).await?
    };

    // Convert to response format with tags and user info
    let mut responses = Vec::new();
    for dataset in datasets {
        let tags = DatasetTag::get_dataset_tags(state.db.pool(), dataset.id).await?;

        // For now, we don't have user service, so just use author field
        let author_avatar: Option<String> = None;

        responses.push(DatasetResponse {
            id: dataset.id as u64,
            name: dataset.name,
            ui_name: dataset.ui_name,
            desc: dataset.desc,
            file_hash: dataset.file_hash,
            ipfs_cid: dataset.ipfs_cid,
            file_size: dataset.file_size as u64,
            file_format: dataset.file_format,
            author: dataset.author,
            wallet: dataset.wallet,
            author_avatar,
            sample_url: dataset.sample_url,
            file_path: dataset.file_path,
            author_id: dataset.author_id.map(|id| id as u64),
            is_encrypted: dataset.is_encrypted,
            slug: dataset.slug,
            license: dataset.license,
            thumbnail_url: dataset.thumbnail_url,
            version: dataset.version,
            detailed_desc: dataset.detailed_desc,
            views_count: dataset.views_count as u64,
            tags,
            created_at: dataset.created_at,
            updated_at: dataset.updated_at,
        });
    }

    data!(responses)
}
