// Utility functions for the secure service
// TODO: Implement helper functions for cryptography, data processing, etc. 

use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use tracing::{info, warn, error};
use common::ApiResult;

// Sub-modules can be added as needed
// pub mod crypto;
// pub mod hash;
// pub mod validation;
// pub mod dataset;

/// Generate a unique request ID for tracing
pub fn generate_request_id() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

/// Hash data using SHA256
pub fn sha256_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Hash a string using SHA256
pub fn sha256_string(input: &str) -> String {
    sha256_hash(input.as_bytes())
}

/// Generate a dataset version ID based on timestamp
pub fn generate_dataset_version() -> String {
    let now = Utc::now();
    now.format("%Y%m%d_%H%M%S").to_string()
}

/// Validate dataset name format
pub fn is_valid_dataset_name(name: &str) -> bool {
    // Must be alphanumeric, underscore, hyphen, or start with __static__
    if name.starts_with("__static__") {
        let suffix = &name[10..];
        return !suffix.is_empty() && suffix.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    }
    
    !name.is_empty() && 
    name.len() <= 100 && 
    name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Check if a dataset is static (IPFS-based)
pub fn is_static_dataset(name: &str) -> bool {
    name.starts_with("__static__")
}

/// Extract static dataset name without prefix
pub fn extract_static_dataset_name(name: &str) -> Option<&str> {
    if is_static_dataset(name) {
        Some(&name[10..]) // Remove "__static__" prefix
    } else {
        None
    }
}

/// Validate algorithm CID format
pub fn is_valid_cid(cid: &str) -> bool {
    // Basic CID validation - starts with Qm and has reasonable length
    cid.starts_with("Qm") && cid.len() >= 44 && cid.len() <= 59 && 
    cid.chars().all(|c| c.is_alphanumeric())
}

/// Validate Ethereum address format
pub fn is_valid_ethereum_address(address: &str) -> bool {
    address.starts_with("0x") && 
    address.len() == 42 && 
    address[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub page: u32,
    pub limit: u32,
    pub offset: u32,
}

impl PaginationParams {
    pub fn new(page: Option<u32>, limit: Option<u32>) -> Self {
        let page = page.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;
        
        Self { page, limit, offset }
    }
    
    pub fn total_pages(total_items: u64, limit: u32) -> u32 {
        ((total_items + limit as u64 - 1) / limit as u64) as u32
    }
}

/// Resource usage monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub network_bytes: u64,
    pub timestamp: DateTime<Utc>,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_bytes: 0,
            disk_bytes: 0,
            network_bytes: 0,
            timestamp: Utc::now(),
        }
    }
}

/// File size formatting
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Duration formatting
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Environment variable helpers
pub fn get_env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn get_env_or_error(key: &str) -> ApiResult<String> {
    std::env::var(key).map_err(|_| {
        common::ApiError::ConfigurationError(format!("Environment variable {} not set", key))
    })
}

/// Configuration validation
pub fn validate_config() -> ApiResult<()> {
    // Check required environment variables
    let required_vars = ["DATABASE_URL", "REDIS_URL"];
    
    for var in required_vars {
        if std::env::var(var).is_err() {
            warn!("Environment variable {} not set, using default", var);
        }
    }
    
    info!("Configuration validation completed");
    Ok(())
}

/// Safe JSON parsing with error handling
pub fn safe_json_parse<T>(json_str: &str) -> ApiResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(json_str).map_err(|e| {
        error!(error = %e, json = %json_str, "JSON parsing failed");
        common::ApiError::InvalidInput(format!("Invalid JSON: {}", e))
    })
}

/// Safe JSON serialization
pub fn safe_json_serialize<T>(value: &T) -> ApiResult<String>
where
    T: Serialize,
{
    serde_json::to_string(value).map_err(|e| {
        error!(error = %e, "JSON serialization failed");
        common::ApiError::SerializationError
    })
}

/// Rate limiting helpers
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            requests_per_hour: 1000,
            requests_per_day: 10000,
        }
    }
}

/// Cache key generation
pub fn generate_cache_key(prefix: &str, params: &[&str]) -> String {
    let mut key = prefix.to_string();
    for param in params {
        key.push(':');
        key.push_str(param);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_name_validation() {
        assert!(is_valid_dataset_name("valid_dataset"));
        assert!(is_valid_dataset_name("dataset-123"));
        assert!(is_valid_dataset_name("__static__blood_test"));
        
        assert!(!is_valid_dataset_name(""));
        assert!(!is_valid_dataset_name("invalid dataset"));
        assert!(!is_valid_dataset_name("__static__"));
    }

    #[test]
    fn test_static_dataset_detection() {
        assert!(is_static_dataset("__static__test"));
        assert!(!is_static_dataset("regular_dataset"));
        
        assert_eq!(extract_static_dataset_name("__static__blood_test"), Some("blood_test"));
        assert_eq!(extract_static_dataset_name("regular_dataset"), None);
    }

    #[test]
    fn test_ethereum_address_validation() {
        assert!(is_valid_ethereum_address("0x742d35cc6564c06e5bf7b3b6b2c8f1c12e12345a"));
        assert!(!is_valid_ethereum_address("0x742d35cc6564c06e5bf7b3b6b2c8f1c12e12345")); // Too short
        assert!(!is_valid_ethereum_address("742d35cc6564c06e5bf7b3b6b2c8f1c12e12345a")); // No 0x prefix
    }

    #[test]
    fn test_cid_validation() {
        assert!(is_valid_cid("QmYjtig7VJQ6XsnUjqqJvj7QaMcCAwtrgNdahSiFofrE7o"));
        assert!(!is_valid_cid("invalid_cid"));
        assert!(!is_valid_cid("Qm123")); // Too short
    }

    #[test]
    fn test_pagination() {
        let params = PaginationParams::new(Some(2), Some(10));
        assert_eq!(params.page, 2);
        assert_eq!(params.limit, 10);
        assert_eq!(params.offset, 10);

        assert_eq!(PaginationParams::total_pages(25, 10), 3);
        assert_eq!(PaginationParams::total_pages(20, 10), 2);
    }

    #[test]
    fn test_file_size_formatting() {
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1536), "1.50 KB");
        assert_eq!(format_file_size(1048576), "1.00 MB");
        assert_eq!(format_file_size(500), "500 B");
    }

    #[test]
    fn test_duration_formatting() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn test_cache_key_generation() {
        let key = generate_cache_key("user", &["123", "profile"]);
        assert_eq!(key, "user:123:profile");
    }

    #[test]
    fn test_sha256_hash() {
        let hash1 = sha256_string("test");
        let hash2 = sha256_string("test");
        let hash3 = sha256_string("different");
        
        assert_eq!(hash1, hash2); // Same input, same hash
        assert_ne!(hash1, hash3); // Different input, different hash
        assert_eq!(hash1.len(), 64); // SHA256 produces 64-char hex string
    }
} 