//! Algorithm runtime execution module
//!
//! This module handles the execution of algorithms in a secure environment,
//! managing the lifecycle of algorithm runs and ensuring proper isolation.

use crate::{
    config::Config,
    error::{AppError, Result},
    infra::{contracts::ContractCaller, db::Database},
    models::{
        algo::Algo,
        algo_exe::AlgoExe,
        blockchain_transaction::{BlockchainTransaction, EntityType, TransactionStatus},
        data_usage::DataUsage,
        pg_types::ExecutionStatus,
        FindById,
    },
};
use alloy::primitives::U256;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use std::{
    fs::{self, File},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tar::Archive;
use tempfile::TempDir;
use tokio::time::{interval, sleep};
use tracing::{error, info, instrument, warn};

use super::scheduler::{AlgoScheduler, SchedulerConfig, SchedulerError, SchedulerHandler};

/// Runtime service that manages algorithm execution lifecycle
pub struct RuntimeService {
    db: Database,
    config: Config,
    ipfs_gateway: String,
    dataset_base_path: PathBuf,
    scheduler: Arc<AlgoScheduler>,
    #[allow(dead_code)]
    contract_caller: Arc<ContractCaller>,
}

/// Implementation of SchedulerHandler for runtime service
struct RuntimeHandler {
    db: Database,
    contract_caller: Arc<ContractCaller>,
    ws_hub: Arc<crate::infra::ws::Hub>,
}

#[async_trait]
impl SchedulerHandler for RuntimeHandler {
    async fn on_resolve(&self, exe_id: u64, algo_cid: &str, resolved_at: DateTime<Utc>) {
        info!(
            "Algorithm {} with CID {} resolved at {}",
            exe_id, algo_cid, resolved_at
        );

        // Send WebSocket notification
        let notifier = crate::infra::ws::Notifier::new(self.ws_hub.clone());
        notifier.push_status(exe_id.to_string(), "resolving").await;

        // Wait until resolution time
        let delay = resolved_at.signed_duration_since(Utc::now());
        if delay.num_seconds() > 0 {
            sleep(Duration::from_secs(delay.num_seconds() as u64)).await;
        }

        // Get votes to determine approval
        match crate::models::vote::Vote::count_by_algo_cid(&self.db.pool, algo_cid).await {
            Ok((approve_count, reject_count)) => {
                let approved = approve_count > reject_count;
                info!(
                    "Algorithm {} votes: {} approve, {} reject -> {}",
                    algo_cid,
                    approve_count,
                    reject_count,
                    if approved { "APPROVED" } else { "REJECTED" }
                );

                // Call contract to resolve algorithm
                match self
                    .contract_caller
                    .resolve_algorithm(algo_cid.to_string(), U256::from(exe_id))
                    .await
                {
                    Ok(tx_hash) => {
                        info!("Resolved algorithm {} with tx {}", algo_cid, tx_hash);
                    }
                    Err(e) => {
                        error!("Failed to resolve algorithm {}: {}", algo_cid, e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to count votes for algorithm {}: {}", algo_cid, e);
            }
        }
    }

    async fn on_run(&self, exe_id: u64) {
        info!("Starting execution for algorithm {}", exe_id);

        // Update status to RUNNING
        let exe_id_i64 = exe_id as i64;
        if let Err(e) =
            AlgoExe::update_status(&self.db.pool, exe_id_i64, ExecutionStatus::Running).await
        {
            error!("Failed to update execution status to RUNNING: {}", e);
        }

        // Send WebSocket notification
        let notifier = crate::infra::ws::Notifier::new(self.ws_hub.clone());
        notifier.push_status(exe_id.to_string(), "running").await;
    }

    async fn on_completed(&self, exe_id: u64, success: bool, output: Vec<u8>, error: Vec<u8>) {
        let exe_id_i64 = exe_id as i64;

        info!(
            "Execution {} completed with success={}, output_len={}, error_len={}",
            exe_id,
            success,
            output.len(),
            error.len()
        );

        // Convert output to string
        let output_str = String::from_utf8_lossy(&output).to_string();
        let error_str = if !error.is_empty() {
            Some(String::from_utf8_lossy(&error).to_string())
        } else {
            None
        };

        // Update execution result
        match AlgoExe::update_completed(
            &self.db.pool,
            exe_id_i64,
            output_str.clone(),
            error_str.clone(),
        )
        .await
        {
            Ok(algo_exe) => {
                if success {
                    // Record data usage
                    if let Err(e) = self.record_data_usage(algo_exe).await {
                        error!("Failed to record data usage: {}", e);
                    }
                }

                // Send WebSocket notification
                let notifier = crate::infra::ws::Notifier::new(self.ws_hub.clone());
                if success {
                    notifier
                        .push_notification(
                            exe_id.to_string(),
                            serde_json::json!({
                                "status": "completed",
                                "output": output_str
                            }),
                        )
                        .await;
                } else {
                    notifier
                        .push_error(exe_id.to_string(), crate::infra::ws::BizCode::TxFailed)
                        .await;
                }
            }
            Err(e) => {
                error!("Failed to update execution result: {}", e);
            }
        }
    }

    async fn on_error(&self, exe_id: u64, error: SchedulerError) {
        error!("Execution {} failed: {}", exe_id, error);

        let exe_id_i64 = exe_id as i64;
        if let Err(e) =
            AlgoExe::update_status(&self.db.pool, exe_id_i64, ExecutionStatus::Failed).await
        {
            error!("Failed to update execution status to FAILED: {}", e);
        }

        // Send WebSocket notification
        let notifier = crate::infra::ws::Notifier::new(self.ws_hub.clone());
        notifier
            .push_error(exe_id.to_string(), crate::infra::ws::BizCode::TxFailed)
            .await;
    }
}

impl RuntimeHandler {
    /// Record data usage on blockchain
    async fn record_data_usage(&self, algo_exe: AlgoExe) -> Result<()> {
        // Get algorithm details
        let algo = Algo::find_by_id(&self.db.pool, algo_exe.algo_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Algorithm not found".to_string()))?;

        let used_at = Utc::now();

        // Start database transaction
        let mut tx = self.db.pool.begin().await?;

        // Create data usage record
        let usage = DataUsage::create(
            &mut tx,
            algo_exe.scientist_wallet.clone(),
            algo.cid.clone(),
            algo_exe.used_dataset.clone(),
            used_at,
        )
        .await?;

        // Record usage on blockchain
        let scientist_address = algo_exe
            .scientist_wallet
            .parse::<alloy::primitives::Address>()
            .map_err(|e| AppError::Internal(format!("Invalid scientist wallet address: {}", e)))?;
        let tx_hash = self
            .contract_caller
            .record_data_usage(
                scientist_address,
                algo.cid.clone(),
                algo_exe.used_dataset.clone(),
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to record data usage: {}", e)))?;

        // Create blockchain transaction record
        BlockchainTransaction::create(
            &mut tx,
            tx_hash.clone(),
            usage.id,
            EntityType::DataUsage,
            TransactionStatus::Pending,
        )
        .await?;

        // Commit transaction
        tx.commit().await?;

        info!(
            "Recorded data usage for execution {}, tx: {}",
            algo_exe.id, tx_hash
        );
        Ok(())
    }
}

impl RuntimeService {
    /// Create a new runtime service
    pub fn new(
        db: Database,
        config: Config,
        contract_caller: Arc<ContractCaller>,
        ws_hub: Arc<crate::infra::ws::Hub>,
    ) -> Result<Self> {
        // Create scheduler configuration
        let scheduler_config = SchedulerConfig {
            channel_size: 100,
            build_size_limit: 100 << 20, // 100MB
            execution_timeout: config.runtime.execution_timeout_secs,
            runtime_image: "python:3.9-slim".to_string(),
            working_directory: PathBuf::from("/tmp/delong-algo"),
            max_concurrent: config.runtime.max_concurrent_executions,
        };

        // Create handler
        let handler = Arc::new(RuntimeHandler {
            db: db.clone(),
            contract_caller: contract_caller.clone(),
            ws_hub: ws_hub.clone(),
        });

        // Create scheduler
        let scheduler = Arc::new(
            AlgoScheduler::new(handler, scheduler_config)
                .map_err(|e| AppError::Internal(format!("Failed to create scheduler: {}", e)))?,
        );

        Ok(Self {
            db,
            config: config.clone(),
            ipfs_gateway: config.ipfs.gateway_url.clone(),
            dataset_base_path: PathBuf::from(&config.runtime.dataset_base_path),
            scheduler,
            contract_caller,
        })
    }

    /// Initialize runtime service
    pub async fn init(&self) -> Result<()> {
        info!("Initializing runtime service");

        // Ensure dataset base path exists
        fs::create_dir_all(&self.dataset_base_path).map_err(|e| {
            AppError::Internal(format!("Failed to create dataset directory: {}", e))
        })?;

        // Start scheduler
        self.scheduler.clone().start().await;

        // Recover pending executions
        self.recover_pending_executions().await?;

        Ok(())
    }

    /// Start the runtime worker
    pub async fn start(self: Arc<Self>) -> Result<()> {
        info!("Starting runtime worker");

        let mut check_interval = interval(Duration::from_secs(self.config.runtime.poll_interval));

        loop {
            check_interval.tick().await;

            match self.process_pending_executions().await {
                Ok(()) => {}
                Err(e) => {
                    error!("Error processing pending executions: {}", e);
                    // Continue processing instead of stopping
                }
            }
        }
    }

    /// Process pending algorithm executions
    #[instrument(skip(self))]
    async fn process_pending_executions(&self) -> Result<()> {
        // Find approved algorithms that are not yet running
        let pending_exes = AlgoExe::find_pending_to_run(&self.db.pool).await?;

        if !pending_exes.is_empty() {
            info!("Found {} pending executions", pending_exes.len());
        }

        for exe in pending_exes {
            // Check if we have capacity
            let active_count = self.scheduler.active_count().await;
            if active_count >= self.config.runtime.max_concurrent_executions {
                warn!(
                    "Max concurrent executions reached ({}/{})",
                    active_count, self.config.runtime.max_concurrent_executions
                );
                break;
            }

            // Schedule execution
            if let Err(e) = self.schedule_execution(exe).await {
                error!("Failed to schedule execution: {}", e);
            }
        }

        Ok(())
    }

    /// Schedule a single execution
    async fn schedule_execution(&self, exe: AlgoExe) -> Result<()> {
        info!(
            "Scheduling execution {} for algorithm {}",
            exe.id, exe.algo_id
        );

        // Get algorithm details
        let algo = Algo::find_by_id(&self.db.pool, exe.algo_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Algorithm not found".to_string()))?;

        // Prepare dataset path
        let dataset_path = self.prepare_dataset(&exe.used_dataset).await?;

        // Download and extract algorithm
        let work_dir = self.fetch_and_extract_algorithm(&algo.cid).await?;

        info!(
            "Scheduling execution {} with dataset {:?} and work dir {:?}",
            exe.id, dataset_path, work_dir
        );

        // Schedule run with scheduler
        self.scheduler
            .schedule_run(exe.id as u64, algo.cid, dataset_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to schedule run: {}", e)))?;

        Ok(())
    }

    /// Prepare dataset for algorithm execution
    async fn prepare_dataset(&self, dataset_name: &str) -> Result<PathBuf> {
        // For static datasets, construct the path
        let dataset_path = self.dataset_base_path.join(dataset_name);

        // Verify dataset exists
        if !dataset_path.exists() {
            // Try to find the dataset in the database
            let dataset = crate::models::static_dataset::StaticDataset::find_by_name(
                &self.db.pool,
                dataset_name,
            )
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Dataset {} not found", dataset_name)))?;

            // Check if dataset has a file path
            if let Some(file_path) = dataset.file_path {
                let path = PathBuf::from(file_path);
                if path.exists() {
                    info!("Using dataset file path from database: {:?}", path);
                    return Ok(path);
                }
            }

            return Err(AppError::NotFound(format!(
                "Dataset {} file not found at expected location",
                dataset_name
            )));
        }

        info!("Prepared dataset path: {:?}", dataset_path);
        Ok(dataset_path)
    }

    /// Fetch and extract algorithm from IPFS
    async fn fetch_and_extract_algorithm(&self, cid: &str) -> Result<PathBuf> {
        info!("Fetching algorithm from IPFS: {}", cid);

        // Create temporary directory
        let temp_dir = TempDir::new()
            .map_err(|e| AppError::Internal(format!("Failed to create temp dir: {}", e)))?;
        let work_dir = temp_dir.path().to_path_buf();

        // Download from IPFS
        let url = format!("{}/ipfs/{}", self.ipfs_gateway, cid);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to download from IPFS: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "Failed to download from IPFS: {}",
                response.status()
            )));
        }

        // Save to temporary file
        let temp_file = work_dir.join("algorithm.tar.gz");
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get response bytes: {}", e)))?;
        fs::write(&temp_file, bytes)
            .map_err(|e| AppError::Internal(format!("Failed to write temp file: {}", e)))?;

        // Extract tar.gz
        let tar_gz = File::open(&temp_file)
            .map_err(|e| AppError::Internal(format!("Failed to open temp file: {}", e)))?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        archive
            .unpack(&work_dir)
            .map_err(|e| AppError::Internal(format!("Failed to unpack tar archive: {}", e)))?;

        // Remove the tar.gz file
        fs::remove_file(&temp_file)
            .map_err(|e| AppError::Internal(format!("Failed to remove temp file: {}", e)))?;

        // Verify Dockerfile exists
        let dockerfile_path = work_dir.join("Dockerfile");
        if !dockerfile_path.exists() {
            return Err(AppError::Validation(
                "Dockerfile not found in algorithm package".to_string(),
            ));
        }

        info!("Extracted algorithm to: {:?}", work_dir);

        // Keep the temp_dir alive by storing it in a static variable
        // This prevents the directory from being cleaned up
        // In production, we should manage this more carefully
        Box::leak(Box::new(temp_dir));

        Ok(work_dir)
    }

    /// Recover pending executions after restart
    async fn recover_pending_executions(&self) -> Result<()> {
        info!("Recovering pending executions");

        // Find executions that were RUNNING
        let running_exes = AlgoExe::find_by_status(&self.db.pool, ExecutionStatus::Running).await?;
        let count = running_exes.len();

        for exe in running_exes {
            info!("Recovering execution {}", exe.id);

            // Reset to QUEUED so it will be picked up again
            AlgoExe::update_status(&self.db.pool, exe.id, ExecutionStatus::Queued).await?;
        }

        info!("Recovered {} executions", count);
        Ok(())
    }
}

/// Start the runtime worker
pub async fn start_runtime_worker(
    db: Database,
    config: Config,
    contract_caller: Arc<ContractCaller>,
    ws_hub: Arc<crate::infra::ws::Hub>,
) -> Result<()> {
    let runtime_service = Arc::new(RuntimeService::new(db, config, contract_caller, ws_hub)?);

    // Initialize the service
    runtime_service.init().await?;

    // Start the worker loop
    runtime_service.start().await
}
