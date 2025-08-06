use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

use super::hub::Hub;

use crate::models::blockchain_transaction::BlockchainTransaction;

/// Business codes for responses
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BizCode {
    Success = 0,
    InvalidParams = 10001,
    InternalError = 10002,
    NotFound = 10003,
    Unauthorized = 10004,
    TxFailed = 20001,
    TxPending = 20002,
}

/// Standard response format
#[derive(Debug, Serialize, Deserialize)]
pub struct Response<T> {
    pub code: BizCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> Response<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: BizCode::Success,
            data: Some(data),
            message: None,
        }
    }

    pub fn error(code: BizCode, message: String) -> Self {
        Self {
            code,
            data: None,
            message: Some(message),
        }
    }
}

/// WebSocket Notifier for pushing notifications to connected clients
pub struct Notifier {
    hub: Arc<Hub>,
}

impl Notifier {
    /// Create a new Notifier instance
    pub fn new(hub: Arc<Hub>) -> Self {
        Self { hub }
    }

    /// Get the underlying Hub
    pub fn hub(&self) -> Arc<Hub> {
        Arc::clone(&self.hub)
    }

    /// Push an error notification for a transaction
    pub async fn push_error(&self, tx_hash: String, code: BizCode) {
        let response = Response::<()>::error(code, format!("Transaction failed: {:?}", code));

        if let Err(e) = self.hub.notify(tx_hash, response).await {
            error!("Failed to push error notification: {}", e);
        }
    }

    /// Push a transaction result notification
    pub async fn push_tx_result(&self, tx_hash: String, tx_result: &BlockchainTransaction) {
        let response = Response::success(tx_result);

        if let Err(e) = self.hub.notify(tx_hash, response).await {
            error!("Failed to push transaction result: {}", e);
        }
    }

    /// Push a custom notification
    pub async fn push_notification<T: Serialize>(&self, task_id: String, data: T) {
        let response = Response::success(data);

        if let Err(e) = self.hub.notify(task_id, response).await {
            error!("Failed to push notification: {}", e);
        }
    }

    /// Push a status update
    pub async fn push_status(&self, task_id: String, status: &str) {
        #[derive(Serialize)]
        struct StatusUpdate {
            r#type: String,
            status: String,
            task_id: String,
        }

        let update = StatusUpdate {
            r#type: "status_update".to_string(),
            status: status.to_string(),
            task_id: task_id.clone(),
        };

        let response = Response::success(update);

        if let Err(e) = self.hub.notify(task_id, response).await {
            error!("Failed to push status update: {}", e);
        }
    }
}

impl Clone for Notifier {
    fn clone(&self) -> Self {
        Self {
            hub: Arc::clone(&self.hub),
        }
    }
}
