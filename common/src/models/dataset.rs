//! Dataset management data models
//!
//! This module contains all data structures related to dataset management,
//! including static datasets (IPFS + blockchain), dynamic datasets (local filesystem),
//! and dataset operations.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Static dataset information as returned by the API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[schema(example = json!({
    "id": 1,
    "name": "sample_dataset",
    "ui_name": "Sample Dataset",
    "desc": "A sample dataset for testing",
    "file_hash": "abc123def456",
    "ipfs_cid": "QmSampleCID123",
    "file_size": 1024,
    "file_format": "csv",
    "author": "John Doe",
    "author_wallet": "0x1234567890abcdef",
    "sample_url": "https://api.example.com/sample/QmSampleCID123",
    "file_path": "/data/sample_dataset.csv",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
}))]
pub struct StaticDatasetInfo {
    /// Unique identifier for the dataset
    #[schema(example = 1)]
    pub id: u32,
    /// Internal name of the dataset
    #[schema(example = "sample_dataset")]
    pub name: String,
    /// User-friendly display name
    #[schema(example = "Sample Dataset")]
    pub ui_name: String,
    /// Optional description of the dataset
    #[schema(example = "A sample dataset for testing")]
    pub desc: Option<String>,
    /// SHA-256 hash of the file content
    #[schema(example = "abc123def456")]
    pub file_hash: String,
    /// IPFS Content Identifier
    #[schema(example = "QmSampleCID123")]
    pub ipfs_cid: String,
    /// Size of the file in bytes
    #[schema(example = 1024)]
    pub file_size: u64,
    /// Format of the file (e.g., "csv", "json", "parquet")
    #[schema(example = "csv")]
    pub file_format: String,
    /// Optional author name
    #[schema(example = "John Doe")]
    pub author: Option<String>,
    /// Optional author wallet address
    #[schema(example = "0x1234567890abcdef")]
    pub author_wallet: Option<String>,
    /// URL to access sample data
    #[schema(example = "https://api.example.com/sample/QmSampleCID123")]
    pub sample_url: String,
    /// File path in the system
    #[schema(example = "/data/sample_dataset.csv")]
    pub file_path: String,
    /// Creation timestamp in RFC3339 format
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: String,
    /// Last update timestamp in RFC3339 format
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub updated_at: String,
}

/// Dynamic dataset information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[schema(example = json!({
    "id": 1,
    "name": "dynamic_dataset",
    "description": "A dynamic dataset for testing",
    "file_path": "/data/dynamic_dataset.csv",
    "file_size": 2048,
    "file_format": "csv",
    "version": 1,
    "status": "active",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
}))]
pub struct DynamicDatasetInfo {
    /// Unique identifier for the dataset
    #[schema(example = 1)]
    pub id: u32,
    /// Internal name of the dataset
    #[schema(example = "dynamic_dataset")]
    pub name: String,
    /// Optional description of the dataset
    #[schema(example = "A dynamic dataset for testing")]
    pub description: Option<String>,
    /// File path in the system
    pub file_path: String,
    /// Creation timestamp in RFC3339 format
    pub created_at: String,
    /// Last update timestamp in RFC3339 format
    pub updated_at: String,
}

/// Request body for creating a new dynamic dataset
#[derive(Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct CreateDatasetRequest {
    /// Internal name of the dataset
    pub name: String,
    /// User-friendly display name
    pub ui_name: String,
    /// Optional description of the dataset
    pub description: Option<String>,
}

/// Request body for updating dynamic dataset
#[derive(Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UpdateDatasetRequest {
    /// Updated description of the dataset
    pub description: Option<String>,
}

/// Request body for updating static dataset
#[derive(Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct UpdateStaticDatasetRequest {
    /// Updated name of the dataset
    pub name: Option<String>,
    /// Updated description of the dataset
    pub desc: Option<String>,
}

/// Query parameters for static dataset listing
#[derive(Debug, Deserialize, PartialEq, ToSchema)]
pub struct StaticDatasetListQuery {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Number of items per page
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

/// Query parameters for dynamic dataset listing
#[derive(Debug, Deserialize, PartialEq, ToSchema)]
pub struct DynamicDatasetListQuery {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Number of items per page
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

/// Paginated response wrapper for datasets
#[derive(Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct DatasetPaginatedResponse<T> {
    /// List of items in the current page
    pub items: Vec<T>,
    /// Total number of items across all pages
    pub total: u64,
    /// Current page number (1-based)
    pub page: u32,
    /// Number of items per page
    pub page_size: u32,
}

/// Dataset file format enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum DatasetFormat {
    /// Comma-separated values
    #[serde(rename = "csv")]
    Csv,
    /// JavaScript Object Notation
    #[serde(rename = "json")]
    Json,
    /// Apache Parquet columnar format
    #[serde(rename = "parquet")]
    Parquet,
    /// Tab-separated values
    #[serde(rename = "tsv")]
    Tsv,
    /// Excel spreadsheet
    #[serde(rename = "xlsx")]
    Excel,
    /// Other format
    #[serde(rename = "other")]
    Other(String),
}

/// Dataset status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum DatasetStatus {
    /// Dataset is being uploaded
    #[serde(rename = "uploading")]
    Uploading,
    /// Dataset is being processed
    #[serde(rename = "processing")]
    Processing,
    /// Dataset is ready for use
    #[serde(rename = "ready")]
    Ready,
    /// Dataset has an error
    #[serde(rename = "error")]
    Error,
    /// Dataset is archived
    #[serde(rename = "archived")]
    Archived,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    10
}

impl StaticDatasetInfo {
    /// Create a new static dataset info instance
    pub fn new(
        id: u32,
        name: String,
        ui_name: String,
        file_hash: String,
        ipfs_cid: String,
        file_size: u64,
        file_format: String,
        sample_url: String,
        file_path: String,
        created_at: String,
        updated_at: String,
    ) -> Self {
        Self {
            id,
            name,
            ui_name,
            desc: None,
            file_hash,
            ipfs_cid,
            file_size,
            file_format,
            author: None,
            author_wallet: None,
            sample_url,
            file_path,
            created_at,
            updated_at,
        }
    }

    /// Get the dataset format as enum
    pub fn format(&self) -> DatasetFormat {
        match self.file_format.to_lowercase().as_str() {
            "csv" => DatasetFormat::Csv,
            "json" => DatasetFormat::Json,
            "parquet" => DatasetFormat::Parquet,
            "tsv" => DatasetFormat::Tsv,
            "xlsx" => DatasetFormat::Excel,
            other => DatasetFormat::Other(other.to_string()),
        }
    }

    /// Check if the dataset has author information
    pub fn has_author(&self) -> bool {
        self.author.is_some() || self.author_wallet.is_some()
    }

    /// Get a human-readable file size
    pub fn formatted_file_size(&self) -> String {
        let size = self.file_size as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.1} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl DynamicDatasetInfo {
    /// Create a new dynamic dataset info instance
    pub fn new(
        id: u32,
        name: String,
        file_path: String,
        created_at: String,
        updated_at: String,
    ) -> Self {
        Self {
            id,
            name,
            description: None,
            file_path,
            created_at,
            updated_at,
        }
    }

    /// Create a new dynamic dataset info instance with description
    pub fn with_description(
        id: u32,
        name: String,
        description: Option<String>,
        file_path: String,
        created_at: String,
        updated_at: String,
    ) -> Self {
        Self {
            id,
            name,
            description,
            file_path,
            created_at,
            updated_at,
        }
    }
}

impl CreateDatasetRequest {
    /// Create a new dataset creation request
    pub fn new(name: String, ui_name: String) -> Self {
        Self {
            name,
            ui_name,
            description: None,
        }
    }

    /// Create a new dataset creation request with description
    pub fn with_description(name: String, ui_name: String, description: String) -> Self {
        Self {
            name,
            ui_name,
            description: Some(description),
        }
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Dataset name cannot be empty".to_string());
        }

        if self.ui_name.trim().is_empty() {
            return Err("Dataset UI name cannot be empty".to_string());
        }

        if self.name.len() > 100 {
            return Err("Dataset name cannot exceed 100 characters".to_string());
        }

        if self.ui_name.len() > 200 {
            return Err("Dataset UI name cannot exceed 200 characters".to_string());
        }

        if let Some(desc) = &self.description {
            if desc.len() > 1000 {
                return Err("Dataset description cannot exceed 1000 characters".to_string());
            }
        }

        // Validate name format (alphanumeric, underscore, hyphen only)
        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "Dataset name can only contain alphanumeric characters, underscores, and hyphens"
                    .to_string(),
            );
        }

        Ok(())
    }
}

impl UpdateDatasetRequest {
    /// Create a new dataset update request
    pub fn new() -> Self {
        Self { description: None }
    }

    /// Create a new dataset update request with description
    pub fn with_description(description: String) -> Self {
        Self {
            description: Some(description),
        }
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if let Some(desc) = &self.description {
            if desc.len() > 1000 {
                return Err("Dataset description cannot exceed 1000 characters".to_string());
            }
        }
        Ok(())
    }
}

impl UpdateStaticDatasetRequest {
    /// Create a new static dataset update request
    pub fn new() -> Self {
        Self {
            name: None,
            desc: None,
        }
    }

    /// Set the name to update
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the description to update
    pub fn with_description(mut self, desc: String) -> Self {
        self.desc = Some(desc);
        self
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err("Dataset name cannot be empty".to_string());
            }
            if name.len() > 200 {
                return Err("Dataset name cannot exceed 200 characters".to_string());
            }
        }

        if let Some(desc) = &self.desc {
            if desc.len() > 1000 {
                return Err("Dataset description cannot exceed 1000 characters".to_string());
            }
        }

        Ok(())
    }

    /// Check if the request has any updates
    pub fn has_updates(&self) -> bool {
        self.name.is_some() || self.desc.is_some()
    }
}

impl<T> DatasetPaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(items: Vec<T>, page: u32, page_size: u32, total: u64) -> Self {
        Self {
            items,
            total,
            page,
            page_size,
        }
    }

    /// Get the total number of pages
    pub fn total_pages(&self) -> u32 {
        if self.total == 0 {
            1
        } else {
            ((self.total - 1) / self.page_size as u64 + 1) as u32
        }
    }

    /// Check if there's a next page
    pub fn has_next_page(&self) -> bool {
        self.page < self.total_pages()
    }

    /// Check if there's a previous page
    pub fn has_previous_page(&self) -> bool {
        self.page > 1
    }
}

impl Default for StaticDatasetListQuery {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
        }
    }
}

impl Default for DynamicDatasetListQuery {
    fn default() -> Self {
        Self {
            page: default_page(),
            page_size: default_page_size(),
        }
    }
}

impl Default for UpdateDatasetRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for UpdateStaticDatasetRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl DatasetFormat {
    /// Get the file extension for this format
    pub fn extension(&self) -> &str {
        match self {
            DatasetFormat::Csv => "csv",
            DatasetFormat::Json => "json",
            DatasetFormat::Parquet => "parquet",
            DatasetFormat::Tsv => "tsv",
            DatasetFormat::Excel => "xlsx",
            DatasetFormat::Other(ext) => ext,
        }
    }

    /// Check if the format is supported for sample generation
    pub fn supports_sample_generation(&self) -> bool {
        matches!(
            self,
            DatasetFormat::Csv | DatasetFormat::Json | DatasetFormat::Tsv
        )
    }
}

impl DatasetStatus {
    /// Check if the dataset is ready for use
    pub fn is_ready(&self) -> bool {
        matches!(self, DatasetStatus::Ready)
    }

    /// Check if the dataset has an error
    pub fn is_error(&self) -> bool {
        matches!(self, DatasetStatus::Error)
    }

    /// Check if the dataset is being processed
    pub fn is_processing(&self) -> bool {
        matches!(self, DatasetStatus::Processing | DatasetStatus::Uploading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_dataset_info_creation() {
        let dataset = StaticDatasetInfo::new(
            1,
            "test_dataset".to_string(),
            "Test Dataset".to_string(),
            "abcdef123456".to_string(),
            "QmTest123".to_string(),
            1024,
            "csv".to_string(),
            "http://example.com/sample".to_string(),
            "/path/to/file".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(dataset.id, 1);
        assert_eq!(dataset.name, "test_dataset");
        assert_eq!(dataset.ui_name, "Test Dataset");
        assert_eq!(dataset.file_size, 1024);
        assert_eq!(dataset.format(), DatasetFormat::Csv);
        assert!(!dataset.has_author());
    }

    #[test]
    fn test_static_dataset_info_formatted_file_size() {
        let dataset = StaticDatasetInfo::new(
            1,
            "test".to_string(),
            "Test".to_string(),
            "hash".to_string(),
            "cid".to_string(),
            1536, // 1.5 KB
            "csv".to_string(),
            "url".to_string(),
            "path".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(dataset.formatted_file_size(), "1.5 KB");
    }

    #[test]
    fn test_dynamic_dataset_info_creation() {
        let dataset = DynamicDatasetInfo::new(
            1,
            "test_dataset".to_string(),
            "/path/to/file".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        assert_eq!(dataset.id, 1);
        assert_eq!(dataset.name, "test_dataset");
        assert!(dataset.description.is_none());
    }

    #[test]
    fn test_create_dataset_request_validation() {
        // Valid request
        let valid_request =
            CreateDatasetRequest::new("valid_dataset".to_string(), "Valid Dataset".to_string());
        assert!(valid_request.validate().is_ok());

        // Empty name
        let empty_name = CreateDatasetRequest::new("".to_string(), "UI Name".to_string());
        assert!(empty_name.validate().is_err());

        // Invalid characters in name
        let invalid_name =
            CreateDatasetRequest::new("invalid dataset!".to_string(), "UI Name".to_string());
        assert!(invalid_name.validate().is_err());

        // Name too long
        let long_name = CreateDatasetRequest::new("a".repeat(101), "UI Name".to_string());
        assert!(long_name.validate().is_err());
    }

    #[test]
    fn test_update_dataset_request_validation() {
        // Valid request
        let valid_request = UpdateDatasetRequest::with_description("Valid description".to_string());
        assert!(valid_request.validate().is_ok());

        // Description too long
        let long_desc = UpdateDatasetRequest::with_description("a".repeat(1001));
        assert!(long_desc.validate().is_err());
    }

    #[test]
    fn test_update_static_dataset_request() {
        let request = UpdateStaticDatasetRequest::new()
            .with_name("New Name".to_string())
            .with_description("New Description".to_string());

        assert_eq!(request.name, Some("New Name".to_string()));
        assert_eq!(request.desc, Some("New Description".to_string()));
        assert!(request.has_updates());
    }

    #[test]
    fn test_dataset_paginated_response() {
        let items = vec!["item1", "item2", "item3"];
        let response = DatasetPaginatedResponse::new(items, 1, 10, 25);

        assert_eq!(response.items.len(), 3);
        assert_eq!(response.total, 25);
        assert_eq!(response.total_pages(), 3);
        assert!(response.has_next_page());
        assert!(!response.has_previous_page());
    }

    #[test]
    fn test_dataset_format() {
        assert_eq!(DatasetFormat::Csv.extension(), "csv");
        assert_eq!(DatasetFormat::Json.extension(), "json");
        assert_eq!(DatasetFormat::Parquet.extension(), "parquet");

        assert!(DatasetFormat::Csv.supports_sample_generation());
        assert!(!DatasetFormat::Parquet.supports_sample_generation());
    }

    #[test]
    fn test_dataset_status() {
        assert!(DatasetStatus::Ready.is_ready());
        assert!(!DatasetStatus::Error.is_ready());

        assert!(DatasetStatus::Error.is_error());
        assert!(!DatasetStatus::Ready.is_error());

        assert!(DatasetStatus::Processing.is_processing());
        assert!(DatasetStatus::Uploading.is_processing());
        assert!(!DatasetStatus::Ready.is_processing());
    }

    #[test]
    fn test_serialization_deserialization() {
        let dataset = StaticDatasetInfo::new(
            1,
            "test_dataset".to_string(),
            "Test Dataset".to_string(),
            "abcdef123456".to_string(),
            "QmTest123".to_string(),
            1024,
            "csv".to_string(),
            "http://example.com/sample".to_string(),
            "/path/to/file".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
            "2023-01-01T00:00:00Z".to_string(),
        );

        let json = serde_json::to_string(&dataset).unwrap();
        let deserialized: StaticDatasetInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(dataset, deserialized);
    }
}
