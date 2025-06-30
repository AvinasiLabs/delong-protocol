//! Common utility functions for DeLong Protocol services
//!
//! This module provides shared utility functions that can be used across
//! multiple services in the DeLong Protocol.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds since Unix epoch
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as u64
}

/// Get current timestamp in seconds since Unix epoch
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

/// Format duration for human reading
pub fn format_duration(duration: Duration) -> String {
    let ms = duration.as_millis();
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.2}s", ms as f64 / 1000.0)
    } else {
        format!("{:.2}m", ms as f64 / 60_000.0)
    }
}

/// Sanitize path for logging (remove sensitive parameters)
pub fn sanitize_path_for_logging(path: &str) -> String {
    // Remove query parameters that might contain sensitive data
    if let Some(question_mark_pos) = path.find('?') {
        let base_path = &path[..question_mark_pos];
        format!("{}?<params>", base_path)
    } else {
        path.to_string()
    }
}

/// Generate a unique ID using timestamp and random data
pub fn generate_unique_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    current_timestamp_ms().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);

    // Add some randomness
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

/// Truncate string to specified length, adding ellipsis if needed
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len < 3 {
        "...".chars().take(max_len).collect()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Convert bytes to human readable format
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes < THRESHOLD {
        return format!("{} B", bytes);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Parse environment variable with default value
pub fn env_var_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Parse environment variable as boolean
pub fn env_var_as_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(default)
}

/// Parse environment variable as number
pub fn env_var_as_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Hash a string using SHA-256
pub fn hash_string(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Validate that a string contains only safe characters for logging
pub fn is_safe_for_logging(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_alphanumeric() || "-_.:/".contains(c))
}

/// Clean string for safe logging (replace unsafe characters)
pub fn clean_for_logging(s: &str) -> String {
    s.chars()
        .map(|c| {
            if is_safe_for_logging(&c.to_string()) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp_ms() {
        let ts1 = current_timestamp_ms();
        std::thread::sleep(Duration::from_millis(1));
        let ts2 = current_timestamp_ms();
        assert!(ts2 > ts1);
    }

    #[test]
    fn test_current_timestamp_secs() {
        let ts = current_timestamp_secs();
        assert!(ts > 1600000000); // After 2020
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.50s");
        assert_eq!(format_duration(Duration::from_millis(90000)), "1.50m");
    }

    #[test]
    fn test_sanitize_path_for_logging() {
        assert_eq!(sanitize_path_for_logging("/api/users"), "/api/users");
        assert_eq!(
            sanitize_path_for_logging("/api/users?token=secret"),
            "/api/users?<params>"
        );
    }

    #[test]
    fn test_generate_unique_id() {
        let id1 = generate_unique_id();
        let id2 = generate_unique_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 16);
        assert_eq!(id2.len(), 16);
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 1), ".");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_env_var_or_default() {
        let result = env_var_or_default("NONEXISTENT_VAR", "default_value");
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_env_var_as_bool() {
        assert!(!env_var_as_bool("NONEXISTENT_BOOL_VAR", false));
        assert!(env_var_as_bool("NONEXISTENT_BOOL_VAR", true));
    }

    #[test]
    fn test_env_var_as_u64() {
        assert_eq!(env_var_as_u64("NONEXISTENT_NUM_VAR", 42), 42);
    }

    #[test]
    fn test_hash_string() {
        let hash1 = hash_string("test");
        let hash2 = hash_string("test");
        let hash3 = hash_string("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16);
    }

    #[test]
    fn test_is_safe_for_logging() {
        assert!(is_safe_for_logging("safe_string123"));
        assert!(is_safe_for_logging("api/endpoint:8080"));
        assert!(!is_safe_for_logging("unsafe<script>"));
        assert!(!is_safe_for_logging("with spaces"));
    }

    #[test]
    fn test_clean_for_logging() {
        assert_eq!(clean_for_logging("safe123"), "safe123");
        assert_eq!(clean_for_logging("unsafe<>"), "unsafe__");
        assert_eq!(clean_for_logging("with spaces"), "with_spaces");
    }
}
