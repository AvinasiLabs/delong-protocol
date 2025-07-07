use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio::sync::RwLock;
use tracing::{info, warn, error, instrument};

use common::ApiError;
use crate::config::SecureConfig;
use crate::runtime::execution_queue::{ExecutionQueue, ExecutionRequest, ExecutionStatus};
use crate::runtime::executor::AlgorithmExecutor;
use crate::tee::KeyVault;

/// Execution scheduler that manages the algorithm execution lifecycle
pub struct ExecutionScheduler {
    config: SecureConfig,
    queue: Arc<ExecutionQueue>,
    executor: Arc<AlgorithmExecutor>,
    is_running: Arc<RwLock<bool>>,
}

impl ExecutionScheduler {
    /// Create a new execution scheduler
    pub fn new(
        config: SecureConfig,
        max_concurrent_executions: usize,
        key_vault: KeyVault,
        ipfs_client: ipfs_api_backend_hyper::IpfsClient,
    ) -> Self {
        let queue = Arc::new(ExecutionQueue::new(max_concurrent_executions));
        let executor = Arc::new(AlgorithmExecutor::new(config.clone(), key_vault, ipfs_client));

        Self {
            config,
            queue,
            executor,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the execution scheduler
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<(), ApiError> {
        info!("Starting algorithm execution scheduler");

        // Set running flag
        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }

        // Start the main scheduling loop
        let schedule_interval = Duration::from_secs(5); // Check every 5 seconds
        let mut timer = interval(schedule_interval);

        loop {
            // Check if we should stop
            {
                let is_running = self.is_running.read().await;
                if !*is_running {
                    info!("Execution scheduler stopping");
                    break;
                }
            }

            timer.tick().await;

            // Process the execution queue
            if let Err(e) = self.process_queue().await {
                error!(error = %e, "Failed to process execution queue");
                // Continue running even on errors
            }

            // Monitor running executions
            if let Err(e) = self.monitor_executions().await {
                error!(error = %e, "Failed to monitor executions");
            }
        }

        Ok(())
    }

    /// Stop the execution scheduler
    pub async fn stop(&self) {
        info!("Stopping execution scheduler");
        let mut is_running = self.is_running.write().await;
        *is_running = false;
    }

    /// Check if the scheduler is currently running
    pub async fn is_running(&self) -> bool {
        let is_running = self.is_running.read().await;
        *is_running
    }

    /// Add a new execution request to the queue
    pub async fn submit_execution(&self, request: ExecutionRequest) -> Result<(), ApiError> {
        info!(
            execution_id = %request.execution_id,
            priority = %request.priority,
            "Submitting execution request to scheduler"
        );

        self.queue.enqueue(request).await
    }

    /// Cancel an execution request
    pub async fn cancel_execution(&self, execution_id: u64) -> Result<(), ApiError> {
        info!(execution_id = %execution_id, "Cancelling execution request");
        self.queue.cancel_execution(execution_id).await
    }

    /// Get queue statistics
    pub async fn get_queue_stats(&self) -> crate::runtime::execution_queue::QueueStats {
        self.queue.get_stats().await
    }

    /// Get all queued requests
    pub async fn get_queued_requests(&self) -> Vec<ExecutionRequest> {
        self.queue.get_queued_requests().await
    }

    /// Get all running requests
    pub async fn get_running_requests(&self) -> Vec<ExecutionRequest> {
        self.queue.get_running_requests().await
    }

    /// Process the execution queue and start new executions if possible
    async fn process_queue(&self) -> Result<(), ApiError> {
        // Check if we can start new executions
        while self.queue.can_start_execution().await {
            // Get the next request from the queue
            if let Some(request) = self.queue.dequeue().await {
                // Mark as running
                self.queue.start_execution(request.clone()).await?;

                // Start execution in background
                let executor = Arc::clone(&self.executor);
                let queue = Arc::clone(&self.queue);
                
                tokio::spawn(async move {
                    let execution_id = request.execution_id;
                    
                    // Execute the algorithm
                    let result = executor.execute(request).await;
                    
                    // Update queue with completion status
                    let status = match result {
                        Ok(exec_result) => {
                            info!(
                                execution_id = %execution_id,
                                status = ?exec_result.status,
                                duration = %exec_result.duration_seconds,
                                "Algorithm execution completed"
                            );
                            exec_result.status
                        }
                        Err(e) => {
                            error!(
                                execution_id = %execution_id,
                                error = %e,
                                "Algorithm execution failed"
                            );
                            ExecutionStatus::Failed
                        }
                    };

                    // Remove from running list
                    if let Err(e) = queue.complete_execution(execution_id, status).await {
                        error!(
                            execution_id = %execution_id,
                            error = %e,
                            "Failed to mark execution as completed"
                        );
                    }

                    // TODO: Update database with execution result
                    // TODO: Emit execution completion event
                });
            } else {
                // No more requests in queue
                break;
            }
        }

        Ok(())
    }

    /// Monitor running executions for timeouts or failures
    async fn monitor_executions(&self) -> Result<(), ApiError> {
        let running_requests = self.queue.get_running_requests().await;
        
        for request in running_requests {
            // Check for execution timeout (example: 30 minutes)
            let max_execution_time = Duration::from_secs(30 * 60);
            let elapsed = chrono::Utc::now().signed_duration_since(request.created_at);
            
            if elapsed.num_seconds() > max_execution_time.as_secs() as i64 {
                warn!(
                    execution_id = %request.execution_id,
                    elapsed_seconds = %elapsed.num_seconds(),
                    "Execution timeout detected, marking as failed"
                );

                // Mark as failed due to timeout
                self.queue.complete_execution(request.execution_id, ExecutionStatus::Failed).await?;
            }

            // Check for cancellation requests
            if request.status == ExecutionStatus::Cancelled {
                info!(
                    execution_id = %request.execution_id,
                    "Processing cancellation request"
                );

                // TODO: Send cancellation signal to running process
                self.queue.complete_execution(request.execution_id, ExecutionStatus::Cancelled).await?;
            }
        }

        Ok(())
    }

    /// Emergency shutdown - cancel all queued and running executions
    pub async fn emergency_shutdown(&self) -> Result<(), ApiError> {
        warn!("Emergency shutdown initiated - cancelling all executions");

        // Clear the queue
        let cleared_count = self.queue.clear_queue().await;
        info!(cleared_queued = %cleared_count, "Cleared queued executions");

        // Cancel running executions
        let running_requests = self.queue.get_running_requests().await;
        for request in running_requests {
            if let Err(e) = self.queue.complete_execution(request.execution_id, ExecutionStatus::Cancelled).await {
                error!(
                    execution_id = %request.execution_id,
                    error = %e,
                    "Failed to cancel running execution"
                );
            }
        }

        // Stop the scheduler
        self.stop().await;

        Ok(())
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        let queue_stats = self.queue.get_stats().await;
        
        ExecutionStats {
            queued_count: queue_stats.queued_count,
            running_count: queue_stats.running_count,
            max_concurrent: queue_stats.max_concurrent,
            available_slots: queue_stats.available_slots,
            total_completed: 0, // TODO: Track from database
            total_failed: 0,    // TODO: Track from database
            total_cancelled: 0, // TODO: Track from database
        }
    }
}

/// Execution statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionStats {
    pub queued_count: usize,
    pub running_count: usize,
    pub max_concurrent: usize,
    pub available_slots: usize,
    pub total_completed: u64,
    pub total_failed: u64,
    pub total_cancelled: u64,
} 