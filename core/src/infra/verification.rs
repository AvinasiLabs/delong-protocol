//! Verification code storage service
//!
//! This service handles the storage and validation of verification codes
//! for email verification during user registration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppError;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Verification code entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEntry {
    pub code: String,
    pub email: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub attempts: u32,
    pub language: String,
}

/// Verification service configuration
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    /// Code expiration time in minutes
    pub expiration_minutes: u64,
    /// Maximum verification attempts
    pub max_attempts: u32,
    /// Rate limit window in minutes
    pub rate_limit_window_minutes: u64,
    /// Development mode - uses fixed code
    pub dev_mode: bool,
    /// Fixed code for development
    pub dev_code: String,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            expiration_minutes: 15,
            max_attempts: 5,
            rate_limit_window_minutes: 2,
            dev_mode: std::env::var("RUST_ENV").unwrap_or_default() != "production" || cfg!(test),
            dev_code: "1234".to_string(),
        }
    }
}

/// In-memory verification store
#[derive(Debug)]
pub struct VerificationStore {
    entries: Arc<RwLock<HashMap<String, VerificationEntry>>>,
    config: VerificationConfig,
}

/// Helper function to get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Helper function to check if timestamp is expired
fn is_expired(expires_at: u64) -> bool {
    current_timestamp() > expires_at
}

impl VerificationStore {
    /// Create a new verification store with default configuration
    pub fn new() -> Self {
        Self::new_with_config(VerificationConfig::default())
    }

    /// Create a new verification store with custom configuration
    pub fn new_with_config(config: VerificationConfig) -> Self {
        let store = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        // Start cleanup task
        store.start_cleanup_task();

        store
    }

    /// Generate a random verification code
    pub fn generate_code(&self) -> String {
        if self.config.dev_mode {
            return self.config.dev_code.clone();
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!("{:06}", rng.gen_range(100000..999999))
    }

    /// Store a verification code
    pub async fn store_code(
        &self,
        email: &str,
        code: &str,
        language: Option<&str>,
    ) -> Result<(), AppError> {
        let email = email.to_lowercase();

        // Check rate limiting
        if self.is_rate_limited(&email).await {
            warn!("Rate limit exceeded for email: {}", email);
            return Err(AppError::Conflict("Email already sent".to_string()));
        }

        let now = current_timestamp();
        let expires_at = now + (self.config.expiration_minutes * 60);

        let entry = VerificationEntry {
            code: code.to_string(),
            email: email.clone(),
            created_at: now,
            expires_at,
            attempts: 0,
            language: language.unwrap_or("en").to_string(),
        };

        let mut entries = self.entries.write().await;
        entries.insert(email.clone(), entry);

        info!("Stored verification code for email: {}", email);
        Ok(())
    }

    /// Verify a code
    pub async fn verify_code(&self, email: &str, code: &str) -> Result<bool, AppError> {
        let email = email.to_lowercase();

        // Dev mode: accept the dev_code for any email
        if self.config.dev_mode && code == self.config.dev_code {
            info!("Dev mode: accepting verification code for email: {}", email);
            return Ok(true);
        }

        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(&email) {
            // Check if code is expired
            if is_expired(entry.expires_at) {
                debug!("Verification code expired for email: {}", email);
                entries.remove(&email);
                return Ok(false);
            }

            // Increment attempts
            entry.attempts += 1;

            // Check max attempts
            if entry.attempts > self.config.max_attempts {
                warn!(
                    "Maximum verification attempts exceeded for email: {}",
                    email
                );
                entries.remove(&email);
                return Err(AppError::Validation(
                    "Invalid verification code".to_string(),
                ));
            }

            // Check code
            if entry.code == code {
                info!("Verification code verified for email: {}", email);
                entries.remove(&email);
                return Ok(true);
            } else {
                debug!("Invalid verification code for email: {}", email);
                return Ok(false);
            }
        }

        debug!("No verification code found for email: {}", email);
        Ok(false)
    }

    /// Remove a verification code
    pub async fn remove_code(&self, email: &str) {
        let email = email.to_lowercase();
        let mut entries = self.entries.write().await;
        if entries.remove(&email).is_some() {
            debug!("Removed verification code for email: {}", email);
        }
    }

    /// Get stored code (for development/testing)
    pub async fn get_code(&self, email: &str) -> Option<String> {
        let email = email.to_lowercase();
        let entries = self.entries.read().await;

        if let Some(entry) = entries.get(&email) {
            if !is_expired(entry.expires_at) {
                return Some(entry.code.clone());
            }
        }

        None
    }

    /// Check if email is rate limited
    async fn is_rate_limited(&self, email: &str) -> bool {
        let entries = self.entries.read().await;

        if let Some(entry) = entries.get(email) {
            let rate_limit_window = self.config.rate_limit_window_minutes * 60;
            return current_timestamp() - entry.created_at < rate_limit_window;
        }

        false
    }

    /// Clean up expired entries
    pub async fn cleanup_expired(&self) {
        let mut entries = self.entries.write().await;

        let initial_count = entries.len();
        entries.retain(|_, entry| !is_expired(entry.expires_at));
        let final_count = entries.len();

        if initial_count != final_count {
            debug!(
                "Cleaned up {} expired verification codes",
                initial_count - final_count
            );
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let entries = self.entries.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                let mut entries_guard = entries.write().await;

                let initial_count = entries_guard.len();
                entries_guard.retain(|_, entry| !is_expired(entry.expires_at));
                let final_count = entries_guard.len();

                if initial_count != final_count {
                    debug!(
                        "Cleanup task removed {} expired verification codes",
                        initial_count - final_count
                    );
                }
            }
        });
    }

    /// Get statistics
    pub async fn get_stats(&self) -> VerificationStats {
        let entries = self.entries.read().await;

        let mut active_count = 0;
        let mut expired_count = 0;

        for entry in entries.values() {
            if !is_expired(entry.expires_at) {
                active_count += 1;
            } else {
                expired_count += 1;
            }
        }

        VerificationStats {
            active_codes: active_count,
            expired_codes: expired_count,
            total_codes: entries.len(),
        }
    }
}

/// Verification statistics
#[derive(Debug, Serialize)]
pub struct VerificationStats {
    pub active_codes: usize,
    pub expired_codes: usize,
    pub total_codes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_store_and_verify_code() {
        let config = VerificationConfig::default();
        let store = VerificationStore::new_with_config(config);

        let email = "test@example.com";
        let code = "123456";

        store.store_code(email, code, None).await.unwrap();

        let result = store.verify_code(email, code).await.unwrap();
        assert!(result);

        // Code should be removed after verification
        let result2 = store.verify_code(email, code).await.unwrap();
        assert!(!result2);
    }

    #[tokio::test]
    async fn test_code_expiration() {
        let mut config = VerificationConfig::default();
        config.expiration_minutes = 0; // Expire immediately

        let store = VerificationStore::new_with_config(config);

        let email = "test@example.com";
        let code = "123456";

        store.store_code(email, code, None).await.unwrap();

        // Wait a moment for expiration
        sleep(Duration::from_millis(100)).await;

        let result = store.verify_code(email, code).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = VerificationConfig::default();
        let store = VerificationStore::new_with_config(config);

        let email = "test@example.com";
        let code = "123456";

        store.store_code(email, code, None).await.unwrap();

        // Second attempt should be rate limited
        let result = store.store_code(email, code, None).await;
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), AppError::Conflict(msg) if msg == "Email already sent")
        );
    }

    #[tokio::test]
    async fn test_max_attempts() {
        let mut config = VerificationConfig::default();
        config.max_attempts = 2;

        let store = VerificationStore::new_with_config(config);

        let email = "test@example.com";
        let code = "123456";

        store.store_code(email, code, None).await.unwrap();

        // First wrong attempt
        let result = store.verify_code(email, "wrong").await.unwrap();
        assert!(!result);

        // Second wrong attempt
        let result = store.verify_code(email, "wrong").await.unwrap();
        assert!(!result);

        // Third attempt should fail with error
        let result = store.verify_code(email, "wrong").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::Validation(_)));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let mut config = VerificationConfig::default();
        config.expiration_minutes = 0;

        let store = VerificationStore::new_with_config(config);

        let email = "test@example.com";
        let code = "123456";

        store.store_code(email, code, None).await.unwrap();

        // Wait for expiration
        sleep(Duration::from_millis(100)).await;

        store.cleanup_expired().await;

        let stats = store.get_stats().await;
        assert_eq!(stats.active_codes, 0);
    }

    #[tokio::test]
    async fn test_dev_mode() {
        let mut config = VerificationConfig::default();
        config.dev_mode = true;
        config.dev_code = "999999".to_string();

        let store = VerificationStore::new_with_config(config);

        let generated_code = store.generate_code();
        assert_eq!(generated_code, "999999");
    }
}
