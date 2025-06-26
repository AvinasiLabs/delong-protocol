//! Logging middleware for HTTP request/response logging
//!
//! This middleware provides structured logging for all HTTP requests and responses,
//! including timing information, status codes, and request metadata.

use axum::{
    extract::Request,
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{Level, info, instrument, warn};

use crate::{
    // handlers::metrics::record_request_metric,
    middleware::MiddlewareUtils,
    utils::{current_timestamp_ms, format_duration, sanitize_path_for_logging},
};

/// Logging configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LoggingConfig {
    /// Whether to log health check requests
    pub log_health_checks: bool,
    /// Whether to log request headers (be careful with sensitive data)
    pub log_headers: bool,
    /// Whether to log request body (only for small requests)
    pub log_request_body: bool,
    /// Maximum request body size to log (in bytes)
    pub max_body_log_size: usize,
    /// Whether to log response body
    pub log_response_body: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_health_checks: false,
            log_headers: false,
            log_request_body: false,
            max_body_log_size: 1024, // 1KB
            log_response_body: false,
        }
    }
}

/// Main logging middleware
#[instrument(skip(request, next), fields(
    method = %request.method(),
    path = %request.uri().path(),
    request_id
))]
pub async fn logging_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let start_time = Instant::now();
    let timestamp = current_timestamp_ms();

    // Extract request information
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();
    let query = uri.query().unwrap_or("");
    let headers = request.headers().clone();

    // Extract client information
    let client_ip = MiddlewareUtils::extract_client_ip(&headers);
    let user_agent = MiddlewareUtils::extract_user_agent(&headers);

    // Get or generate request ID
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Set request ID in tracing span
    tracing::Span::current().record("request_id", &request_id);

    // Check if this is a health check request
    let is_health_check = MiddlewareUtils::is_health_check_request(path, user_agent.as_deref());

    // Log request start (skip health checks by default)
    if !is_health_check || should_log_health_checks() {
        info!(
            request_id = request_id,
            method = %method,
            path = sanitize_path_for_logging(path),
            query = if query.is_empty() { None } else { Some(query) },
            client_ip = client_ip,
            user_agent = user_agent,
            timestamp = timestamp,
            "Request started"
        );

        // Log headers if configured (be careful with sensitive data)
        if should_log_headers() && !is_health_check {
            log_request_headers(&headers, &request_id);
        }
    }

    // Process the request
    let response = next.run(request).await;

    // Calculate response time
    let duration = start_time.elapsed();
    let status = response.status();
    let status_code = status.as_u16();

    // Determine log level based on status code
    let log_level = determine_log_level(status_code, is_health_check);

    // Log response (skip health checks by default)
    if !is_health_check || should_log_health_checks() {
        match log_level {
            Level::ERROR => {
                tracing::error!(
                    request_id = request_id,
                    method = %method,
                    path = sanitize_path_for_logging(path),
                    status = status_code,
                    duration = %format_duration(duration),
                    duration_ms = duration.as_millis() as u64,
                    client_ip = client_ip,
                    "Request completed with error"
                );
            }
            Level::WARN => {
                warn!(
                    request_id = request_id,
                    method = %method,
                    path = sanitize_path_for_logging(path),
                    status = status_code,
                    duration = %format_duration(duration),
                    duration_ms = duration.as_millis() as u64,
                    client_ip = client_ip,
                    "Request completed with warning"
                );
            }
            _ => {
                info!(
                    request_id = request_id,
                    method = %method,
                    path = sanitize_path_for_logging(path),
                    status = status_code,
                    duration = %format_duration(duration),
                    duration_ms = duration.as_millis() as u64,
                    client_ip = client_ip,
                    "Request completed successfully"
                );
            }
        }
    }

    // Record metrics
    // let endpoint = format!("{} {}", method, path);
    // let is_error = status_code >= 400;
    // record_request_metric(&endpoint, duration, is_error);

    Ok(response)
}

/// Log request headers (filtered for security)
fn log_request_headers(headers: &HeaderMap, request_id: &str) {
    let mut safe_headers = std::collections::HashMap::new();

    // Only log safe headers
    let safe_header_names = [
        "content-type",
        "accept",
        "accept-encoding",
        "accept-language",
        "cache-control",
        "connection",
        "host",
        "referer",
        "user-agent",
        "x-forwarded-for",
        "x-real-ip",
        "x-request-id",
    ];

    for header_name in &safe_header_names {
        if let Some(value) = headers.get(*header_name) {
            if let Ok(value_str) = value.to_str() {
                safe_headers.insert(header_name.to_string(), value_str.to_string());
            }
        }
    }

    if !safe_headers.is_empty() {
        info!(
            request_id = request_id,
            headers = ?safe_headers,
            "Request headers"
        );
    }
}

/// Determine appropriate log level based on response status
fn determine_log_level(status_code: u16, is_health_check: bool) -> Level {
    if is_health_check {
        return Level::DEBUG;
    }

    match status_code {
        500..=599 => Level::ERROR,
        400..=499 => Level::WARN,
        _ => Level::INFO,
    }
}

/// Check if health check requests should be logged
fn should_log_health_checks() -> bool {
    // Check environment variable or use default config
    std::env::var("LOG_HEALTH_CHECKS")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Check if request headers should be logged
fn should_log_headers() -> bool {
    // Check environment variable or use default config
    std::env::var("LOG_REQUEST_HEADERS")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Log slow requests (requests taking longer than threshold)
#[allow(dead_code)]
pub fn log_slow_request(
    method: &Method,
    path: &str,
    duration: std::time::Duration,
    threshold: std::time::Duration,
    request_id: &str,
) {
    if duration > threshold {
        warn!(
            request_id = request_id,
            method = %method,
            path = sanitize_path_for_logging(path),
            duration = %format_duration(duration),
            duration_ms = duration.as_millis() as u64,
            threshold_ms = threshold.as_millis() as u64,
            "Slow request detected"
        );
    }
}
#[allow(dead_code)]
/// Log request size warnings
pub fn log_large_request(
    method: &Method,
    path: &str,
    size_bytes: usize,
    threshold_bytes: usize,
    request_id: &str,
) {
    if size_bytes > threshold_bytes {
        warn!(
            request_id = request_id,
            method = %method,
            path = sanitize_path_for_logging(path),
            size_bytes = size_bytes,
            threshold_bytes = threshold_bytes,
            size_mb = size_bytes as f64 / 1024.0 / 1024.0,
            "Large request detected"
        );
    }
}

/// Middleware for detecting and logging suspicious requests
#[instrument(skip(request, next))]
#[allow(dead_code)]
pub async fn security_logging_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = request.method().clone();
    let path = request.uri().path();
    let headers = request.headers();

    // Extract request ID
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    // Check for suspicious patterns
    check_suspicious_patterns(&method, path, headers, request_id);

    // Process request normally
    Ok(next.run(request).await)
}

/// Check for suspicious request patterns
fn check_suspicious_patterns(method: &Method, path: &str, headers: &HeaderMap, request_id: &str) {
    // Check for common attack patterns in path
    let suspicious_patterns = [
        "../",
        "..\\",
        "%2e%2e",
        "%252e%252e",
        "<script",
        "javascript:",
        "data:",
        "union",
        "select",
        "drop",
        "delete",
        "exec",
        "eval",
        "system",
    ];

    let path_lower = path.to_lowercase();
    for pattern in &suspicious_patterns {
        if path_lower.contains(pattern) {
            warn!(
                request_id = request_id,
                method = %method,
                path = sanitize_path_for_logging(path),
                pattern = pattern,
                client_ip = MiddlewareUtils::extract_client_ip(headers),
                user_agent = MiddlewareUtils::extract_user_agent(headers),
                "Suspicious request pattern detected"
            );
            break;
        }
    }

    // Check for suspicious headers
    if let Some(user_agent) = headers.get("user-agent") {
        if let Ok(ua_str) = user_agent.to_str() {
            let ua_lower = ua_str.to_lowercase();
            let suspicious_ua_patterns = [
                "sqlmap", "nikto", "burp", "nmap", "masscan", "bot", "crawler", "scanner",
                "exploit",
            ];

            if suspicious_ua_patterns
                .iter()
                .any(|pattern| ua_lower.contains(pattern))
            {
                warn!(
                    request_id = request_id,
                    method = %method,
                    path = sanitize_path_for_logging(path),
                    user_agent = ua_str,
                    client_ip = MiddlewareUtils::extract_client_ip(headers),
                    "Suspicious user agent detected"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_determine_log_level() {
        assert_eq!(determine_log_level(200, false), Level::INFO);
        assert_eq!(determine_log_level(404, false), Level::WARN);
        assert_eq!(determine_log_level(500, false), Level::ERROR);
        assert_eq!(determine_log_level(200, true), Level::DEBUG);
    }

    #[test]
    fn test_should_log_health_checks() {
        // Default should be false
        assert!(!should_log_health_checks());
    }

    #[test]
    fn test_log_request_headers_filtering() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer secret"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert("user-agent", HeaderValue::from_static("test-agent"));
        headers.insert("x-api-key", HeaderValue::from_static("secret-key"));

        // The function should only log safe headers, not sensitive ones
        log_request_headers(&headers, "test-123");
        // This test mainly ensures the function doesn't panic
        // In a real test, we'd capture log output and verify filtering
    }

    #[test]
    fn test_suspicious_pattern_detection() {
        let method = Method::GET;
        let headers = HeaderMap::new();

        // Test various suspicious patterns
        let suspicious_paths = [
            "/api/../../../etc/passwd",
            "/api/users?id=1' OR 1=1--",
            "/api/data?search=<script>alert('xss')</script>",
        ];

        for path in &suspicious_paths {
            check_suspicious_patterns(&method, path, &headers, "test-req");
            // This mainly tests that the function doesn't panic
            // In practice, we'd capture and verify log messages
        }
    }

    #[test]
    fn test_log_slow_request() {
        use std::time::Duration;

        let method = Method::POST;
        let path = "/api/slow-endpoint";
        let duration = Duration::from_millis(5000);
        let threshold = Duration::from_millis(1000);

        log_slow_request(&method, path, duration, threshold, "slow-req-123");
        // Test that function executes without panic
    }

    #[test]
    fn test_log_large_request() {
        let method = Method::POST;
        let path = "/api/upload";
        let size_bytes = 10 * 1024 * 1024; // 10MB
        let threshold_bytes = 5 * 1024 * 1024; // 5MB

        log_large_request(&method, path, size_bytes, threshold_bytes, "large-req-456");
        // Test that function executes without panic
    }
}
