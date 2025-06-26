//! Data management handlers for dataset operations
//!
//! This module handles all dataset-related operations including upload,
//! retrieval, listing, and deletion of biomedical datasets.

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument, warn};

use crate::{
    handlers::{ApiResponse, PaginatedResponse, PaginationParams},
    utils::generate_request_id,
};

/// Dataset metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub dataset_type: DatasetType,
    pub size_bytes: u64,
    pub record_count: u64,
    pub upload_timestamp: String,
    pub status: DatasetStatus,
    pub owner_id: String,
    pub tags: Vec<String>,
    pub privacy_level: PrivacyLevel,
}

/// Types of datasets supported by the platform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetType {
    /// Genomic data (DNA sequences, SNPs, etc.)
    Genomic,
    /// Transcriptomic data (RNA-seq, gene expression)
    Transcriptomic,
    /// Proteomic data (protein expression, mass spectrometry)
    Proteomic,
    /// Metabolomic data (metabolite profiles)
    Metabolomic,
    /// Clinical data (medical records, diagnostics)
    Clinical,
    /// Imaging data (medical images, scans)
    Imaging,
    /// Longitudinal data (time-series health data)
    Longitudinal,
    /// Other biomedical data types
    Other,
}

/// Dataset processing status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetStatus {
    /// Dataset is being uploaded
    Uploading,
    /// Dataset is being processed and validated
    Processing,
    /// Dataset is ready for use in computations
    Ready,
    /// Dataset processing failed
    Failed,
    /// Dataset has been archived
    Archived,
}

/// Privacy level for datasets
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyLevel {
    /// Public dataset, no restrictions
    Public,
    /// Dataset requires user consent for access
    Restricted,
    /// Highly sensitive dataset, strict access controls
    Private,
}

/// Request body for dataset upload
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UploadDatasetRequest {
    pub name: String,
    pub description: Option<String>,
    pub dataset_type: DatasetType,
    pub tags: Option<Vec<String>>,
    pub privacy_level: Option<PrivacyLevel>,
    /// Base64 encoded dataset content or file reference
    pub data: String,
    /// Expected record count for validation
    pub expected_record_count: Option<u64>,
}

/// Response for successful dataset upload
#[derive(Debug, Serialize)]
pub struct UploadDatasetResponse {
    pub dataset_id: String,
    pub upload_url: Option<String>,
    pub status: DatasetStatus,
    pub estimated_processing_time_seconds: u64,
}

/// Query parameters for dataset listing
#[derive(Debug, Deserialize)]
pub struct DatasetListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub dataset_type: Option<DatasetType>,
    pub status: Option<DatasetStatus>,
    pub tags: Option<String>, // Comma-separated list
    pub search: Option<String>,
}

/// Upload a new dataset
#[instrument(skip(payload), fields(request_id))]
pub async fn upload_dataset_handler(
    Json(payload): Json<UploadDatasetRequest>,
) -> Result<Json<ApiResponse<UploadDatasetResponse>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_name = payload.name,
        dataset_type = ?payload.dataset_type,
        "Dataset upload request received"
    );

    // Validate request
    if payload.name.trim().is_empty() {
        warn!(request_id = request_id, "Dataset upload failed: empty name");
        return Err(StatusCode::BAD_REQUEST);
    }

    if payload.data.is_empty() {
        warn!(
            request_id = request_id,
            "Dataset upload failed: no data provided"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Forward to core service for actual processing
    // This is a mock implementation for now
    let dataset_id = format!("ds_{}", uuid::Uuid::new_v4().simple());
    let estimated_processing_time = estimate_processing_time(&payload);

    let response_data = UploadDatasetResponse {
        dataset_id: dataset_id.clone(),
        upload_url: None, // Could be used for direct upload to storage
        status: DatasetStatus::Processing,
        estimated_processing_time_seconds: estimated_processing_time,
    };

    info!(
        request_id = request_id,
        dataset_id = dataset_id,
        "Dataset upload initiated successfully"
    );

    Ok(Json(ApiResponse::success_with_id(
        response_data,
        request_id,
    )))
}

/// Get list of user's datasets
#[instrument(fields(request_id))]
pub async fn get_dataset_list_handler(
    Query(query): Query<DatasetListQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<DatasetInfo>>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        page = query.pagination.page,
        limit = query.pagination.limit,
        "Dataset list request received"
    );

    // TODO: Forward to core service to get actual data
    // This is a mock implementation
    let mock_datasets = create_mock_datasets();
    let filtered_datasets = filter_datasets(&mock_datasets, &query);
    let total = filtered_datasets.len() as u64;

    // Apply pagination
    let start = ((query.pagination.page - 1) * query.pagination.limit) as usize;
    let end = std::cmp::min(
        start + query.pagination.limit as usize,
        filtered_datasets.len(),
    );
    let page_datasets = filtered_datasets[start..end].to_vec();

    let response_data = PaginatedResponse::new(
        page_datasets,
        total,
        query.pagination.page,
        query.pagination.limit,
    );

    info!(
        request_id = request_id,
        total_datasets = total,
        returned_count = response_data.items.len(),
        "Dataset list retrieved successfully"
    );

    Ok(Json(ApiResponse::success_with_id(
        response_data,
        request_id,
    )))
}

/// Get specific dataset information
#[instrument(fields(request_id, dataset_id = %dataset_id))]
pub async fn get_dataset_handler(
    Path(dataset_id): Path<String>,
) -> Result<Json<ApiResponse<DatasetInfo>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = dataset_id,
        "Dataset info request received"
    );

    // Validate dataset ID format
    if !dataset_id.starts_with("ds_") {
        warn!(
            request_id = request_id,
            dataset_id = dataset_id,
            "Invalid dataset ID format"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Forward to core service to get actual data
    // This is a mock implementation
    let dataset_info = DatasetInfo {
        id: dataset_id.clone(),
        name: "Sample Genomic Dataset".to_string(),
        description: Some("A sample genomic dataset for testing".to_string()),
        dataset_type: DatasetType::Genomic,
        size_bytes: 1024 * 1024 * 10, // 10MB
        record_count: 1000,
        upload_timestamp: chrono::Utc::now().to_rfc3339(),
        status: DatasetStatus::Ready,
        owner_id: "user_123".to_string(),
        tags: vec!["genomic".to_string(), "test".to_string()],
        privacy_level: PrivacyLevel::Private,
    };

    info!(
        request_id = request_id,
        dataset_id = dataset_id,
        "Dataset info retrieved successfully"
    );

    Ok(Json(ApiResponse::success_with_id(dataset_info, request_id)))
}

/// Delete a dataset
pub async fn delete_dataset_handler(
    Path(dataset_id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let request_id = generate_request_id();
    tracing::Span::current().record("request_id", &request_id);

    info!(
        request_id = request_id,
        dataset_id = dataset_id,
        "Dataset deletion request received"
    );

    // Validate dataset ID format
    if !dataset_id.starts_with("ds_") {
        warn!(
            request_id = request_id,
            dataset_id = dataset_id,
            "Invalid dataset ID format"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Forward to core service for actual deletion
    // This would include:
    // 1. Check if dataset exists
    // 2. Verify user ownership
    // 3. Check if dataset is being used in active computations
    // 4. Delete dataset and associated metadata

    info!(
        request_id = request_id,
        dataset_id = dataset_id,
        "Dataset deleted successfully"
    );

    Ok(Json(ApiResponse::success_with_id((), request_id)))
}

/// Estimate processing time based on dataset characteristics
fn estimate_processing_time(request: &UploadDatasetRequest) -> u64 {
    let base_time = 30; // Base 30 seconds
    let data_size_factor = (request.data.len() / 1024 / 1024) as u64; // Per MB
    let type_factor = match request.dataset_type {
        DatasetType::Genomic => 5,
        DatasetType::Imaging => 10,
        DatasetType::Clinical => 2,
        _ => 3,
    };

    base_time + (data_size_factor * type_factor)
}

/// Create mock datasets for testing
fn create_mock_datasets() -> Vec<DatasetInfo> {
    vec![
        DatasetInfo {
            id: "ds_001".to_string(),
            name: "Genomic Dataset 1".to_string(),
            description: Some("Sample genomic data".to_string()),
            dataset_type: DatasetType::Genomic,
            size_bytes: 1024 * 1024 * 5,
            record_count: 500,
            upload_timestamp: chrono::Utc::now().to_rfc3339(),
            status: DatasetStatus::Ready,
            owner_id: "user_123".to_string(),
            tags: vec!["genomic".to_string(), "test".to_string()],
            privacy_level: PrivacyLevel::Private,
        },
        DatasetInfo {
            id: "ds_002".to_string(),
            name: "Clinical Dataset 1".to_string(),
            description: Some("Sample clinical data".to_string()),
            dataset_type: DatasetType::Clinical,
            size_bytes: 1024 * 1024 * 2,
            record_count: 200,
            upload_timestamp: chrono::Utc::now().to_rfc3339(),
            status: DatasetStatus::Processing,
            owner_id: "user_123".to_string(),
            tags: vec!["clinical".to_string(), "test".to_string()],
            privacy_level: PrivacyLevel::Restricted,
        },
    ]
}

/// Filter datasets based on query parameters
fn filter_datasets(datasets: &[DatasetInfo], query: &DatasetListQuery) -> Vec<DatasetInfo> {
    let mut filtered = datasets.to_vec();

    // Filter by dataset type
    if let Some(ref dataset_type) = query.dataset_type {
        filtered.retain(|d| {
            std::mem::discriminant(&d.dataset_type) == std::mem::discriminant(dataset_type)
        });
    }

    // Filter by status
    if let Some(ref status) = query.status {
        filtered.retain(|d| std::mem::discriminant(&d.status) == std::mem::discriminant(status));
    }

    // Filter by tags
    if let Some(ref tags_str) = query.tags {
        let search_tags: Vec<String> = tags_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();
        filtered.retain(|d| {
            search_tags.iter().any(|tag| {
                d.tags
                    .iter()
                    .any(|d_tag| d_tag.to_lowercase().contains(tag))
            })
        });
    }

    // Filter by search term
    if let Some(ref search) = query.search {
        let search_lower = search.to_lowercase();
        filtered.retain(|d| {
            d.name.to_lowercase().contains(&search_lower)
                || d.description
                    .as_ref()
                    .map_or(false, |desc| desc.to_lowercase().contains(&search_lower))
                || d.tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&search_lower))
        });
    }

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_processing_time() {
        let request = UploadDatasetRequest {
            name: "test".to_string(),
            description: None,
            dataset_type: DatasetType::Genomic,
            tags: None,
            privacy_level: None,
            data: "x".repeat(1024 * 1024), // 1MB of data
            expected_record_count: None,
        };

        let time = estimate_processing_time(&request);
        assert!(time > 30); // Should be more than base time
    }

    #[test]
    fn test_filter_datasets_by_type() {
        let datasets = create_mock_datasets();
        let query = DatasetListQuery {
            pagination: PaginationParams::default(),
            dataset_type: Some(DatasetType::Genomic),
            status: None,
            tags: None,
            search: None,
        };

        let filtered = filter_datasets(&datasets, &query);
        assert_eq!(filtered.len(), 1);
        assert!(matches!(filtered[0].dataset_type, DatasetType::Genomic));
    }

    #[test]
    fn test_filter_datasets_by_search() {
        let datasets = create_mock_datasets();
        let query = DatasetListQuery {
            pagination: PaginationParams::default(),
            dataset_type: None,
            status: None,
            tags: None,
            search: Some("genomic".to_string()),
        };

        let filtered = filter_datasets(&datasets, &query);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].name.to_lowercase().contains("genomic"));
    }
}
