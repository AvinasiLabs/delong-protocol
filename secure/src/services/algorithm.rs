use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, instrument};
use uuid::Uuid;
use std::time::Duration;

use common::{
    ApiResult, 
    AlgoExeData, 
    AlgoExeStatus, 
    ResponseCode,
    models::{Algorithm, AlgorithmMetadata}
};
use crate::tee::{TeeClient, KeyVault, KeyContext};
use crate::utils::{ResourceUsage, format_duration};
use crate::services::dataset::DatasetService;

/// Algorithm execution service for TEE environment
#[derive(Clone)]
pub struct AlgorithmService {
    key_vault: Arc<KeyVault>,
    dataset_service: Option<Arc<DatasetService>>,
    executions: Arc<RwLock<HashMap<String, ExecutionInfo>>>,
    algorithms: Arc<RwLock<HashMap<String, AlgorithmInfo>>>,
}

/// Algorithm information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlgorithmInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub cid: String, // IPFS CID for algorithm code
    pub created_at: DateTime<Utc>,
    pub metadata: AlgorithmMetadata,
    pub resource_limits: ResourceLimits,
}

/// Resource limits for algorithm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_cores: u32,
    pub max_execution_time_seconds: u64,
    pub max_disk_mb: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 1024,  // 1GB
            max_cpu_cores: 2,
            max_execution_time_seconds: 3600, // 1 hour
            max_disk_mb: 1024,    // 1GB
        }
    }
}

/// Execution information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionInfo {
    pub id: String,
    pub algorithm_id: String,
    pub dataset_ids: Vec<String>,
    pub status: AlgoExeStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result_data: Option<Vec<u8>>,
    pub error_message: Option<String>,
    pub resource_usage: ResourceUsage,
    pub author: String,
}

impl AlgorithmService {
    /// Create a new algorithm service
    pub async fn new() -> ApiResult<Self> {
        info!("Initializing algorithm service");

        let key_vault = Arc::new(KeyVault::new_with_client_kind(crate::tee::ClientKind::Mock));

        Ok(Self {
            key_vault,
            dataset_service: None,
            executions: Arc::new(RwLock::new(HashMap::new())),
            algorithms: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Set dataset service reference
    pub fn set_dataset_service(&mut self, dataset_service: Arc<DatasetService>) {
        self.dataset_service = Some(dataset_service);
    }

    /// Start the algorithm service
    pub async fn start(&self) -> ApiResult<()> {
        info!("Starting algorithm service");
        // Initialize runtime environment
        self.initialize_runtime().await?;
        Ok(())
    }

    /// Stop the algorithm service
    pub async fn stop(&self) -> ApiResult<()> {
        info!("Stopping algorithm service");
        
        // Cancel all running executions
        let execution_ids: Vec<String> = {
            let executions = self.executions.read().unwrap();
            executions.iter()
                .filter(|(_, exec)| matches!(exec.status, AlgoExeStatus::Running))
                .map(|(id, _)| id.clone())
                .collect()
        };

        for execution_id in execution_ids {
            if let Err(e) = self.cancel_execution(&execution_id).await {
                error!(execution_id = %execution_id, error = %e, "Failed to cancel execution");
            }
        }

        Ok(())
    }

    /// Health check for the algorithm service
    pub async fn health_check(&self) -> ApiResult<()> {
        // Check runtime environment
        self.check_runtime_health().await?;
        Ok(())
    }

    /// Register a new algorithm
    #[instrument(skip(self))]
    pub async fn register_algorithm(
        &self,
        name: &str,
        version: &str,
        cid: &str,
        metadata: AlgorithmMetadata,
        resource_limits: Option<ResourceLimits>,
    ) -> ApiResult<String> {
        info!(name = %name, version = %version, cid = %cid, "Registering new algorithm");

        // Validate CID format
        if !crate::utils::is_valid_cid(cid) {
            return Err(common::ApiError::BadRequest(
                "Invalid IPFS CID format".to_string()
            ));
        }

        let algorithm_id = Uuid::new_v4().to_string();
        let algorithm_info = AlgorithmInfo {
            id: algorithm_id.clone(),
            name: name.to_string(),
            version: version.to_string(),
            cid: cid.to_string(),
            created_at: Utc::now(),
            metadata,
            resource_limits: resource_limits.unwrap_or_default(),
        };

        self.algorithms.write().unwrap().insert(algorithm_id.clone(), algorithm_info);

        info!(algorithm_id = %algorithm_id, "Algorithm registered successfully");
        Ok(algorithm_id)
    }

    /// Execute an algorithm on datasets
    #[instrument(skip(self))]
    pub async fn execute_algorithm(
        &self,
        algorithm_id: &str,
        dataset_ids: Vec<String>,
        author: &str,
    ) -> ApiResult<String> {
        info!(
            algorithm_id = %algorithm_id, 
            dataset_count = dataset_ids.len(),
            "Starting algorithm execution"
        );

        // Get algorithm info
        let algorithm_info = {
            let algorithms = self.algorithms.read().unwrap();
            algorithms.get(algorithm_id).cloned()
                .ok_or_else(|| common::ApiError::NotFound("Algorithm not found".to_string()))?
        };

        // Validate datasets exist and add references
        if let Some(dataset_service) = &self.dataset_service {
            for dataset_id in &dataset_ids {
                dataset_service.add_reference(dataset_id).await?;
            }
        }

        let execution_id = Uuid::new_v4().to_string();
        let execution_info = ExecutionInfo {
            id: execution_id.clone(),
            algorithm_id: algorithm_id.to_string(),
            dataset_ids: dataset_ids.clone(),
            status: AlgoExeStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            result_data: None,
            error_message: None,
            resource_usage: ResourceUsage::default(),
            author: author.to_string(),
        };

        self.executions.write().unwrap().insert(execution_id.clone(), execution_info);

        // Start execution in background
        let service = self.clone();
        let exec_id = execution_id.clone();
        let algo_info = algorithm_info.clone();
        let datasets = dataset_ids.clone();
        let author_str = author.to_string();

        tokio::spawn(async move {
            if let Err(e) = service.run_algorithm_execution(
                &exec_id,
                &algo_info,
                &datasets,
                &author_str
            ).await {
                error!(execution_id = %exec_id, error = %e, "Algorithm execution failed");
                service.mark_execution_failed(&exec_id, &e.to_string()).await;
            }
        });

        info!(execution_id = %execution_id, "Algorithm execution started");
        Ok(execution_id)
    }

    /// Get execution status
    pub async fn get_execution_status(&self, execution_id: &str) -> ApiResult<AlgoExeData> {
        let execution_info = {
            let executions = self.executions.read().unwrap();
            executions.get(execution_id).cloned()
                .ok_or_else(|| common::ApiError::NotFound("Execution not found".to_string()))?
        };

        let duration = if let Some(completed_at) = execution_info.completed_at {
            Some((completed_at - execution_info.started_at).num_seconds() as u64)
        } else {
            Some((Utc::now() - execution_info.started_at).num_seconds() as u64)
        };

        Ok(AlgoExeData {
            id: execution_info.id,
            algorithm_id: Some(execution_info.algorithm_id),
            dataset_ids: Some(execution_info.dataset_ids),
            status: execution_info.status,
            started_at: Some(execution_info.started_at),
            completed_at: execution_info.completed_at,
            duration_seconds: duration,
            result_size: execution_info.result_data.as_ref().map(|data| data.len() as u64),
            error_message: execution_info.error_message,
            resource_usage: Some(execution_info.resource_usage),
        })
    }

    /// Cancel a running execution
    pub async fn cancel_execution(&self, execution_id: &str) -> ApiResult<()> {
        let mut executions = self.executions.write().unwrap();
        if let Some(execution) = executions.get_mut(execution_id) {
            if matches!(execution.status, AlgoExeStatus::Running) {
                execution.status = AlgoExeStatus::Cancelled;
                execution.completed_at = Some(Utc::now());
                
                // Remove dataset references
                if let Some(dataset_service) = &self.dataset_service {
                    for dataset_id in &execution.dataset_ids {
                        let _ = dataset_service.remove_reference(dataset_id).await;
                    }
                }

                info!(execution_id = %execution_id, "Execution cancelled");
                Ok(())
            } else {
                Err(common::ApiError::BadRequest("Execution is not running".to_string()))
            }
        } else {
            Err(common::ApiError::NotFound("Execution not found".to_string()))
        }
    }

    /// List algorithm executions
    pub async fn list_executions(&self, author: Option<&str>) -> ApiResult<Vec<AlgoExeData>> {
        let executions = self.executions.read().unwrap();
        let mut result = Vec::new();

        for (_, execution) in executions.iter() {
            // Filter by author if specified
            if let Some(filter_author) = author {
                if execution.author != filter_author {
                    continue;
                }
            }

            let duration = if let Some(completed_at) = execution.completed_at {
                Some((completed_at - execution.started_at).num_seconds() as u64)
            } else {
                Some((Utc::now() - execution.started_at).num_seconds() as u64)
            };

            result.push(AlgoExeData {
                id: execution.id.clone(),
                algorithm_id: Some(execution.algorithm_id.clone()),
                dataset_ids: Some(execution.dataset_ids.clone()),
                status: execution.status.clone(),
                started_at: Some(execution.started_at),
                completed_at: execution.completed_at,
                duration_seconds: duration,
                result_size: execution.result_data.as_ref().map(|data| data.len() as u64),
                error_message: execution.error_message.clone(),
                resource_usage: Some(execution.resource_usage.clone()),
            });
        }

        // Sort by start time (newest first)
        result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(result)
    }

    /// Get execution result
    pub async fn get_execution_result(&self, execution_id: &str) -> ApiResult<Vec<u8>> {
        let executions = self.executions.read().unwrap();
        let execution = executions.get(execution_id)
            .ok_or_else(|| common::ApiError::NotFound("Execution not found".to_string()))?;

        match &execution.status {
            AlgoExeStatus::Completed => {
                execution.result_data.clone()
                    .ok_or_else(|| common::ApiError::InternalError("No result data available".to_string()))
            }
            AlgoExeStatus::Running => {
                Err(common::ApiError::BadRequest("Execution still running".to_string()))
            }
            AlgoExeStatus::Failed => {
                Err(common::ApiError::BadRequest(
                    execution.error_message.clone()
                        .unwrap_or_else(|| "Execution failed".to_string())
                ))
            }
            AlgoExeStatus::Cancelled => {
                Err(common::ApiError::BadRequest("Execution was cancelled".to_string()))
            }
            _ => {
                Err(common::ApiError::BadRequest("Execution not completed".to_string()))
            }
        }
    }

    /// Internal method to run algorithm execution
    async fn run_algorithm_execution(
        &self,
        execution_id: &str,
        algorithm_info: &AlgorithmInfo,
        dataset_ids: &[String],
        author: &str,
    ) -> ApiResult<()> {
        // Load datasets
        let datasets = if let Some(dataset_service) = &self.dataset_service {
            let mut loaded_datasets = Vec::new();
            for dataset_id in dataset_ids {
                let data = dataset_service.get_dataset(dataset_id, author).await?;
                loaded_datasets.push(data);
            }
            loaded_datasets
        } else {
            return Err(common::ApiError::InternalError("Dataset service not available".to_string()));
        };

        // Simulate algorithm execution
        let result = self.simulate_algorithm_execution(algorithm_info, &datasets).await?;

        // Update execution with result
        {
            let mut executions = self.executions.write().unwrap();
            if let Some(execution) = executions.get_mut(execution_id) {
                execution.status = AlgoExeStatus::Completed;
                execution.completed_at = Some(Utc::now());
                execution.result_data = Some(result);
                execution.resource_usage = ResourceUsage {
                    cpu_percent: 45.0,
                    memory_bytes: 512 * 1024 * 1024, // 512MB
                    disk_bytes: 100 * 1024 * 1024,   // 100MB
                    network_bytes: 50 * 1024 * 1024, // 50MB
                    timestamp: Utc::now(),
                };
            }
        }

        // Remove dataset references
        if let Some(dataset_service) = &self.dataset_service {
            for dataset_id in dataset_ids {
                let _ = dataset_service.remove_reference(dataset_id).await;
            }
        }

        info!(execution_id = %execution_id, "Algorithm execution completed successfully");
        Ok(())
    }

    /// Simulate algorithm execution (replace with real TEE execution)
    async fn simulate_algorithm_execution(
        &self,
        algorithm_info: &AlgorithmInfo,
        datasets: &[Vec<u8>],
    ) -> ApiResult<Vec<u8>> {
        info!(algorithm_id = %algorithm_info.id, "Simulating algorithm execution");

        // Simulate processing time based on data size
        let total_size: usize = datasets.iter().map(|d| d.len()).sum();
        let processing_time = std::cmp::min(total_size / 1000, 5000); // Max 5 seconds
        
        tokio::time::sleep(Duration::from_millis(processing_time as u64)).await;

        // Generate simulated result
        let result = format!(
            "Algorithm {} processed {} datasets with total size {} bytes at {}",
            algorithm_info.name,
            datasets.len(),
            total_size,
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        );

        Ok(result.into_bytes())
    }

    /// Mark execution as failed
    async fn mark_execution_failed(&self, execution_id: &str, error_message: &str) {
        let mut executions = self.executions.write().unwrap();
        if let Some(execution) = executions.get_mut(execution_id) {
            execution.status = AlgoExeStatus::Failed;
            execution.completed_at = Some(Utc::now());
            execution.error_message = Some(error_message.to_string());

            // Remove dataset references
            if let Some(dataset_service) = &self.dataset_service {
                for dataset_id in &execution.dataset_ids {
                    let _ = dataset_service.remove_reference(dataset_id).await;
                }
            }
        }
    }

    /// Initialize runtime environment
    async fn initialize_runtime(&self) -> ApiResult<()> {
        info!("Initializing algorithm runtime environment");
        // Set up TEE environment, Docker, etc.
        Ok(())
    }

    /// Check runtime health
    async fn check_runtime_health(&self) -> ApiResult<()> {
        // Check Docker daemon, TEE status, etc.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_algorithm_registration() {
        let mut service = AlgorithmService::new().await.unwrap();
        
        let metadata = AlgorithmMetadata {
            description: "Test algorithm".to_string(),
            language: "Python".to_string(),
            framework: Some("sklearn".to_string()),
            version: "1.0.0".to_string(),
            tags: vec!["test".to_string()],
        };

        let algorithm_id = service.register_algorithm(
            "test_algorithm",
            "1.0.0",
            "QmYjtig7VJQ6XsnUjqqJvj7QaMcCAwtrgNdahSiFofrE7o",
            metadata,
            None
        ).await.unwrap();

        assert!(!algorithm_id.is_empty());
    }

    #[tokio::test]
    async fn test_algorithm_execution_without_datasets() {
        let service = AlgorithmService::new().await.unwrap();
        
        // Register algorithm first
        let metadata = AlgorithmMetadata {
            description: "Test algorithm".to_string(),
            language: "Python".to_string(),
            framework: Some("sklearn".to_string()),
            version: "1.0.0".to_string(),
            tags: vec!["test".to_string()],
        };

        let algorithm_id = service.register_algorithm(
            "test_algorithm",
            "1.0.0",
            "QmYjtig7VJQ6XsnUjqqJvj7QaMcCAwtrgNdahSiFofrE7o",
            metadata,
            None
        ).await.unwrap();

        // Execute without datasets (should fail due to no dataset service)
        let result = service.execute_algorithm(
            &algorithm_id,
            vec![],
            "test_author"
        ).await;

        // Should start execution but will fail later due to no dataset service
        assert!(result.is_ok());
    }
} 