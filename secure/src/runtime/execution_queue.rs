use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};


use common::ApiError;

/// Algorithm execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    /// Unique execution ID
    pub execution_id: u64,
    
    /// Algorithm IPFS CID
    pub algorithm_cid: String,
    
    /// Dataset identifier
    pub dataset_id: String,
    
    /// Scientist wallet address who submitted the request
    pub scientist_wallet: String,
    
    /// Priority level (higher number = higher priority)
    pub priority: u32,
    
    /// Request timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Current status
    pub status: ExecutionStatus,
    
    /// Optional execution parameters
    pub parameters: serde_json::Value,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    /// Queued for execution
    Queued,
    
    /// Currently running
    Running,
    
    /// Completed successfully
    Completed,
    
    /// Failed with error
    Failed,
    
    /// Cancelled by user or system
    Cancelled,
}

/// Algorithm execution queue for managing pending executions
pub struct ExecutionQueue {
    /// Priority queue of execution requests
    queue: Arc<Mutex<VecDeque<ExecutionRequest>>>,
    
    /// Currently running executions
    running: Arc<RwLock<Vec<ExecutionRequest>>>,
    
    /// Maximum number of concurrent executions
    max_concurrent: usize,
}

impl ExecutionQueue {
    /// Create a new execution queue
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            running: Arc::new(RwLock::new(Vec::new())),
            max_concurrent,
        }
    }

    /// Add a new execution request to the queue
    pub async fn enqueue(&self, mut request: ExecutionRequest) -> Result<(), ApiError> {
        request.status = ExecutionStatus::Queued;
        request.created_at = chrono::Utc::now();

        let mut queue = self.queue.lock().await;
        
        // Insert based on priority (higher priority first)
        let insert_pos = queue
            .iter()
            .position(|req| req.priority < request.priority)
            .unwrap_or(queue.len());
        
        queue.insert(insert_pos, request.clone());
        
        info!(
            execution_id = %request.execution_id,
            priority = %request.priority,
            queue_size = %queue.len(),
            "Enqueued algorithm execution request"
        );

        Ok(())
    }

    /// Get the next execution request from the queue
    pub async fn dequeue(&self) -> Option<ExecutionRequest> {
        let mut queue = self.queue.lock().await;
        let request = queue.pop_front();
        
        if let Some(ref req) = request {
            info!(
                execution_id = %req.execution_id,
                remaining_queue_size = %queue.len(),
                "Dequeued algorithm execution request"
            );
        }
        
        request
    }

    /// Check if we can start a new execution (within concurrent limit)
    pub async fn can_start_execution(&self) -> bool {
        let running = self.running.read().await;
        running.len() < self.max_concurrent
    }

    /// Mark an execution as started (move from queue to running)
    pub async fn start_execution(&self, mut request: ExecutionRequest) -> Result<(), ApiError> {
        request.status = ExecutionStatus::Running;
        
        let mut running = self.running.write().await;
        running.push(request.clone());
        
        info!(
            execution_id = %request.execution_id,
            running_count = %running.len(),
            "Started algorithm execution"
        );

        Ok(())
    }

    /// Mark an execution as completed (remove from running)
    pub async fn complete_execution(&self, execution_id: u64, status: ExecutionStatus) -> Result<(), ApiError> {
        let mut running = self.running.write().await;
        
        if let Some(pos) = running.iter().position(|req| req.execution_id == execution_id) {
            let mut request = running.remove(pos);
            request.status = status.clone();
            
            info!(
                execution_id = %execution_id,
                status = ?status,
                remaining_running = %running.len(),
                "Completed algorithm execution"
            );
            
            Ok(())
        } else {
            error!(execution_id = %execution_id, "Execution not found in running list");
            Err(ApiError::NotFound(format!("Execution {} not found", execution_id)))
        }
    }

    /// Get current queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        let queue = self.queue.lock().await;
        let running = self.running.read().await;
        
        QueueStats {
            queued_count: queue.len(),
            running_count: running.len(),
            max_concurrent: self.max_concurrent,
            available_slots: self.max_concurrent.saturating_sub(running.len()),
        }
    }

    /// Get all queued requests (for monitoring)
    pub async fn get_queued_requests(&self) -> Vec<ExecutionRequest> {
        let queue = self.queue.lock().await;
        queue.iter().cloned().collect()
    }

    /// Get all running requests (for monitoring)
    pub async fn get_running_requests(&self) -> Vec<ExecutionRequest> {
        let running = self.running.read().await;
        running.clone()
    }

    /// Cancel a queued execution
    pub async fn cancel_execution(&self, execution_id: u64) -> Result<(), ApiError> {
        // Try to remove from queue first
        {
            let mut queue = self.queue.lock().await;
            if let Some(pos) = queue.iter().position(|req| req.execution_id == execution_id) {
                let mut request = queue.remove(pos).unwrap();
                request.status = ExecutionStatus::Cancelled;
                
                info!(execution_id = %execution_id, "Cancelled queued execution");
                return Ok(());
            }
        }

        // Try to cancel running execution
        {
            let mut running = self.running.write().await;
            if let Some(request) = running.iter_mut().find(|req| req.execution_id == execution_id) {
                request.status = ExecutionStatus::Cancelled;
                
                warn!(execution_id = %execution_id, "Marked running execution for cancellation");
                return Ok(());
            }
        }

        Err(ApiError::NotFound(format!("Execution {} not found", execution_id)))
    }

    /// Clear all queued requests (emergency stop)
    pub async fn clear_queue(&self) -> usize {
        let mut queue = self.queue.lock().await;
        let count = queue.len();
        queue.clear();
        
        warn!(cleared_count = %count, "Cleared execution queue");
        count
    }
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub queued_count: usize,
    pub running_count: usize,
    pub max_concurrent: usize,
    pub available_slots: usize,
}

impl Default for ExecutionRequest {
    fn default() -> Self {
        Self {
            execution_id: 0,
            algorithm_cid: String::new(),
            dataset_id: String::new(),
            scientist_wallet: String::new(),
            priority: 0,
            created_at: chrono::Utc::now(),
            status: ExecutionStatus::Queued,
            parameters: serde_json::Value::Null,
        }
    }
} 