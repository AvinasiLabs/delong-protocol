//! Request ID middleware for HTTP request tracking
//!
//! This middleware ensures every HTTP request has a unique identifier that can be
//! used for tracing, logging, and debugging across services.

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::SystemTime;
use tracing::{Span, instrument};

/// Header name for request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Request ID middleware that generates or extracts request IDs
#[instrument(skip(request, next), fields(request_id))]
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Try to extract existing request ID from headers
    let request_id = extract_or_generate_request_id(request.headers());

    // Record request ID in the current tracing span
    Span::current().record("request_id", &request_id);

    // Add request ID to request headers for downstream services
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        request
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), header_value);
    }

    // Process the request
    let mut response = next.run(request).await;

    // Add request ID to response headers for client tracking
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), header_value);
    }

    response
}

/// Extract request ID from headers or generate a new one
fn extract_or_generate_request_id(headers: &HeaderMap) -> String {
    // Try common request ID headers
    let request_id_headers = [
        REQUEST_ID_HEADER,
        "x-request-uuid",
        "x-correlation-id",
        "x-trace-id",
        "request-id",
    ];

    for header_name in &request_id_headers {
        if let Some(header_value) = headers.get(*header_name) {
            if let Ok(request_id) = header_value.to_str() {
                if is_valid_request_id(request_id) {
                    return request_id.to_string();
                }
            }
        }
    }

    // No valid request ID found, generate a new one
    generate_request_id()
}

/// Generate a new unique request ID
pub fn generate_request_id() -> String {
    // Create a request ID based on current time and thread
    let mut hasher = DefaultHasher::new();

    // Add current timestamp
    if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        duration.as_nanos().hash(&mut hasher);
    }

    // Add thread ID for additional uniqueness
    std::thread::current().id().hash(&mut hasher);

    // Add some randomness
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);

    // Format as a readable request ID
    format!("req_{:016x}", hasher.finish())
}

/// Validate request ID format
fn is_valid_request_id(request_id: &str) -> bool {
    // Basic validation: should be reasonable length and contain safe characters
    if request_id.is_empty() || request_id.len() > 128 {
        return false;
    }

    // Should only contain alphanumeric characters, hyphens, and underscores
    request_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Extract request ID from current request headers (utility for handlers)
#[allow(dead_code)]
pub fn get_request_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string())
}

/// Extract request ID from current tracing span (utility for handlers)
#[allow(dead_code)]
pub fn get_current_request_id() -> Option<String> {
    // This would require custom span visitor to extract fields
    // For now, we'll return None and rely on header extraction
    None
}

/// Request ID generator with custom format
#[allow(dead_code)]
pub struct RequestIdGenerator {
    prefix: String,
    counter: std::sync::atomic::AtomicU64,
}

#[allow(dead_code)]
impl RequestIdGenerator {
    /// Create a new request ID generator with custom prefix
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            counter: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Generate a request ID with the configured prefix
    pub fn generate(&self) -> String {
        let count = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        format!("{}_{:013x}_{:08x}", self.prefix, timestamp, count)
    }
}

impl Default for RequestIdGenerator {
    fn default() -> Self {
        Self::new("req")
    }
}

/// Configuration for request ID middleware
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RequestIdConfig {
    /// Custom header name for request ID
    pub header_name: String,
    /// Custom prefix for generated request IDs
    pub prefix: String,
    /// Whether to add request ID to response headers
    pub add_to_response: bool,
    /// Whether to override existing request IDs
    pub override_existing: bool,
}

impl Default for RequestIdConfig {
    fn default() -> Self {
        Self {
            header_name: REQUEST_ID_HEADER.to_string(),
            prefix: "req".to_string(),
            add_to_response: true,
            override_existing: false,
        }
    }
}

/// Request ID middleware with custom configuration
#[allow(dead_code)]
#[instrument(skip(request, next, config), fields(request_id))]
pub async fn request_id_middleware_with_config(
    mut request: Request,
    next: Next,
    config: RequestIdConfig,
) -> Response {
    let header_name = HeaderName::from_bytes(config.header_name.as_bytes())
        .unwrap_or_else(|_| HeaderName::from_static(REQUEST_ID_HEADER));

    // Extract or generate request ID based on configuration
    let request_id = if config.override_existing {
        generate_request_id_with_prefix(&config.prefix)
    } else {
        extract_or_generate_request_id_with_config(request.headers(), &config)
    };

    // Record in tracing span
    Span::current().record("request_id", &request_id);

    // Add to request headers
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        request
            .headers_mut()
            .insert(header_name.clone(), header_value);
    }

    // Process request
    let mut response = next.run(request).await;

    // Add to response headers if configured
    if config.add_to_response {
        if let Ok(header_value) = HeaderValue::from_str(&request_id) {
            response.headers_mut().insert(header_name, header_value);
        }
    }

    response
}

/// Extract request ID with custom configuration
fn extract_or_generate_request_id_with_config(
    headers: &HeaderMap,
    config: &RequestIdConfig,
) -> String {
    if let Some(header_value) = headers.get(&config.header_name) {
        if let Ok(request_id) = header_value.to_str() {
            if is_valid_request_id(request_id) {
                return request_id.to_string();
            }
        }
    }

    generate_request_id_with_prefix(&config.prefix)
}

/// Generate request ID with custom prefix
#[allow(dead_code)]
fn generate_request_id_with_prefix(prefix: &str) -> String {
    let mut hasher = DefaultHasher::new();

    if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        duration.as_nanos().hash(&mut hasher);
    }

    std::thread::current().id().hash(&mut hasher);

    format!("{}_{:016x}", prefix, hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();

        // Should generate unique IDs
        assert_ne!(id1, id2);

        // Should have correct format
        assert!(id1.starts_with("req_"));
        assert!(id2.starts_with("req_"));

        // Should be valid format
        assert!(is_valid_request_id(&id1));
        assert!(is_valid_request_id(&id2));
    }

    #[test]
    fn test_is_valid_request_id() {
        // Valid request IDs
        assert!(is_valid_request_id("req_123456789abcdef0"));
        assert!(is_valid_request_id("trace-12345"));
        assert!(is_valid_request_id("correlation_id_123"));

        // Invalid request IDs
        assert!(!is_valid_request_id(""));
        assert!(!is_valid_request_id("invalid id with spaces"));
        assert!(!is_valid_request_id("id@with#special$chars"));
        assert!(!is_valid_request_id(&"x".repeat(200))); // Too long
    }

    #[test]
    fn test_extract_existing_request_id() {
        let mut headers = HeaderMap::new();
        headers.insert(
            REQUEST_ID_HEADER,
            HeaderValue::from_static("existing-request-123"),
        );

        let request_id = extract_or_generate_request_id(&headers);
        assert_eq!(request_id, "existing-request-123");
    }

    #[test]
    fn test_extract_from_alternative_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-correlation-id",
            HeaderValue::from_static("correlation-456"),
        );

        let request_id = extract_or_generate_request_id(&headers);
        assert_eq!(request_id, "correlation-456");
    }

    #[test]
    fn test_generate_when_no_existing_id() {
        let headers = HeaderMap::new();
        let request_id = extract_or_generate_request_id(&headers);

        assert!(request_id.starts_with("req_"));
        assert!(is_valid_request_id(&request_id));
    }

    #[test]
    fn test_request_id_generator() {
        let generator = RequestIdGenerator::new("test");

        let id1 = generator.generate();
        let id2 = generator.generate();

        assert_ne!(id1, id2);
        assert!(id1.starts_with("test_"));
        assert!(id2.starts_with("test_"));
    }

    #[test]
    fn test_request_id_generator_default() {
        let generator = RequestIdGenerator::default();
        let id = generator.generate();

        assert!(id.starts_with("req_"));
    }

    #[test]
    fn test_get_request_id_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            REQUEST_ID_HEADER,
            HeaderValue::from_static("test-request-789"),
        );

        let request_id = get_request_id_from_headers(&headers);
        assert_eq!(request_id, Some("test-request-789".to_string()));
    }

    #[test]
    fn test_get_request_id_from_empty_headers() {
        let headers = HeaderMap::new();
        let request_id = get_request_id_from_headers(&headers);
        assert_eq!(request_id, None);
    }

    #[test]
    fn test_request_id_config() {
        let config = RequestIdConfig {
            header_name: "x-custom-id".to_string(),
            prefix: "custom".to_string(),
            add_to_response: false,
            override_existing: true,
        };

        assert_eq!(config.header_name, "x-custom-id");
        assert_eq!(config.prefix, "custom");
        assert!(!config.add_to_response);
        assert!(config.override_existing);
    }

    #[test]
    fn test_generate_request_id_with_prefix() {
        let id = generate_request_id_with_prefix("custom");
        assert!(id.starts_with("custom_"));
        assert!(is_valid_request_id(&id));
    }

    #[test]
    fn test_request_id_uniqueness() {
        // Generate multiple IDs and ensure they're all unique
        let mut ids = std::collections::HashSet::new();

        for _ in 0..100 {
            let id = generate_request_id();
            assert!(ids.insert(id), "Generated duplicate request ID");
        }
    }
}
