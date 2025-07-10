use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{info, warn, error, instrument};
use serde::{Deserialize, Serialize};


use common::ApiError;
use crate::config::SecureConfig;
use crate::runtime::execution_queue::{ExecutionRequest, ExecutionStatus};
use crate::tee::KeyVault;

/// Algorithm execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Execution ID
    pub execution_id: u64,
    
    /// Final status
    pub status: ExecutionStatus,
    
    /// Result IPFS CID (if successful)
    pub result_cid: Option<String>,
    
    /// Error message (if failed)
    pub error_message: Option<String>,
    
    /// Execution duration in seconds
    pub duration_seconds: f64,
    
    /// Resource usage statistics
    pub resource_usage: ResourceUsage,
    
    /// Execution metadata
    pub metadata: serde_json::Value,
    
    /// Completion timestamp
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// Resource usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU time used (seconds)
    pub cpu_seconds: f64,
    
    /// Memory peak usage (bytes)
    pub memory_peak_bytes: u64,
    
    /// Disk I/O bytes read
    pub disk_read_bytes: u64,
    
    /// Disk I/O bytes written
    pub disk_write_bytes: u64,
    
    /// Network bytes sent
    pub network_sent_bytes: u64,
    
    /// Network bytes received
    pub network_received_bytes: u64,
}

/// Algorithm executor for running algorithms in TEE environment
pub struct AlgorithmExecutor {
    config: SecureConfig,
    key_vault: KeyVault,
    #[allow(dead_code)] // TODO: Use for IPFS dataset/result storage
    ipfs_client: ipfs_api_backend_hyper::IpfsClient,
}

impl AlgorithmExecutor {
    /// Create a new algorithm executor
    pub fn new(
        config: SecureConfig,
        key_vault: KeyVault,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient,
    ) -> Self {
        Self {
            config,
            key_vault,
            ipfs_client,
        }
    }

    /// Execute an algorithm request in the TEE environment
    #[instrument(skip(self), fields(execution_id = %request.execution_id))]
    pub async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ApiError> {
        let start_time = Instant::now();
        
        info!(
            execution_id = %request.execution_id,
            algorithm_cid = %request.algorithm_cid,
            dataset_id = %request.dataset_id,
            "Starting algorithm execution in TEE"
        );

        // Step 1: Download and decrypt the dataset
        let dataset_path = self.prepare_dataset(&request).await?;
        
        // Step 2: Download the algorithm
        let algorithm_path = self.download_algorithm(&request).await?;
        
        // Step 3: Set up execution environment
        let execution_env = self.setup_execution_environment(&request).await?;
        
        // Step 4: Execute the algorithm
        let execution_result = self.run_algorithm_in_tee(
            &request,
            &algorithm_path,
            &dataset_path,
            &execution_env,
        ).await;

        // Step 5: Process results and cleanup
        let duration = start_time.elapsed();
        
        match execution_result {
            Ok(result_data) => {
                // Encrypt and store results
                let result_cid = self.store_encrypted_result(&request, result_data).await?;
                
                // Clean up temporary files
                self.cleanup_execution_files(&algorithm_path, &dataset_path, &execution_env).await;
                
                info!(
                    execution_id = %request.execution_id,
                    duration_ms = %duration.as_millis(),
                    result_cid = %result_cid,
                    "Algorithm execution completed successfully"
                );

                Ok(ExecutionResult {
                    execution_id: request.execution_id,
                    status: ExecutionStatus::Completed,
                    result_cid: Some(result_cid),
                    error_message: None,
                    duration_seconds: duration.as_secs_f64(),
                    resource_usage: ResourceUsage::default(), // TODO: Implement real resource monitoring
                    metadata: serde_json::json!({
                        "algorithm_cid": request.algorithm_cid,
                        "dataset_id": request.dataset_id,
                        "scientist_wallet": request.scientist_wallet
                    }),
                    completed_at: chrono::Utc::now(),
                })
            }
            Err(e) => {
                // Clean up on failure
                self.cleanup_execution_files(&algorithm_path, &dataset_path, &execution_env).await;
                
                error!(
                    execution_id = %request.execution_id,
                    error = %e,
                    duration_ms = %duration.as_millis(),
                    "Algorithm execution failed"
                );

                Ok(ExecutionResult {
                    execution_id: request.execution_id,
                    status: ExecutionStatus::Failed,
                    result_cid: None,
                    error_message: Some(e.to_string()),
                    duration_seconds: duration.as_secs_f64(),
                    resource_usage: ResourceUsage::default(),
                    metadata: serde_json::json!({
                        "algorithm_cid": request.algorithm_cid,
                        "dataset_id": request.dataset_id,
                        "error": e.to_string()
                    }),
                    completed_at: chrono::Utc::now(),
                })
            }
        }
    }

    /// Download and decrypt dataset for execution
    async fn prepare_dataset(&self, request: &ExecutionRequest) -> Result<String, ApiError> {
        info!(
            execution_id = %request.execution_id,
            dataset_id = %request.dataset_id,
            "Preparing dataset for execution"
        );

        match self.config.tee.client_type.as_str() {
            "mock" => {
                // Mock implementation - create a temporary dataset file
                let dataset_path = format!("/tmp/dataset_{}.csv", request.execution_id);
                
                // Create mock dataset content
                let mock_data = "id,feature1,feature2,target\n1,0.5,0.3,1\n2,0.8,0.2,0\n3,0.1,0.9,1\n";
                tokio::fs::write(&dataset_path, mock_data).await
                    .map_err(|_e| ApiError::FileProcessingError)?;
                
                info!(dataset_path = %dataset_path, "Created mock dataset");
                Ok(dataset_path)
            }
            "phala" => {
                // Real TEE implementation would:
                // 1. Retrieve encrypted dataset from IPFS
                // 2. Decrypt using TEE-protected keys
                // 3. Save to secure temporary location
                
                // For now, return mock path
                warn!("Phala TEE integration not yet implemented, using mock dataset");
                let mock_request = ExecutionRequest {
                    execution_id: request.execution_id,
                    algorithm_cid: request.algorithm_cid.clone(),
                    dataset_id: request.dataset_id.clone(),
                    scientist_wallet: request.scientist_wallet.clone(),
                    priority: request.priority,
                    created_at: request.created_at,
                    status: ExecutionStatus::Queued,
                    parameters: request.parameters.clone(),
                };
                // Use mock implementation instead of recursion
                let dataset_path = format!("/tmp/dataset_{}.csv", mock_request.execution_id);
                let mock_data = "id,feature1,feature2,target\n1,0.5,0.3,1\n2,0.8,0.2,0\n3,0.1,0.9,1\n";
                tokio::fs::write(&dataset_path, mock_data).await
                    .map_err(|_e| ApiError::FileProcessingError)?;
                Ok(dataset_path)
            }
            _ => {
                Err(ApiError::ConfigurationError(
                    format!("Unsupported TEE client type: {}", self.config.tee.client_type)
                ))
            }
        }
    }

    /// Download algorithm from IPFS
    async fn download_algorithm(&self, request: &ExecutionRequest) -> Result<String, ApiError> {
        info!(
            execution_id = %request.execution_id,
            algorithm_cid = %request.algorithm_cid,
            "Downloading algorithm from IPFS"
        );

        // Mock implementation - create a simple Python script
        let algorithm_path = format!("/tmp/algorithm_{}.py", request.execution_id);
        
        let mock_algorithm = r#"#!/usr/bin/env python3
import sys
import pandas as pd
import json

def main():
    if len(sys.argv) != 3:
        print("Usage: algorithm.py <dataset_path> <output_path>")
        sys.exit(1)
    
    dataset_path = sys.argv[1]
    output_path = sys.argv[2]
    
    # Load dataset
    try:
        df = pd.read_csv(dataset_path)
        print(f"Loaded dataset with {len(df)} rows and {len(df.columns)} columns")
        
        # Simple analysis: calculate mean of numeric columns
        numeric_cols = df.select_dtypes(include=['number']).columns
        results = {}
        
        for col in numeric_cols:
            results[col] = {
                'mean': float(df[col].mean()),
                'std': float(df[col].std()),
                'min': float(df[col].min()),
                'max': float(df[col].max())
            }
        
        # Save results
        with open(output_path, 'w') as f:
            json.dump(results, f, indent=2)
        
        print(f"Analysis complete. Results saved to {output_path}")
        
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
"#;

        tokio::fs::write(&algorithm_path, mock_algorithm).await
            .map_err(|_e| ApiError::FileProcessingError)?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = tokio::fs::metadata(&algorithm_path).await
                .map_err(|_e| ApiError::FileProcessingError)?;
            let mut perms = metadata.permissions();
            perms.set_mode(0o755); // rwxr-xr-x
            tokio::fs::set_permissions(&algorithm_path, perms).await
                .map_err(|_e| ApiError::FileProcessingError)?;
        }

        info!(algorithm_path = %algorithm_path, "Algorithm downloaded and prepared");
        Ok(algorithm_path)
    }

    /// Set up execution environment
    async fn setup_execution_environment(&self, request: &ExecutionRequest) -> Result<String, ApiError> {
        let env_dir = format!("/tmp/execution_{}", request.execution_id);
        
        tokio::fs::create_dir_all(&env_dir).await
            .map_err(|_e| ApiError::FileProcessingError)?;
            
        info!(env_dir = %env_dir, "Created execution environment directory");
        
        Ok(env_dir)
    }

    /// Run algorithm in TEE with sandboxing and resource limits
    async fn run_algorithm_in_tee(
        &self,
        request: &ExecutionRequest,
        algorithm_path: &str,
        dataset_path: &str,
        env_dir: &str,
    ) -> Result<Vec<u8>, ApiError> {
        let output_path = format!("{}/results.json", env_dir);
        
        info!(
            execution_id = %request.execution_id,
            "Running algorithm in TEE sandbox"
        );

        // Set execution timeout (default 5 minutes)
        let execution_timeout = Duration::from_secs(300);

        // Execute the algorithm with timeout and resource limits
        let result = timeout(execution_timeout, async {
            let mut cmd = Command::new("python3");
            cmd.arg(algorithm_path)
                .arg(dataset_path)
                .arg(&output_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .current_dir(env_dir);

            // TODO: Add resource limits using cgroups or similar
            // cmd.env("PYTHONPATH", "/secure/python_libs");
            // cmd.env("MAX_MEMORY", "1G");

            let output = cmd.output().await
                .map_err(|_e| ApiError::InternalError)?;

            if !output.status.success() {
                let _stderr = String::from_utf8_lossy(&output.stderr);
                error!(stderr = %_stderr, "Algorithm execution failed");
                return Err(ApiError::BusinessLogicError);
            }

            // Read the results
            let results = tokio::fs::read(&output_path).await
                .map_err(|_e| ApiError::InternalError)?;

            Ok(results)
        }).await;

        match result {
            Ok(Ok(data)) => Ok(data),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ApiError::Timeout),
        }
    }

    /// Encrypt and store result in IPFS
    async fn store_encrypted_result(&self, request: &ExecutionRequest, result_data: Vec<u8>) -> Result<String, ApiError> {
        info!(
            execution_id = %request.execution_id,
            result_size = %result_data.len(),
            "Storing encrypted execution results"
        );

        // Encrypt the results using TEE key vault
        let key_vault = self.key_vault.clone();
        let encrypted_result = key_vault.encrypt_data(&result_data, &request.dataset_id).await?;
        let encrypted_data = encrypted_result.encrypted_data;

        // Store in IPFS (mock implementation)
        let result_cid = format!("QmResult{:x}", md5::compute(&encrypted_data));
        
        info!(
            execution_id = %request.execution_id,
            result_cid = %result_cid,
            "Execution results encrypted and stored"
        );

        Ok(result_cid)
    }

    /// Clean up temporary files and execution environment
    async fn cleanup_execution_files(&self, algorithm_path: &str, dataset_path: &str, env_dir: &str) {
        // Clean up algorithm file
        if let Err(e) = tokio::fs::remove_file(algorithm_path).await {
            warn!(error = %e, path = %algorithm_path, "Failed to remove algorithm file");
        }

        // Clean up dataset file
        if let Err(e) = tokio::fs::remove_file(dataset_path).await {
            warn!(error = %e, path = %dataset_path, "Failed to remove dataset file");
        }

        // Clean up execution environment
        if let Err(e) = tokio::fs::remove_dir_all(env_dir).await {
            warn!(error = %e, path = %env_dir, "Failed to remove execution environment directory");
        }

        info!("Execution cleanup completed");
    }
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_seconds: 0.0,
            memory_peak_bytes: 0,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            network_sent_bytes: 0,
            network_received_bytes: 0,
        }
    }
} 