use axum::http::HeaderMap;

pub mod http_client;

// Re-export utilities from common crate
pub use common::utils::{
    clean_for_logging, current_timestamp_ms, current_timestamp_secs, env_var_as_bool,
    env_var_as_u64, env_var_or_default, format_bytes, format_duration, generate_unique_id,
    hash_string, is_safe_for_logging, sanitize_path_for_logging, truncate_string,
};

// Re-export auth utilities from common crate
pub use common::is_valid_api_key_format;

/// Extract API key from request headers
pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // Try different header formats
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            // API key format (Bearer is handled separately for JWT)
            if auth_str.starts_with("ApiKey ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }

    // Direct API key header
    if let Some(api_key_header) = headers.get("x-api-key") {
        if let Ok(api_key) = api_key_header.to_str() {
            return Some(api_key.to_string());
        }
    }

    None
}

/// Generate a unique request ID with "req_" prefix
pub fn generate_request_id() -> String {
    format!("req_{}", generate_unique_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};
    use std::time::Duration;

    #[test]
    fn test_extract_api_key_api_key_format() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("ApiKey test-api-key-123"),
        );

        let api_key = extract_api_key(&headers);
        assert_eq!(api_key, Some("test-api-key-123".to_string()));
    }

    #[test]
    fn test_extract_api_key_direct() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("test-api-key-123"));

        let api_key = extract_api_key(&headers);
        assert_eq!(api_key, Some("test-api-key-123".to_string()));
    }

    #[test]
    fn test_extract_api_key_ignores_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer jwt-token-123"),
        );

        let api_key = extract_api_key(&headers);
        assert_eq!(api_key, None); // Should not extract JWT tokens as API keys
    }

    #[test]
    fn test_is_valid_api_key_format() {
        assert!(is_valid_api_key_format("test-api-key-123456"));
        assert!(!is_valid_api_key_format("short"));
        assert!(!is_valid_api_key_format("test@api#key"));
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
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.50s");
        assert_eq!(format_duration(Duration::from_millis(90000)), "1.50m");
    }
}
