//! Verification code storage service using Redis
//!
//! This service handles the storage and validation of verification codes
//! for email verification using Redis as the backing store.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{AppError, config::VerificationConfig};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

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

/// Verification statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStats {
    pub total_codes: usize,
    pub active_codes: usize,
    pub expired_codes: usize,
    pub total_attempts: usize,
}

/// Verification store using Redis
#[derive(Clone)]
pub struct VerificationStore {
    redis_pool: Arc<deadpool_redis::Pool>,
    config: VerificationConfig,
}

impl VerificationStore {
    /// Create a new verification store with configuration
    pub fn new(redis_pool: Arc<deadpool_redis::Pool>, config: VerificationConfig) -> Self {
        Self { redis_pool, config }
    }

    /// Generate Redis key for verification
    fn verification_key(email: &str) -> String {
        format!("verification:{}", email)
    }

    /// Generate a random 6-digit verification code
    fn generate_code() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let code = rng.gen_range(100000..999999);
        code.to_string()
    }

    /// Get current timestamp in seconds
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Store a verification code
    pub async fn store(&self, email: &str, language: &str) -> Result<String, AppError> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal("Redis connection failed".to_string())
        })?;

        // Generate verification code (use fixed code in dev/test mode)
        let code = if self.config.use_fixed_code {
            println!("DEBUG: Using fixed code: {}", self.config.fixed_code);
            self.config.fixed_code.clone()
        } else {
            let generated = Self::generate_code();
            debug!("DEBUG: Generated random code: {}", generated);
            generated
        };

        info!(
            "Storing verification code for email: {} (use_fixed_code: {}, code: {})",
            email, self.config.use_fixed_code, code
        );

        // Create verification entry
        let now = Self::current_timestamp();
        let entry = VerificationEntry {
            code: code.clone(),
            email: email.to_string(),
            created_at: now,
            expires_at: now + (self.config.expiration_minutes * 60),
            attempts: 0,
            language: language.to_string(),
        };

        // Serialize entry
        let entry_json = serde_json::to_string(&entry).map_err(|e| {
            error!("Failed to serialize verification entry: {}", e);
            AppError::Internal("Failed to store verification code".to_string())
        })?;

        // Store in Redis with expiration
        let key = Self::verification_key(email);
        debug!("Storing to Redis key: {}", key);
        use deadpool_redis::redis::AsyncCommands;

        let expiration = self.config.expiration_minutes * 60;
        let _: () = conn
            .set_ex(&key, entry_json.clone(), expiration)
            .await
            .map_err(|e| {
                error!("Failed to store verification code in Redis: {}", e);
                AppError::Internal("Failed to store verification code".to_string())
            })?;

        debug!(
            "Successfully stored verification code for {}: {} (expires in {} seconds, entry: {})",
            email, code, expiration, entry_json
        );
        debug!(
            "Verification code stored for {}: {} (expires in {} minutes)",
            email, code, self.config.expiration_minutes
        );

        Ok(code)
    }

    /// Verify a code
    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AppError> {
        println!(
            "DEBUG: Verifying code for email: {}, provided code: {}",
            email, code
        );

        let mut conn = self.redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal("Redis connection failed".to_string())
        })?;

        let key = Self::verification_key(email);
        println!("DEBUG: Looking for Redis key: {}", key);
        use deadpool_redis::redis::AsyncCommands;

        // Get the verification entry
        let entry_json: Option<String> = conn.get(&key).await.map_err(|e| {
            error!("Failed to get verification code from Redis: {}", e);
            AppError::Internal("Failed to retrieve verification code".to_string())
        })?;

        println!("DEBUG: Retrieved from Redis: {:?}", entry_json);

        let entry_json = entry_json.ok_or_else(|| {
            warn!("Verification code not found for email: {}", email);
            println!("DEBUG: No entry found in Redis for key: {}", key);
            AppError::NotFound("Verification code not found or expired".to_string())
        })?;

        // Parse the entry
        let mut entry: VerificationEntry = serde_json::from_str(&entry_json).map_err(|e| {
            error!("Failed to parse verification entry: {}", e);
            AppError::Internal("Invalid verification data".to_string())
        })?;

        println!(
            "DEBUG: Parsed entry - code: {}, expires_at: {}, attempts: {}",
            entry.code, entry.expires_at, entry.attempts
        );

        // Check expiration
        let now = Self::current_timestamp();
        if now > entry.expires_at {
            warn!("Verification code expired for email: {}", email);
            // Delete expired entry
            let _: Option<String> = conn.del(&key).await.ok();
            return Err(AppError::Validation(
                "Verification code expired".to_string(),
            ));
        }

        // Check attempts
        if entry.attempts >= self.config.max_attempts {
            warn!("Max verification attempts exceeded for email: {}", email);
            // Delete after max attempts
            let _: Option<String> = conn.del(&key).await.ok();
            return Err(AppError::Validation(
                "Too many verification attempts".to_string(),
            ));
        }

        // Verify code
        println!(
            "DEBUG: Comparing codes - stored: '{}', provided: '{}'",
            entry.code, code
        );
        if entry.code != code {
            // Increment attempts
            entry.attempts += 1;
            println!(
                "DEBUG: Code mismatch! Incrementing attempts to {}",
                entry.attempts
            );
            let updated_json = serde_json::to_string(&entry).map_err(|e| {
                error!("Failed to serialize updated entry: {}", e);
                AppError::Internal("Failed to update verification attempts".to_string())
            })?;

            // Calculate remaining TTL
            let ttl = if entry.expires_at > now {
                (entry.expires_at - now) as usize
            } else {
                1
            };

            let _: () = conn
                .set_ex(&key, updated_json, ttl as u64)
                .await
                .map_err(|e| {
                    error!("Failed to update verification attempts in Redis: {}", e);
                    AppError::Internal("Failed to update verification attempts".to_string())
                })?;

            warn!(
                "Invalid verification code for email: {} (attempt {}/{})",
                email, entry.attempts, self.config.max_attempts
            );
            return Err(AppError::Validation(
                "Invalid verification code".to_string(),
            ));
        }

        // Verification successful - delete the entry
        let _: Option<String> = conn.del(&key).await.ok();

        info!("Verification successful for email: {}", email);
        Ok(())
    }

    /// Clear verification code for an email
    pub async fn clear(&self, email: &str) -> Result<(), AppError> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal("Redis connection failed".to_string())
        })?;

        let key = Self::verification_key(email);
        use deadpool_redis::redis::AsyncCommands;

        let _: Option<String> = conn.del(&key).await.ok();
        debug!("Cleared verification code for email: {}", email);
        Ok(())
    }

    /// Check if a verification code exists for an email
    pub async fn exists(&self, email: &str) -> Result<bool, AppError> {
        // In test environment with fixed code, always return false to skip conflict check
        if self.config.use_fixed_code {
            debug!("Test environment detected, skipping verification code existence check");
            return Ok(false);
        }

        let mut conn = self.redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal("Redis connection failed".to_string())
        })?;

        let key = Self::verification_key(email);
        use deadpool_redis::redis::AsyncCommands;

        let exists: bool = conn.exists(&key).await.map_err(|e| {
            error!("Failed to check existence in Redis: {}", e);
            AppError::Internal("Failed to check verification code".to_string())
        })?;

        Ok(exists)
    }

    /// Get verification statistics
    pub async fn get_stats(&self) -> Result<VerificationStats, AppError> {
        let mut conn = self.redis_pool.get().await.map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            AppError::Internal("Redis connection failed".to_string())
        })?;

        use deadpool_redis::redis::AsyncCommands;

        // Get all verification keys
        let keys: Vec<String> = conn.keys("verification:*").await.map_err(|e| {
            error!("Failed to get keys from Redis: {}", e);
            AppError::Internal("Failed to get verification stats".to_string())
        })?;

        let total_codes = keys.len();
        let mut active_codes = 0;
        let mut expired_codes = 0;
        let mut total_attempts = 0;

        let now = Self::current_timestamp();

        for key in keys {
            if let Ok(Some(entry_json)) = conn.get::<_, Option<String>>(&key).await {
                if let Ok(entry) = serde_json::from_str::<VerificationEntry>(&entry_json) {
                    if now <= entry.expires_at {
                        active_codes += 1;
                    } else {
                        expired_codes += 1;
                    }
                    total_attempts += entry.attempts as usize;
                }
            }
        }

        Ok(VerificationStats {
            total_codes,
            active_codes,
            expired_codes,
            total_attempts,
        })
    }
}
