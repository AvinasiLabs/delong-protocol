use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod http_client;

/// Common error response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub timestamp: String,
    pub request_id: Option<String>,
}

#[allow(dead_code)]
impl ErrorResponse {
    /// Create a new error response
    pub fn new(error: &str, message: &str) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: None,
        }
    }

    /// Create error response with request ID
    pub fn with_request_id(error: &str, message: &str, request_id: String) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: Some(request_id),
        }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let status = match self.error.as_str() {
            "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
            "FORBIDDEN" => StatusCode::FORBIDDEN,
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            "BAD_REQUEST" => StatusCode::BAD_REQUEST,
            "TIMEOUT" => StatusCode::REQUEST_TIMEOUT,
            "TOO_MANY_REQUESTS" => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}

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

/// Generate a unique request ID
pub fn generate_request_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);

    format!("req_{:x}", hasher.finish())
}

/// Get current timestamp in milliseconds
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as u64
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

/// Validate API key format
pub fn is_valid_api_key_format(api_key: &str) -> bool {
    // Basic validation: should be alphanumeric, minimum 16 characters
    api_key.len() >= 16
        && api_key.len() <= 64
        && api_key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

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
