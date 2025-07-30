//! Algorithm execution scheduler
//!
//! This module provides a scheduling system for algorithm execution,
//! managing Docker containers and coordinating execution lifecycle events.

use async_trait::async_trait;
use bollard::{
    Docker,
    container::{
        Config, CreateContainerOptions, LogsOptions, RemoveContainerOptions, StartContainerOptions,
    },
    image::CreateImageOptions,
    models::HostConfig,
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, timeout};
use tracing::{error, info};

/// Errors that can occur during scheduling operations
#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("Docker error: {0}")]
    DockerError(#[from] bollard::errors::Error),

    #[error("Channel send error: {0}")]
    ChannelError(String),

    #[error("Execution timeout: {0}")]
    Timeout(String),

    #[error("Container error: {0}")]
    ContainerError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Build context too large: {size} bytes exceeds limit of {limit} bytes")]
    BuildSizeLimitExceeded { size: u64, limit: u64 },
}

/// Result type for scheduler operations
pub type Result<T> = std::result::Result<T, SchedulerError>;

/// Event types for the scheduler
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    /// Algorithm CID resolved and ready to execute
    Resolve {
        exe_id: u64,
        algo_cid: String,
        resolved_at: DateTime<Utc>,
    },
    /// Start algorithm execution
    Run {
        exe_id: u64,
        algo_cid: String,
        dataset_path: PathBuf,
    },
    /// Clean up after execution
    Cleanup { exe_id: u64 },
}

/// Handler trait for scheduler events
#[async_trait]
pub trait SchedulerHandler: Send + Sync {
    /// Called when an algorithm CID is resolved
    async fn on_resolve(&self, exe_id: u64, algo_cid: &str, resolved_at: DateTime<Utc>);

    /// Called when algorithm execution starts
    async fn on_run(&self, exe_id: u64);

    /// Called when algorithm execution completes
    async fn on_completed(&self, exe_id: u64, success: bool, output: Vec<u8>, error: Vec<u8>);

    /// Called when an error occurs
    async fn on_error(&self, exe_id: u64, error: SchedulerError);
}

/// Configuration for the algorithm scheduler
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Channel buffer size
    pub channel_size: usize,
    /// Build context size limit in bytes
    pub build_size_limit: u64,
    /// Execution timeout in seconds
    pub execution_timeout: u64,
    /// Docker image to use for execution
    pub runtime_image: String,
    /// Working directory for algorithm execution
    pub working_directory: PathBuf,
    /// Maximum concurrent executions
    pub max_concurrent: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            channel_size: 10,
            build_size_limit: 100 << 20, // 100MB
            execution_timeout: 3600,     // 1 hour
            runtime_image: "python:3.9-slim".to_string(),
            working_directory: PathBuf::from("/tmp/delong-algo"),
            max_concurrent: 5,
        }
    }
}

/// Algorithm scheduler that manages execution lifecycle
pub struct AlgoScheduler {
    event_tx: mpsc::Sender<SchedulerEvent>,
    event_rx: Arc<Mutex<mpsc::Receiver<SchedulerEvent>>>,
    handler: Arc<dyn SchedulerHandler>,
    docker: Docker,
    config: SchedulerConfig,
    active_containers: Arc<Mutex<HashMap<u64, String>>>,
}

impl AlgoScheduler {
    /// Create a new algorithm scheduler
    pub fn new(handler: Arc<dyn SchedulerHandler>, config: SchedulerConfig) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        let (event_tx, event_rx) = mpsc::channel(config.channel_size);

        Ok(Self {
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            handler,
            docker,
            config,
            active_containers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Start the scheduler event processing loop
    pub async fn start(self: Arc<Self>) {
        info!("Starting algorithm scheduler");

        let scheduler = self.clone();
        tokio::spawn(async move {
            scheduler.process_events().await;
        });
    }

    /// Process events from the channel
    async fn process_events(&self) {
        let mut rx = self.event_rx.lock().await;

        while let Some(event) = rx.recv().await {
            match event {
                SchedulerEvent::Resolve {
                    exe_id,
                    algo_cid,
                    resolved_at,
                } => {
                    self.handler
                        .on_resolve(exe_id, &algo_cid, resolved_at)
                        .await;
                }
                SchedulerEvent::Run {
                    exe_id,
                    algo_cid,
                    dataset_path,
                } => {
                    self.handler.on_run(exe_id).await;
                    if let Err(e) = self.run_algorithm(exe_id, &algo_cid, &dataset_path).await {
                        error!("Failed to run algorithm {}: {}", exe_id, e);
                        self.handler.on_error(exe_id, e).await;
                    }
                }
                SchedulerEvent::Cleanup { exe_id } => {
                    if let Err(e) = self.cleanup_container(exe_id).await {
                        error!("Failed to cleanup container for {}: {}", exe_id, e);
                    }
                }
            }
        }
    }

    /// Schedule an algorithm resolution event
    pub async fn schedule_resolve(
        &self,
        exe_id: u64,
        algo_cid: String,
        resolved_at: DateTime<Utc>,
    ) -> Result<()> {
        self.event_tx
            .send(SchedulerEvent::Resolve {
                exe_id,
                algo_cid,
                resolved_at,
            })
            .await
            .map_err(|e| SchedulerError::ChannelError(e.to_string()))
    }

    /// Schedule an algorithm run event
    pub async fn schedule_run(
        &self,
        exe_id: u64,
        algo_cid: String,
        dataset_path: PathBuf,
    ) -> Result<()> {
        self.event_tx
            .send(SchedulerEvent::Run {
                exe_id,
                algo_cid,
                dataset_path,
            })
            .await
            .map_err(|e| SchedulerError::ChannelError(e.to_string()))
    }

    /// Run an algorithm in a Docker container
    async fn run_algorithm(
        &self,
        exe_id: u64,
        algo_cid: &str,
        dataset_path: &PathBuf,
    ) -> Result<()> {
        info!("Running algorithm {} with CID {}", exe_id, algo_cid);

        // Create container name
        let container_name = format!("delong-algo-{}", exe_id);

        // Pull the runtime image if needed
        self.pull_image(&self.config.runtime_image).await?;

        // Create container configuration
        let config = Config {
            image: Some(self.config.runtime_image.clone()),
            cmd: Some(vec!["python".to_string(), "/workspace/main.py".to_string()]),
            host_config: Some(HostConfig {
                binds: Some(vec![
                    format!("{}:/dataset:ro", dataset_path.display()),
                    format!("{}:/workspace:rw", self.config.working_directory.display()),
                ]),
                memory: Some(512 * 1024 * 1024), // 512MB
                cpu_quota: Some(100000),         // 1 CPU
                ..Default::default()
            }),
            working_dir: Some("/workspace".to_string()),
            ..Default::default()
        };

        // Create and start container
        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            ..Default::default()
        };

        let container_info = self
            .docker
            .create_container(Some(create_options), config)
            .await?;
        let container_id = container_info.id;

        // Store container ID
        {
            let mut containers = self.active_containers.lock().await;
            containers.insert(exe_id, container_id.clone());
        }

        // Start container
        self.docker
            .start_container(&container_id, None::<StartContainerOptions<String>>)
            .await?;

        // Wait for container to complete with timeout
        let execution_result = timeout(
            Duration::from_secs(self.config.execution_timeout),
            self.wait_for_container(&container_id),
        )
        .await;

        match execution_result {
            Ok(Ok((exit_code, output, error))) => {
                let success = exit_code == 0;
                self.handler
                    .on_completed(exe_id, success, output, error)
                    .await;
            }
            Ok(Err(e)) => {
                error!("Container execution failed: {}", e);
                self.handler.on_error(exe_id, e).await;
            }
            Err(_) => {
                error!("Container execution timeout");
                let timeout_error = SchedulerError::Timeout(format!(
                    "Execution exceeded {} seconds",
                    self.config.execution_timeout
                ));
                self.handler.on_error(exe_id, timeout_error).await;

                // Stop the container
                let _ = self.docker.stop_container(&container_id, None).await;
            }
        }

        // Schedule cleanup
        let _ = self.event_tx.send(SchedulerEvent::Cleanup { exe_id }).await;

        Ok(())
    }

    /// Wait for container to complete and collect output
    async fn wait_for_container(&self, container_id: &str) -> Result<(i64, Vec<u8>, Vec<u8>)> {
        // Wait for container to exit
        let wait_stream = self.docker.wait_container::<String>(container_id, None);
        let exit_info =
            wait_stream.into_future().await.0.ok_or_else(|| {
                SchedulerError::ContainerError("Container wait failed".to_string())
            })??;

        let exit_code = exit_info.status_code;

        // Collect logs
        let log_options: LogsOptions<String> = LogsOptions {
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let mut log_stream = self.docker.logs(container_id, Some(log_options));
        while let Some(log_result) = log_stream.next().await {
            match log_result {
                Ok(log) => match log {
                    bollard::container::LogOutput::StdOut { message } => {
                        stdout.extend_from_slice(&message)
                    }
                    bollard::container::LogOutput::StdErr { message } => {
                        stderr.extend_from_slice(&message)
                    }
                    _ => {}
                },
                Err(e) => tracing::warn!("Error reading logs: {}", e),
            }
        }

        Ok((exit_code, stdout, stderr))
    }

    /// Pull Docker image if not present
    async fn pull_image(&self, image: &str) -> Result<()> {
        info!("Pulling Docker image: {}", image);

        let options = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };

        let mut stream = self.docker.create_image(Some(options), None, None);
        while let Some(info) = stream.next().await {
            match info {
                Ok(_) => {} // Progress info
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    /// Clean up container after execution
    async fn cleanup_container(&self, exe_id: u64) -> Result<()> {
        let container_id = {
            let mut containers = self.active_containers.lock().await;
            containers.remove(&exe_id)
        };

        if let Some(container_id) = container_id {
            info!(
                "Cleaning up container {} for execution {}",
                container_id, exe_id
            );

            let options = RemoveContainerOptions {
                force: true,
                ..Default::default()
            };

            self.docker
                .remove_container(&container_id, Some(options))
                .await?;
        }

        Ok(())
    }

    /// Get number of active containers
    pub async fn active_count(&self) -> usize {
        self.active_containers.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler;

    #[async_trait]
    impl SchedulerHandler for MockHandler {
        async fn on_resolve(&self, exe_id: u64, algo_cid: &str, resolved_at: DateTime<Utc>) {
            info!(
                "Resolved {} with CID {} at {}",
                exe_id, algo_cid, resolved_at
            );
        }

        async fn on_run(&self, exe_id: u64) {
            info!("Running execution {}", exe_id);
        }

        async fn on_completed(&self, exe_id: u64, success: bool, output: Vec<u8>, error: Vec<u8>) {
            info!(
                "Completed {} with success={}, output_len={}, error_len={}",
                exe_id,
                success,
                output.len(),
                error.len()
            );
        }

        async fn on_error(&self, exe_id: u64, error: SchedulerError) {
            error!("Execution {} failed: {}", exe_id, error);
        }
    }

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.channel_size, 10);
        assert_eq!(config.build_size_limit, 100 << 20);
        assert_eq!(config.execution_timeout, 3600);
    }
}
