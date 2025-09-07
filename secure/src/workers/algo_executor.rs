//! Algorithm Executor Module
//!
//! This module handles the complete lifecycle of algorithm execution,
//! including downloading from IPFS, building Docker images, running containers,
//! and recording results on the blockchain.

use crate::{
    config::ExecutorConfig,
    infra::{contracts::ContractCaller, db::Database},
    models::{
        algo::Algo,
        algo_exe::{AlgoExe, ExecutionStatus},
        blockchain_transaction::{BlockchainTransaction, EntityType},
        data_usage::DataUsage,
        FindById,
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
pub struct AlgoExecutor {
    db: Arc<Database>,
    docker: Docker,
    ipfs_client: Arc<ipfs_api_backend_hyper::IpfsClient>,
    contract_caller: Arc<ContractCaller>,
    config: ExecutorConfig,
    event_tx: mpsc::Sender<ExecutionEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<ExecutionEvent>>>,
    active_executions: Arc<RwLock<HashMap<i64, String>>>, // exe_id -> container_id
}

impl AlgoExecutor {
    /// Create a new algorithm executor
    pub fn new(
        db: Arc<Database>,
        ipfs_client: Arc<ipfs_api_backend_hyper::IpfsClient>,
        contract_caller: Arc<ContractCaller>,
        config: ExecutorConfig,
    ) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        let (event_tx, event_rx) = mpsc::channel(100);

        Ok(Self {
            db,
            docker,
            ipfs_client,
            contract_caller,
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
        fs::create_dir_all(&self.config.dataset_base_path).await?;

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
                self.handle_execute(exe_id).await;
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

    /// Handle algorithm execution
    async fn handle_execute(&self, exe_id: i64) {
        info!("Starting execution {}", exe_id);

        // Update status to running
        if let Err(e) =
            AlgoExe::update_status(&self.db.pool, exe_id, ExecutionStatus::Running).await
        {
            error!("Failed to update status to running: {:?}", e);
            return;
        }

        // Execute the algorithm
        match self.execute_algorithm(exe_id).await {
            Ok((output, error, success)) => {
                self.handle_completion(exe_id, success, output, error).await;
            }
            Err(e) => {
                error!("Execution {} failed: {:?}", exe_id, e);
                self.handle_error(exe_id, e).await;
            }
        }
    }

    /// Execute an algorithm
    async fn execute_algorithm(&self, exe_id: i64) -> Result<(Vec<u8>, Vec<u8>, bool)> {
        // Get execution details
        let execution = AlgoExe::find_by_id(&self.db.pool, exe_id)
            .await?
            .ok_or_else(|| ExecutorError::NotFound(format!("Execution {} not found", exe_id)))?;

        // Get algorithm details
        let algo = Algo::find_by_id(&self.db.pool, execution.algo_id)
            .await?
            .ok_or_else(|| {
                ExecutorError::NotFound(format!("Algorithm {} not found", execution.algo_id))
            })?;

        // Download and extract algorithm from IPFS
        let work_dir = self.download_and_extract(&algo.cid).await?;

        // Verify Dockerfile exists
        let dockerfile_path = work_dir.join("Dockerfile");
        if !dockerfile_path.exists() {
            return Err(ExecutorError::ContainerFailed(
                "Dockerfile not found in algorithm package".into(),
            ));
        }

        // Build Docker image
        let image_name = format!("delong-algo-{}", exe_id);
        self.build_docker_image(&work_dir, &image_name).await?;

        // Prepare dataset path
        let dataset_path = self.prepare_dataset(&execution.used_dataset).await?;

        // Run container
        let (output, error, success) = self
            .run_container(&image_name, exe_id, dataset_path)
            .await?;

        // Cleanup work directory
        if let Err(e) = fs::remove_dir_all(&work_dir).await {
            warn!("Failed to cleanup work directory: {:?}", e);
        }

        Ok((output, error, success))
    }

    /// Download and extract algorithm from IPFS
    async fn download_and_extract(&self, cid: &str) -> Result<PathBuf> {
        info!("Downloading algorithm from IPFS: {}", cid);

        // Create temporary directory
        let temp_dir = tempfile::tempdir()?;
        let tar_gz_path = temp_dir.path().join("algorithm.tar.gz");
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir_all(&extract_dir).await?;

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
        let tar_gz_file = std::fs::File::open(&tar_gz_path)?;
        let tar = GzDecoder::new(tar_gz_file);
        let mut archive = Archive::new(tar);
        archive.unpack(&extract_dir)?;

        // Keep temp_dir alive (will be cleaned up manually)
        let _ = temp_dir.keep();

        Ok(extract_dir)
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

    /// Prepare dataset for execution
    async fn prepare_dataset(&self, dataset_name: &str) -> Result<PathBuf> {
        let dataset_path = self.config.dataset_base_path.join(dataset_name);

        if !dataset_path.exists() {
            // Try to find in database
            let dataset =
                crate::models::dataset::Dataset::find_by_name(&self.db.pool, dataset_name)
                    .await?
                    .ok_or_else(|| {
                        ExecutorError::NotFound(format!("Dataset {} not found", dataset_name))
                    })?;

            if let Some(file_path) = dataset.file_path {
                return Ok(PathBuf::from(file_path));
            }

            return Err(ExecutorError::NotFound(format!(
                "Dataset {} file not found",
                dataset_name
            )));
        }

        Ok(dataset_path)
    }

    /// Run Docker container
    async fn run_container(
        &self,
        image_name: &str,
        exe_id: i64,
        dataset_path: PathBuf,
    ) -> Result<(Vec<u8>, Vec<u8>, bool)> {
        info!("Running container for execution {}", exe_id);

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

    /// Handle successful completion
    async fn handle_completion(&self, exe_id: i64, success: bool, output: Vec<u8>, error: Vec<u8>) {
        info!(
            "Execution {} completed: success={}, output_len={}, error_len={}",
            exe_id,
            success,
            output.len(),
            error.len()
        );

        let status = if success {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };

        // Update execution status
        if let Err(e) = AlgoExe::update_status(&self.db.pool, exe_id, status).await {
            error!("Failed to update execution status: {:?}", e);
            return;
        }

        // Update execution result
        let output_str = String::from_utf8_lossy(&output);
        let error_str = if !error.is_empty() {
            Some(String::from_utf8_lossy(&error).to_string())
        } else {
            None
        };

        if let Err(e) =
            AlgoExe::update_completed(&self.db.pool, exe_id, output_str.to_string(), error_str)
                .await
        {
            error!("Failed to update execution result: {:?}", e);
            return;
        }

        // Record data usage if successful
        if success {
            if let Err(e) = self.record_data_usage(exe_id).await {
                error!("Failed to record data usage: {:?}", e);
            }
        }
    }

    /// Handle execution error
    async fn handle_error(&self, exe_id: i64, error: ExecutorError) {
        error!("Execution {} failed with error: {:?}", exe_id, error);

        // Update status to failed
        if let Err(e) = AlgoExe::update_status(&self.db.pool, exe_id, ExecutionStatus::Failed).await
        {
            error!("Failed to update execution status: {:?}", e);
        }

        // Store error message
        let error_msg = format!("Execution failed: {:?}", error);
        if let Err(e) =
            AlgoExe::update_completed(&self.db.pool, exe_id, String::new(), Some(error_msg)).await
        {
            error!("Failed to update execution error: {:?}", e);
        }
    }

    /// Record data usage on blockchain
    async fn record_data_usage(&self, exe_id: i64) -> Result<()> {
        // Get execution details
        let execution = AlgoExe::find_by_id(&self.db.pool, exe_id)
            .await?
            .ok_or_else(|| ExecutorError::NotFound(format!("Execution {} not found", exe_id)))?;

        // Get algorithm details
        let algo = Algo::find_by_id(&self.db.pool, execution.algo_id)
            .await?
            .ok_or_else(|| {
                ExecutorError::NotFound(format!("Algorithm {} not found", execution.algo_id))
            })?;

        // Start database transaction
        let mut tx = self.db.pool.begin().await?;

        // Create data usage record
        let usage = DataUsage::create(
            &mut tx,
            execution.scientist_wallet.clone(),
            algo.cid.clone(),
            execution.used_dataset.clone(),
            Utc::now(),
        )
        .await?;

        // Record on blockchain
        let scientist_address = execution
            .scientist_wallet
            .parse::<Address>()
            .map_err(|e| ExecutorError::ContainerFailed(format!("Invalid address: {}", e)))?;

        // Look up the dataset to get its ID
        let dataset =
            crate::models::dataset::Dataset::find_by_name(&self.db.pool, &execution.used_dataset)
                .await?
                .ok_or_else(|| {
                    ExecutorError::NotFound(format!("Dataset {} not found", execution.used_dataset))
                })?;

        let tx_hash = self
            .contract_caller
            .record_data_usage(
                scientist_address,
                algo.cid.clone(),
                U256::from(dataset.id as u64),
                execution.used_dataset.clone(),
            )
            .await
            .map_err(|e| ExecutorError::ContainerFailed(format!("Contract call failed: {}", e)))?;

        // Create transaction record
        BlockchainTransaction::create(
            &mut tx,
            tx_hash.clone(),
            usage.id,
            EntityType::DataUsage,
            crate::models::blockchain_transaction::TransactionStatus::Pending,
        )
        .await?;

        // Commit transaction
        tx.commit().await.map_err(|e| ExecutorError::Database(e))?;

        info!(
            "Recorded data usage for execution {}, tx: {}",
            exe_id, tx_hash
        );

        Ok(())
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
    config: ExecutorConfig,
) -> Result<Arc<AlgoExecutor>> {
    let executor = Arc::new(AlgoExecutor::new(db, ipfs_client, contract_caller, config)?);

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
