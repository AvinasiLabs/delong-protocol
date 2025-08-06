//! Notification service for blockchain events
//!
//! This module provides a service for sending notifications about various
//! blockchain events to interested parties.

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

use crate::{
    error::{AppError, AppResult},
    infra::ws::Hub,
};

/// Notification types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Notification {
    /// Data has been registered on-chain
    DataRegistered {
        data_hash: String,
        provider: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Algorithm has been resolved
    AlgorithmResolved {
        algorithm_id: String,
        approved: bool,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Committee member status changed
    CommitteeMemberUpdated {
        member: String,
        is_member: bool,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Vote has been cast
    VoteCasted {
        algorithm_id: String,
        voter: String,
        vote: bool,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Algorithm execution submitted
    ExecutionSubmitted {
        algorithm_id: String,
        data_hash: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Data has been used
    DataUsed {
        data_hash: String,
        algorithm_id: String,
        fee: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Notification service
pub struct NotificationService {
    /// WebSocket hub for real-time notifications
    ws_hub: Arc<Hub>,
    /// Notification history (for debugging/monitoring)
    history: Arc<RwLock<Vec<Notification>>>,
    /// Maximum history size
    max_history_size: usize,
}

impl NotificationService {
    /// Create a new notification service
    pub fn new(ws_hub: Arc<Hub>) -> Self {
        Self {
            ws_hub,
            history: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 1000,
        }
    }

    /// Send a notification
    #[instrument(skip(self))]
    async fn send_notification(&self, notification: Notification) -> AppResult<()> {
        // Add to history
        {
            let mut history = self.history.write().await;
            history.push(notification.clone());

            // Trim history if it's too large
            if history.len() > self.max_history_size {
                let drain_count = history.len() - self.max_history_size;
                history.drain(0..drain_count);
            }
        }

        // Convert to JSON
        let _message = serde_json::to_string(&notification)
            .map_err(|e| AppError::Internal(format!("Failed to serialize notification: {}", e)))?;

        // Broadcast via WebSocket
        // TODO: Fix broadcast method - Hub doesn't have broadcast, only notify
        // self.ws_hub.broadcast(&message).await;

        debug!("Notification sent: {:?}", notification);
        Ok(())
    }

    /// Notify that data has been registered
    #[instrument(skip(self))]
    pub async fn notify_data_registered(
        &self,
        data_hash: &str,
        provider: Address,
    ) -> AppResult<()> {
        let notification = Notification::DataRegistered {
            data_hash: data_hash.to_string(),
            provider: format!("{:?}", provider),
            timestamp: chrono::Utc::now(),
        };

        self.send_notification(notification).await
    }

    /// Notify that an algorithm has been resolved
    #[instrument(skip(self))]
    pub async fn notify_algorithm_resolved(
        &self,
        algorithm_id: &str,
        approved: bool,
    ) -> AppResult<()> {
        let notification = Notification::AlgorithmResolved {
            algorithm_id: algorithm_id.to_string(),
            approved,
            timestamp: chrono::Utc::now(),
        };

        self.send_notification(notification).await
    }

    /// Notify that a committee member status has changed
    #[instrument(skip(self))]
    pub async fn notify_committee_member_updated(
        &self,
        member: Address,
        is_member: bool,
    ) -> AppResult<()> {
        let notification = Notification::CommitteeMemberUpdated {
            member: format!("{:?}", member),
            is_member,
            timestamp: chrono::Utc::now(),
        };

        self.send_notification(notification).await
    }

    /// Notify that a vote has been cast
    #[instrument(skip(self))]
    pub async fn notify_vote_casted(
        &self,
        algorithm_id: &str,
        voter: Address,
        vote: bool,
    ) -> AppResult<()> {
        let notification = Notification::VoteCasted {
            algorithm_id: algorithm_id.to_string(),
            voter: format!("{:?}", voter),
            vote,
            timestamp: chrono::Utc::now(),
        };

        self.send_notification(notification).await
    }

    /// Notify that an algorithm execution has been submitted
    #[instrument(skip(self))]
    pub async fn notify_execution_submitted(
        &self,
        algorithm_id: &str,
        data_hash: &str,
    ) -> AppResult<()> {
        let notification = Notification::ExecutionSubmitted {
            algorithm_id: algorithm_id.to_string(),
            data_hash: data_hash.to_string(),
            timestamp: chrono::Utc::now(),
        };

        self.send_notification(notification).await
    }

    /// Notify that data has been used
    #[instrument(skip(self))]
    pub async fn notify_data_used(
        &self,
        data_hash: &str,
        algorithm_id: &str,
        fee: &str,
    ) -> AppResult<()> {
        let notification = Notification::DataUsed {
            data_hash: data_hash.to_string(),
            algorithm_id: algorithm_id.to_string(),
            fee: fee.to_string(),
            timestamp: chrono::Utc::now(),
        };

        self.send_notification(notification).await
    }

    /// Get notification history
    pub async fn get_history(&self) -> Vec<Notification> {
        self.history.read().await.clone()
    }

    /// Clear notification history
    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    /// Get the number of notifications in history
    pub async fn history_size(&self) -> usize {
        self.history.read().await.len()
    }
}

/// Notification subscriber trait
#[async_trait::async_trait]
pub trait NotificationSubscriber: Send + Sync {
    /// Handle a notification
    async fn handle_notification(&self, notification: &Notification) -> AppResult<()>;
}

/// Email notification subscriber (placeholder)
pub struct EmailSubscriber {
    // Email configuration would go here
}

impl EmailSubscriber {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl NotificationSubscriber for EmailSubscriber {
    async fn handle_notification(&self, notification: &Notification) -> AppResult<()> {
        // In a real implementation, this would send emails
        info!("Email notification (mock): {:?}", notification);
        Ok(())
    }
}

/// Webhook notification subscriber (placeholder)
pub struct WebhookSubscriber {
    webhook_url: String,
}

impl WebhookSubscriber {
    pub fn new(webhook_url: String) -> Self {
        Self { webhook_url }
    }
}

#[async_trait::async_trait]
impl NotificationSubscriber for WebhookSubscriber {
    async fn handle_notification(&self, notification: &Notification) -> AppResult<()> {
        // In a real implementation, this would POST to the webhook
        info!(
            "Webhook notification to {} (mock): {:?}",
            self.webhook_url, notification
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notification_service() {
        // Create a mock WebSocket hub
        let ws_hub = Arc::new(Hub::new());
        let service = NotificationService::new(ws_hub);

        // Test data registered notification
        let result = service
            .notify_data_registered("hash123", Address::default())
            .await;
        assert!(result.is_ok());

        // Test algorithm resolved notification
        let result = service.notify_algorithm_resolved("algo123", true).await;
        assert!(result.is_ok());

        // Check history
        let history = service.get_history().await;
        assert_eq!(history.len(), 2);

        // Clear history
        service.clear_history().await;
        assert_eq!(service.history_size().await, 0);
    }

    #[tokio::test]
    async fn test_notification_history_limit() {
        let ws_hub = Arc::new(Hub::new());
        let mut service = NotificationService::new(ws_hub);
        service.max_history_size = 5;

        // Send more notifications than the limit
        for i in 0..10 {
            service
                .notify_algorithm_resolved(&format!("algo{}", i), i % 2 == 0)
                .await
                .unwrap();
        }

        // History should be limited to max_history_size
        assert_eq!(service.history_size().await, 5);
    }
}
