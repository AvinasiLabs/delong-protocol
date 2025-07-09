use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, instrument};
use uuid::Uuid;

use common::{
    ApiResult, 
    AlgoExeData, 
    AlgoExeStatus, 
    ResponseCode,
    models::{Dataset, DatasetMetadata, DatasetVersion}
};
use crate::tee::{TeeClient, KeyVault, KeyContext, ClientKind};
use crate::tee::encryption::MockEncryption;
use crate::utils;



/// Dataset service for managing encrypted datasets in TEE
#[derive(Clone)]
pub struct DatasetService {
    key_vault: Arc<KeyVault>,
    encryption: Arc<MockEncryption>,
    datasets: Arc<RwLock<HashMap<String, DatasetInfo>>>,
    reference_counts: Arc<RwLock<HashMap<String, u32>>>,
}

/// Internal dataset information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatasetInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub size: u64,
    pub encrypted_cid: Option<String>, // IPFS CID for static datasets
    pub created_at: DateTime<Utc>,
    pub metadata: DatasetMetadata,
    pub is_static: bool,
}

impl DatasetService {
    /// Create a new dataset service
    pub async fn new() -> ApiResult<Self> {
        info!("Initializing dataset service");

        let key_vault = Arc::new(KeyVault::new_with_client_kind(crate::tee::ClientKind::Mock));
        let encryption = Arc::new(MockEncryption::new());

        Ok(Self {
            key_vault,
            encryption,
            datasets: Arc::new(RwLock::new(HashMap::new())),
            reference_counts: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start the dataset service
    pub async fn start(&self) -> ApiResult<()> {
        info!("Starting dataset service");
        // Initialize any background tasks here
        Ok(())
    }

    /// Stop the dataset service
    pub async fn stop(&self) -> ApiResult<()> {
        info!("Stopping dataset service");
        // Clean up any resources here
        Ok(())
    }

    /// Health check for the dataset service
    pub async fn health_check(&self) -> ApiResult<()> {
        // Verify key vault is working
        let test_context = KeyContext {
            dataset_id: "health_check".to_string(),
            author: "system".to_string(),
            purpose: "test".to_string(),
        };

        self.key_vault.get_symmetric_key(&test_context).await?;
        Ok(())
    }

    /// Store a new dataset (encrypted)
    #[instrument(skip(self, data))]
    pub async fn store_dataset(
        &self,
        name: &str,
        data: &[u8],
        metadata: DatasetMetadata,
        author: &str,
    ) -> ApiResult<String> {
        info!(name = %name, size = data.len(), "Storing new dataset");

        // Validate dataset name
        if !utils::is_valid_dataset_name(name) {
            return Err(common::ApiError::BadRequest(
                "Invalid dataset name format".to_string()
            ));
        }

        let dataset_id = Uuid::new_v4().to_string();
        let version = utils::generate_dataset_version();
        let is_static = utils::is_static_dataset(name);

        // Create encryption key context
        let key_context = KeyContext {
            dataset_id: dataset_id.clone(),
            author: author.to_string(),
            purpose: "storage".to_string(),
        };

        // Get encryption key from TEE
        let encryption_key = self.key_vault.get_symmetric_key(&key_context).await?;

        // Encrypt the dataset
        let encrypted_data = self.encryption.encrypt(data, &encryption_key).await?;

        // For static datasets, store in IPFS (simulated)
        let encrypted_cid = if is_static {
            Some(self.store_to_ipfs(&encrypted_data).await?)
        } else {
            // For dynamic datasets, store locally (simulated)
            self.store_locally(&dataset_id, &encrypted_data).await?;
            None
        };

        // Store dataset metadata
        let dataset_info = DatasetInfo {
            id: dataset_id.clone(),
            name: name.to_string(),
            version,
            size: data.len() as u64,
            encrypted_cid,
            created_at: Utc::now(),
            metadata,
            is_static,
        };

        self.datasets.write().unwrap().insert(dataset_id.clone(), dataset_info);
        self.reference_counts.write().unwrap().insert(dataset_id.clone(), 0);

        info!(dataset_id = %dataset_id, "Dataset stored successfully");
        Ok(dataset_id)
    }

    /// Retrieve and decrypt a dataset
    #[instrument(skip(self))]
    pub async fn get_dataset(
        &self,
        dataset_id: &str,
        author: &str,
    ) -> ApiResult<Vec<u8>> {
        info!(dataset_id = %dataset_id, "Retrieving dataset");

        // Get dataset info
        let dataset_info = {
            let datasets = self.datasets.read().unwrap();
            datasets.get(dataset_id).cloned()
                .ok_or_else(|| common::ApiError::NotFound("Dataset not found".to_string()))?
        };

        // Create decryption key context
        let key_context = KeyContext {
            dataset_id: dataset_id.to_string(),
            author: author.to_string(),
            purpose: "storage".to_string(),
        };

        // Get decryption key from TEE
        let decryption_key = self.key_vault.get_symmetric_key(&key_context).await?;

        // Retrieve encrypted data
        let encrypted_data = if dataset_info.is_static {
            if let Some(cid) = &dataset_info.encrypted_cid {
                self.retrieve_from_ipfs(cid).await?
            } else {
                return Err(common::ApiError::InternalError(
                    "Static dataset missing IPFS CID".to_string()
                ));
            }
        } else {
            self.retrieve_locally(dataset_id).await?
        };

        // Decrypt the dataset
        let decrypted_data = self.encryption.decrypt(&encrypted_data, &decryption_key).await?;

        info!(dataset_id = %dataset_id, size = decrypted_data.len(), "Dataset retrieved successfully");
        Ok(decrypted_data)
    }

    /// Increment reference count for a dataset
    pub async fn add_reference(&self, dataset_id: &str) -> ApiResult<u32> {
        let mut ref_counts = self.reference_counts.write().unwrap();
        if let Some(count) = ref_counts.get_mut(dataset_id) {
            *count += 1;
            info!(dataset_id = %dataset_id, ref_count = *count, "Reference added");
            Ok(*count)
        } else {
            Err(common::ApiError::NotFound("Dataset not found".to_string()))
        }
    }

    /// Decrement reference count for a dataset
    pub async fn remove_reference(&self, dataset_id: &str) -> ApiResult<u32> {
        let mut ref_counts = self.reference_counts.write().unwrap();
        if let Some(count) = ref_counts.get_mut(dataset_id) {
            if *count > 0 {
                *count -= 1;
            }
            info!(dataset_id = %dataset_id, ref_count = *count, "Reference removed");
            Ok(*count)
        } else {
            Err(common::ApiError::NotFound("Dataset not found".to_string()))
        }
    }

    /// Get reference count for a dataset
    pub async fn get_reference_count(&self, dataset_id: &str) -> ApiResult<u32> {
        let ref_counts = self.reference_counts.read().unwrap();
        ref_counts.get(dataset_id)
            .copied()
            .ok_or_else(|| common::ApiError::NotFound("Dataset not found".to_string()))
    }

    /// List all available datasets
    pub async fn list_datasets(&self) -> ApiResult<Vec<Dataset>> {
        let datasets = self.datasets.read().unwrap();
        let mut result = Vec::new();

        for (id, info) in datasets.iter() {
            let ref_count = self.reference_counts.read().unwrap()
                .get(id).copied().unwrap_or(0);

            result.push(Dataset {
                id: id.clone(),
                name: info.name.clone(),
                version: Some(info.version.clone()),
                size: Some(info.size),
                created_at: Some(info.created_at),
                metadata: Some(info.metadata.clone()),
                reference_count: Some(ref_count),
            });
        }

        Ok(result)
    }

    /// Get dataset metadata
    pub async fn get_dataset_metadata(&self, dataset_id: &str) -> ApiResult<DatasetMetadata> {
        let datasets = self.datasets.read().unwrap();
        let dataset_info = datasets.get(dataset_id)
            .ok_or_else(|| common::ApiError::NotFound("Dataset not found".to_string()))?;

        Ok(dataset_info.metadata.clone())
    }

    /// Delete a dataset (only if reference count is 0)
    pub async fn delete_dataset(&self, dataset_id: &str) -> ApiResult<()> {
        let ref_count = self.get_reference_count(dataset_id).await?;
        if ref_count > 0 {
            return Err(common::ApiError::BadRequest(
                format!("Cannot delete dataset with {} active references", ref_count)
            ));
        }

        // Remove from storage
        let dataset_info = {
            let mut datasets = self.datasets.write().unwrap();
            datasets.remove(dataset_id)
                .ok_or_else(|| common::ApiError::NotFound("Dataset not found".to_string()))?
        };

        // Clean up storage
        if dataset_info.is_static {
            if let Some(cid) = &dataset_info.encrypted_cid {
                self.remove_from_ipfs(cid).await?;
            }
        } else {
            self.remove_locally(dataset_id).await?;
        }

        // Remove reference count
        self.reference_counts.write().unwrap().remove(dataset_id);

        info!(dataset_id = %dataset_id, "Dataset deleted successfully");
        Ok(())
    }

    /// Simulate IPFS storage
    async fn store_to_ipfs(&self, data: &[u8]) -> ApiResult<String> {
        // Simulate IPFS CID generation
        let cid = format!("Qm{}", utils::sha256_hash(data)[..44].to_uppercase());
        info!(cid = %cid, size = data.len(), "Stored to IPFS (simulated)");
        Ok(cid)
    }

    /// Simulate IPFS retrieval
    async fn retrieve_from_ipfs(&self, _cid: &str) -> ApiResult<Vec<u8>> {
        // Simulate IPFS data retrieval
        info!(cid = %_cid, "Retrieved from IPFS (simulated)");
        Ok(b"simulated_encrypted_data".to_vec())
    }

    /// Simulate IPFS removal
    async fn remove_from_ipfs(&self, _cid: &str) -> ApiResult<()> {
        info!(cid = %_cid, "Removed from IPFS (simulated)");
        Ok(())
    }

    /// Simulate local storage
    async fn store_locally(&self, dataset_id: &str, _data: &[u8]) -> ApiResult<()> {
        info!(dataset_id = %dataset_id, "Stored locally (simulated)");
        Ok(())
    }

    /// Simulate local retrieval
    async fn retrieve_locally(&self, dataset_id: &str) -> ApiResult<Vec<u8>> {
        info!(dataset_id = %dataset_id, "Retrieved locally (simulated)");
        Ok(b"simulated_local_encrypted_data".to_vec())
    }

    /// Simulate local removal
    async fn remove_locally(&self, dataset_id: &str) -> ApiResult<()> {
        info!(dataset_id = %dataset_id, "Removed locally (simulated)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dataset_lifecycle() {
        let service = DatasetService::new().await.unwrap();
        
        let metadata = DatasetMetadata {
            description: "Test dataset".to_string(),
            format: "CSV".to_string(),
            schema: Some("name,age,score".to_string()),
            tags: vec!["test".to_string()],
        };

        // Store dataset
        let dataset_id = service.store_dataset(
            "test_dataset",
            b"test,data,content",
            metadata.clone(),
            "test_author"
        ).await.unwrap();

        // Retrieve dataset
        let data = service.get_dataset(&dataset_id, "test_author").await.unwrap();
        assert!(!data.is_empty());

        // Add and remove references
        let ref_count = service.add_reference(&dataset_id).await.unwrap();
        assert_eq!(ref_count, 1);

        let ref_count = service.remove_reference(&dataset_id).await.unwrap();
        assert_eq!(ref_count, 0);

        // Delete dataset
        service.delete_dataset(&dataset_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_static_dataset() {
        let service = DatasetService::new().await.unwrap();
        
        let metadata = DatasetMetadata {
            description: "Static test dataset".to_string(),
            format: "JSON".to_string(),
            schema: None,
            tags: vec!["static".to_string()],
        };

        // Store static dataset
        let dataset_id = service.store_dataset(
            "__static__blood_test",
            b"static,test,data",
            metadata,
            "static_author"
        ).await.unwrap();

        // Verify it's treated as static
        let datasets = service.list_datasets().await.unwrap();
        let dataset = datasets.iter().find(|d| d.id == dataset_id).unwrap();
        assert!(dataset.name.starts_with("__static__"));
    }
} 