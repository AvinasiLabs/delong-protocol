//! Algorithm Executor Module
//!
//! This module handles the complete lifecycle of algorithm execution,
//! including downloading from IPFS, building Docker images, running containers,
//! and recording results on the blockchain.

use crate::{
    config::ExecutorConfig,
    infra::{contracts::ContractCaller, db::Database, TeeCryptoService},
    models::{
        algorithm_execution::{AlgorithmExecution, ContainerResult, ExecutionContext},
        blockchain_transaction::{BlockchainTransaction, EntityType},
    },
};
use alloy::primitives::{Address, U256};
use bollard::{
    container::{
        Config as ContainerConfig, CreateContainerOptions, LogOutput, LogsOptions,
        RemoveContainerOptions, WaitContainerOptions,
    },
    image::BuildImageOptions,
    Docker,
};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use futures_util::{StreamExt, TryStreamExt};
use ipfs_api_backend_hyper::IpfsApi;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tar::Archive;
use tokio::{
    fs,
    io::AsyncWriteExt,
    sync::{mpsc, RwLock},
    time::sleep,
};
use tracing::{debug, error, info, warn};

/// Errors that can occur during algorithm execution
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("IPFS error: {0}")]
    Ipfs(String),

    #[error("Container execution failed: {0}")]
    ContainerFailed(String),

    #[error("Build size limit exceeded: {size} > {limit}")]
    BuildSizeTooLarge { size: u64, limit: u64 },

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Execution timeout")]
    Timeout,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Application error: {0}")]
    App(#[from] crate::AppError),
}

type Result<T> = std::result::Result<T, ExecutorError>;

/// Algorithm execution event
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    /// Resolve algorithm at specified time
    Resolve {
        exe_id: i64,
        algo_cid: String,
        resolved_at: DateTime<Utc>,
    },
    /// Execute algorithm immediately
    Execute { exe_id: i64 },
}

/// Main algorithm executor
pub struct Executor {
    db: Arc<Database>,
    docker: Docker,
    ipfs_client: Arc<ipfs_api_backend_hyper::IpfsClient>,
    contract_caller: Arc<ContractCaller>,
    tee_crypto: Arc<TeeCryptoService>,
    config: ExecutorConfig,
    event_tx: mpsc::Sender<ExecutionEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<ExecutionEvent>>>,
    active_executions: Arc<RwLock<HashMap<i64, String>>>, // exe_id -> container_id
}

impl Executor {
    /// Create a new algorithm executor
    pub fn new(
        db: Arc<Database>,
        ipfs_client: Arc<ipfs_api_backend_hyper::IpfsClient>,
        contract_caller: Arc<ContractCaller>,
        tee_crypto: Arc<TeeCryptoService>,
        config: ExecutorConfig,
    ) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        let (event_tx, event_rx) = mpsc::channel(100);

        Ok(Self {
            db,
            docker,
            ipfs_client,
            contract_caller,
            tee_crypto,
            config,
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start the executor event loop
    pub async fn start(&self) -> Result<()> {
        info!("Starting algorithm executor");

        // Ensure working directory exists
        fs::create_dir_all(&self.config.working_directory).await?;

        // Start event processing loop
        let mut rx = self.event_rx.write().await;
        while let Some(event) = rx.recv().await {
            self.handle_event(event).await;
        }

        Ok(())
    }

    /// Schedule algorithm resolution
    pub async fn schedule_resolve(
        &self,
        exe_id: i64,
        algo_cid: String,
        resolved_at: DateTime<Utc>,
    ) -> Result<()> {
        self.event_tx
            .send(ExecutionEvent::Resolve {
                exe_id,
                algo_cid,
                resolved_at,
            })
            .await
            .map_err(|_| ExecutorError::ContainerFailed("Failed to send event".into()))?;

        info!("Scheduled resolution for execution {}", exe_id);
        Ok(())
    }

    /// Schedule algorithm execution
    pub async fn schedule_execution(&self, exe_id: i64) -> Result<()> {
        self.event_tx
            .send(ExecutionEvent::Execute { exe_id })
            .await
            .map_err(|_| ExecutorError::ContainerFailed("Failed to send event".into()))?;

        info!("Scheduled execution for {}", exe_id);
        Ok(())
    }

    /// Handle a single event
    async fn handle_event(&self, event: ExecutionEvent) {
        match event {
            ExecutionEvent::Resolve {
                exe_id,
                algo_cid,
                resolved_at,
            } => {
                self.handle_resolve(exe_id, algo_cid, resolved_at).await;
            }
            ExecutionEvent::Execute { exe_id } => {
                if let Err(e) = self.execute_task(exe_id).await {
                    error!("Task execution failed for {}: {:?}", exe_id, e);
                }
            }
        }
    }

    /// Handle algorithm resolution
    async fn handle_resolve(&self, exe_id: i64, algo_cid: String, resolved_at: DateTime<Utc>) {
        // Wait until resolution time
        let now = Utc::now();
        let delay =
            (resolved_at - now).to_std().unwrap_or(Duration::ZERO) + Duration::from_secs(60);

        if delay > Duration::ZERO {
            sleep(delay).await;
        }

        info!(
            "Resolving algorithm {} at {} (scheduled for {})",
            algo_cid,
            Utc::now(),
            resolved_at
        );

        // Call contract to resolve
        match self
            .contract_caller
            .resolve_algorithm(algo_cid.clone(), U256::from(exe_id as u64))
            .await
        {
            Ok(tx_hash) => {
                info!("Resolved algorithm {} with tx: {}", algo_cid, tx_hash);
            }
            Err(e) => {
                error!("Failed to resolve algorithm {}: {:?}", algo_cid, e);
            }
        }
    }

    /// Execute a task using the new unified model
    pub async fn execute_task(&self, exe_id: i64) -> Result<()> {
        info!("Starting execution task {}", exe_id);

        // Load execution context from new model
        let context = AlgorithmExecution::load_context(&self.db.pool, exe_id).await?;

        // Update status to running
        AlgorithmExecution::update_status(&self.db.pool, exe_id, "running").await?;

        // Execute the algorithm and record results
        let result = match self.run_algorithm_execution(&context).await {
            Ok(container_result) => container_result,
            Err(e) => {
                error!("Execution {} failed: {:?}", exe_id, e);
                // Convert error to ContainerResult
                match e {
                    ExecutorError::Timeout => ContainerResult::Timeout,
                    _ => ContainerResult::Failed {
                        error: format!("{:?}", e),
                        exit_code: -1,
                    },
                }
            }
        };

        // Start database transaction to record results and blockchain interaction
        let mut tx = self.db.pool.begin().await?;

        // Record execution result to database
        AlgorithmExecution::record_result(&mut tx, exe_id, result.clone()).await?;

        // Record execution on blockchain for ALL executions (not just successful ones)
        let blockchain_result = self.record_execution(&context, &result).await;

        // Handle blockchain recording result
        match blockchain_result {
            Ok(tx_hash) => {
                // Create blockchain transaction record
                BlockchainTransaction::create(
                    &mut tx,
                    tx_hash,
                    exe_id,
                    EntityType::Execution, // Use 'execution' as entity type
                    crate::models::blockchain_transaction::TransactionStatus::Pending,
                )
                .await?;

                info!("Execution {} completed and recorded on blockchain", exe_id);
            }
            Err(e) => {
                error!(
                    "Failed to record execution {} on blockchain: {:?}",
                    exe_id, e
                );
                // Still commit the database changes even if blockchain recording fails
            }
        }

        // Commit database transaction
        tx.commit().await?;

        Ok(())
    }

/// Execute algorithm using new unified model with parallel downloads
    async fn run_algorithm_execution(&self, context: &ExecutionContext) -> Result<ContainerResult> {
        info!("Running algorithm execution for context: {:?}", context.id);

        // Perform parallel downloads of algorithm and dataset
        let (work_dir, dataset_path) = tokio::try_join!(
            self.download_and_extract(&context.algo_cid),
            self.prepare_dataset_parallel(&context.dataset_name)
        )?;

        // Verify Dockerfile exists
        let dockerfile_path = work_dir.join("Dockerfile");
        info!("Looking for Dockerfile at: {:?}", dockerfile_path);

        if !dockerfile_path.exists() {
            error!("Dockerfile not found at {:?}", dockerfile_path);

            // Try to list what files are actually present for debugging
            if let Ok(mut entries) = fs::read_dir(&work_dir).await {
                error!("Files in work directory:");
                while let Some(entry) = entries.next_entry().await.ok().flatten() {
                    let path = entry.path();
                    error!("  - {:?}", path.file_name().unwrap_or_default());
                }
            }

            return Err(ExecutorError::ContainerFailed(
                "Dockerfile not found in algorithm package".into(),
            ));
        }

        info!("Found Dockerfile, proceeding with build");

        // Build Docker image
        let image_name = format!("delong-algo-{}", context.id);
        self.build_docker_image(&work_dir, &image_name).await?;

        // Run container and convert result to ContainerResult
        let (output, error, success) = self
            .run_container(&image_name, context.id, dataset_path)
            .await?;

        // Cleanup work directory
        if let Some(parent) = work_dir.parent() {
            if parent.to_string_lossy().contains("delong-algo-") {
                if let Err(e) = fs::remove_dir_all(parent).await {
                    warn!("Failed to cleanup temp directory: {:?}", e);
                }
            }
        } else {
            if let Err(e) = fs::remove_dir_all(&work_dir).await {
                warn!("Failed to cleanup work directory: {:?}", e);
            }
        }

        // Convert to ContainerResult
        if success {
            Ok(ContainerResult::Success {
                output: String::from_utf8_lossy(&output).to_string(),
                exit_code: 0,
            })
        } else {
            Ok(ContainerResult::Failed {
                error: String::from_utf8_lossy(&error).to_string(),
                exit_code: 1,
            })
        }
    }

    /// Download and extract algorithm from IPFS
    async fn download_and_extract(&self, cid: &str) -> Result<PathBuf> {
        info!("Downloading algorithm from IPFS: {}", cid);

        // Create a unique temporary directory using timestamp and CID
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let safe_cid = cid.chars().take(8).collect::<String>();
        let temp_base = format!("/tmp/delong-algo-{}-{}", timestamp, safe_cid);
        let temp_path = PathBuf::from(&temp_base);

        // Ensure the directory is clean
        if temp_path.exists() {
            fs::remove_dir_all(&temp_path).await?;
        }
        fs::create_dir_all(&temp_path).await?;

        let tar_gz_path = temp_path.join("algorithm.tar.gz");
        let extract_dir = temp_path.join("extracted");
        // Don't create extract_dir here, will be created before unpacking

        // Download from IPFS
        let data = self
            .ipfs_client
            .cat(cid)
            .map_err(|e| ExecutorError::Ipfs(e.to_string()))
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await
            .map_err(|e| ExecutorError::Ipfs(e.to_string()))?;

        info!("Downloaded {} bytes from IPFS", data.len());

        // Check size limit
        if data.len() as u64 > self.config.build_size_limit {
            return Err(ExecutorError::BuildSizeTooLarge {
                size: data.len() as u64,
                limit: self.config.build_size_limit,
            });
        }

        // Write to file
        let mut file = fs::File::create(&tar_gz_path).await?;
        file.write_all(&data).await?;
        file.sync_all().await?;
        drop(file);

        // Extract tar.gz
        // Ensure extract directory is clean before unpacking
        if extract_dir.exists() {
            std::fs::remove_dir_all(&extract_dir)?;
        }
        std::fs::create_dir_all(&extract_dir)?;

        let tar_gz_file = std::fs::File::open(&tar_gz_path)?;
        let tar = GzDecoder::new(tar_gz_file);
        let mut archive = Archive::new(tar);
        archive.unpack(&extract_dir)?;

        info!("Extracted archive to: {:?}", extract_dir);

        // GitHub archives create a top-level directory like "repo-name-commit-hash"
        // We need to find the actual project directory inside extract_dir
        let mut entries = fs::read_dir(&extract_dir).await?;
        let mut actual_dir = extract_dir.clone();

        // Check if there's a single top-level directory (typical for GitHub archives)
        let mut dir_count = 0;
        let mut first_dir = None;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                dir_count += 1;
                if first_dir.is_none() {
                    first_dir = Some(path);
                }
            }
        }

        // If there's exactly one directory, use it as the actual project root
        if dir_count == 1 && first_dir.is_some() {
            actual_dir = first_dir.unwrap();
            info!("Found project directory inside archive: {:?}", actual_dir);
        } else {
            info!(
                "Using extract directory as is (found {} directories)",
                dir_count
            );
        }

        // List contents of the actual directory for debugging
        let mut entries = fs::read_dir(&actual_dir).await?;
        info!("Contents of algorithm directory:");
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            let is_dir = path.is_dir();
            info!(
                "  {} {}",
                if is_dir { "[DIR]" } else { "[FILE]" },
                file_name
            );
        }

        // Return the actual directory path
        // The temporary directory will be cleaned up in execute_algorithm
        Ok(actual_dir)
    }

    /// Build Docker image from directory
    async fn build_docker_image(&self, build_dir: &PathBuf, image_name: &str) -> Result<()> {
        info!("Building Docker image: {}", image_name);

        // Create tar archive of the build directory
        let tar_data = self.create_tar_archive(build_dir).await?;

        let build_options = BuildImageOptions {
            dockerfile: "Dockerfile",
            t: image_name,
            rm: true,
            forcerm: true,
            nocache: true,
            ..Default::default()
        };

        let mut build_stream = self
            .docker
            .build_image(build_options, None, Some(tar_data.into()));

        // Process build output
        while let Some(build_info) = build_stream.next().await {
            match build_info {
                Ok(info) => {
                    debug!("Build output: {:?}", info);
                }
                Err(e) => return Err(e.into()),
            }
        }

        info!("Successfully built image: {}", image_name);
        Ok(())
    }

    /// Create a tar archive from directory
    async fn create_tar_archive(&self, dir: &PathBuf) -> Result<Vec<u8>> {
        use tar::Builder;

        let mut tar_data = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_data);
            builder.append_dir_all(".", dir)?;
            builder.finish()?;
        }
        Ok(tar_data)
    }

    /// Prepare dataset for parallel execution by downloading from IPFS to local directory
    async fn prepare_dataset_parallel(&self, dataset_name: &str) -> Result<PathBuf> {
        self.prepare_dataset(dataset_name).await
    }

    /// Prepare dataset for execution by downloading from IPFS to local directory
    async fn prepare_dataset(&self, dataset_name: &str) -> Result<PathBuf> {
        // Find dataset in database
        let dataset = crate::models::dataset::Dataset::find_by_name(&self.db.pool, dataset_name)
            .await?
            .ok_or_else(|| {
                ExecutorError::NotFound(format!("Dataset {} not found in database", dataset_name))
            })?;

        // Create a directory for datasets if it doesn't exist
        let dataset_dir = PathBuf::from("/tmp/delong-datasets");
        fs::create_dir_all(&dataset_dir).await?;

        // Create a directory for this dataset
        let safe_dirname = dataset_name.replace('/', "-");
        let dataset_folder = dataset_dir.join(&safe_dirname);


        // Define the CSV file path within the directory
        let dataset_file = dataset_folder.join(format!("{}.csv", safe_dirname));

        // Check if dataset file already exists locally
        if dataset_file.exists() {
            info!("Dataset already exists at: {:?}", dataset_file);
            return Ok(dataset_folder); // Return the folder, not the file
        }

        // Create the dataset-specific directory only if we need to download
        if !dataset_folder.exists() {
            fs::create_dir_all(&dataset_folder).await?;
        }

        // Download from IPFS using the IPFS client
        info!(
            "Downloading dataset {} from IPFS CID {}",
            dataset_name, dataset.ipfs_cid
        );

        use futures::TryStreamExt;
        let encrypted_data = self
            .ipfs_client
            .cat(&dataset.ipfs_cid)
            .map_ok(|chunk| chunk.to_vec())
            .try_concat()
            .await
            .map_err(|e| ExecutorError::Ipfs(format!("Failed to download from IPFS: {}", e)))?;

        // Decrypt the dataset using the universal TEE key
        info!("Decrypting dataset with UNIVERSAL_DATASET_KEY");
        let dataset_key_id = "UNIVERSAL_DATASET_KEY";
        let decrypted_data = self
            .tee_crypto
            .decrypt_dataset(&encrypted_data, dataset_key_id)
            .await
            .map_err(|e| ExecutorError::Internal(format!("Failed to decrypt dataset: {}", e)))?;

        // Write decrypted data to the CSV file within the directory
        fs::write(&dataset_file, &decrypted_data).await?;
        info!(
            "Successfully downloaded and decrypted dataset to: {:?} ({} bytes)",
            dataset_file,
            decrypted_data.len()
        );

        Ok(dataset_folder) // Return the folder path for mounting
    }

    /// Run Docker container
    async fn run_container(
        &self,
        image_name: &str,
        exe_id: i64,
        dataset_path: PathBuf,
    ) -> Result<(Vec<u8>, Vec<u8>, bool)> {
        info!("Running container for execution {}", exe_id);
        info!(
            "Mounting dataset directory from: {:?} to /data",
            dataset_path
        );

        // Container configuration
        let container_config = ContainerConfig {
            image: Some(image_name.to_string()),
            env: Some(vec![
                format!("DATASET_PATH=/data"),
                format!("EXECUTION_ID={}", exe_id),
            ]),
            host_config: Some(bollard::models::HostConfig {
                binds: Some(vec![format!("{}:/data:ro", dataset_path.display())]),
                memory: Some(1 << 30),          // 1GB
                nano_cpus: Some(1_000_000_000), // 1 CPU
                security_opt: Some(vec!["no-new-privileges".to_string()]),
                ..Default::default()
            }),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        // Create container
        let container_name = format!("delong-exe-{}", exe_id);
        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name,
                    ..Default::default()
                }),
                container_config,
            )
            .await?;

        let container_id = container.id;

        // Track active container
        {
            let mut active = self.active_executions.write().await;
            active.insert(exe_id, container_id.clone());
        }

        // Start container
        self.docker
            .start_container::<String>(&container_id, None)
            .await?;

        // Wait for container with timeout
        let timeout_duration = Duration::from_secs(self.config.execution_timeout);
        let wait_result =
            tokio::time::timeout(timeout_duration, self.wait_for_container(&container_id)).await;

        // Cleanup container
        self.cleanup_container(exe_id).await;

        match wait_result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                let success = exit_code == 0;
                Ok((stdout, stderr, success))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ExecutorError::Timeout),
        }
    }

    /// Wait for container to complete
    async fn wait_for_container(&self, container_id: &str) -> Result<(Vec<u8>, Vec<u8>, i64)> {
        // Wait for exit
        let mut wait_stream = self
            .docker
            .wait_container(container_id, None::<WaitContainerOptions<String>>);

        let exit_code = match wait_stream.next().await {
            Some(Ok(wait_response)) => wait_response.status_code,
            Some(Err(e)) => return Err(e.into()),
            None => {
                return Err(ExecutorError::ContainerFailed(
                    "Container wait failed".into(),
                ))
            }
        };

        // Collect logs
        let logs_options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut log_stream = self.docker.logs(container_id, Some(logs_options));

        while let Some(log_result) = log_stream.next().await {
            match log_result {
                Ok(LogOutput::StdOut { message }) => stdout.extend_from_slice(&message),
                Ok(LogOutput::StdErr { message }) => stderr.extend_from_slice(&message),
                Ok(_) => {}
                Err(e) => warn!("Error reading logs: {}", e),
            }
        }

        // Log container output for debugging
        if !stdout.is_empty() {
            info!(
                "Container stdout: {}",
                String::from_utf8_lossy(&stdout).trim()
            );
        }
        if !stderr.is_empty() {
            warn!(
                "Container stderr: {}",
                String::from_utf8_lossy(&stderr).trim()
            );
        }

        if exit_code != 0 {
            error!(
                "Container exited with code {}: stdout='{}', stderr='{}'",
                exit_code,
                String::from_utf8_lossy(&stdout).trim(),
                String::from_utf8_lossy(&stderr).trim()
            );
        }

        Ok((stdout, stderr, exit_code))
    }

    /// Cleanup container
    async fn cleanup_container(&self, exe_id: i64) {
        let container_id = {
            let mut active = self.active_executions.write().await;
            active.remove(&exe_id)
        };

        if let Some(container_id) = container_id {
            // Stop container
            if let Err(e) = self.docker.stop_container(&container_id, None).await {
                warn!("Failed to stop container {}: {}", container_id, e);
            }

            // Remove container
            let remove_options = RemoveContainerOptions {
                force: true,
                v: true,
                ..Default::default()
            };

            if let Err(e) = self
                .docker
                .remove_container(&container_id, Some(remove_options))
                .await
            {
                warn!("Failed to remove container {}: {}", container_id, e);
            }

            debug!(
                "Cleaned up container {} for execution {}",
                container_id, exe_id
            );
        }
    }

    /// Record execution on blockchain using unified model
    async fn record_execution(
        &self,
        context: &ExecutionContext,
        result: &ContainerResult,
    ) -> Result<String> {
        // Determine if execution was successful
        let execution_success = matches!(result, ContainerResult::Success { .. });

        // Get scientist wallet address
        let scientist_address = context
            .wallet
            .parse::<Address>()
            .map_err(|e| ExecutorError::ContainerFailed(format!("Invalid address: {}", e)))?;

        // Get dataset ID - if we have it in context, use it; otherwise look it up
        let dataset_id = if let Some(id) = context.dataset_id {
            U256::from(id as u64)
        } else {
            // Look up dataset to get its ID
            let dataset =
                crate::models::dataset::Dataset::find_by_name(&self.db.pool, &context.dataset_name)
                    .await?
                    .ok_or_else(|| {
                        ExecutorError::NotFound(format!(
                            "Dataset {} not found",
                            context.dataset_name
                        ))
                    })?;
            U256::from(dataset.id as u64)
        };

        // Record execution on blockchain
        let execution_id = U256::from(context.id as u64);
        let dataset_cid = context.dataset_cid.clone().unwrap_or_else(|| {
            // Fallback to empty string if dataset CID is not available
            String::new()
        });
        let tx_hash = self
            .contract_caller
            .record_execution(
                execution_id,
                scientist_address,
                context.algo_cid.clone(),
                dataset_id,
                dataset_cid,
                execution_success,
            )
            .await
            .map_err(|e| ExecutorError::ContainerFailed(format!("Contract call failed: {}", e)))?;

        info!(
            "Recorded execution for context {}: algo_cid={}, success={}, tx={}",
            context.id, context.algo_cid, execution_success, tx_hash
        );

        Ok(tx_hash)
    }

    /// Get active execution count
    pub async fn active_count(&self) -> usize {
        self.active_executions.read().await.len()
    }
}

/// Create and start the algorithm executor service
pub async fn create_executor_service(
    db: Arc<Database>,
    ipfs_client: Arc<ipfs_api_backend_hyper::IpfsClient>,
    contract_caller: Arc<ContractCaller>,
    tee_crypto: Arc<TeeCryptoService>,
    config: ExecutorConfig,
) -> Result<Arc<Executor>> {
    let executor = Arc::new(Executor::new(
        db,
        ipfs_client,
        contract_caller,
        tee_crypto,
        config,
    )?);

    // Start the executor in background
    let executor_clone = executor.clone();
    tokio::spawn(async move {
        if let Err(e) = executor_clone.start().await {
            error!("Algorithm executor failed: {:?}", e);
        }
    });

    Ok(executor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = ExecutorConfig::default();
        assert_eq!(config.build_size_limit, 100 * 1024 * 1024);
        assert_eq!(config.execution_timeout, 3600);
        assert_eq!(config.max_concurrent, 10);
    }
}
