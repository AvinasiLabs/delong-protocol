//! Middleware modules for the Delong gateway
//!
//! This module provides simplified middleware for common gateway functionality:
//! - Authentication: API key validation and user authentication
//! - Logging: Request/response logging with structured data
//! - Request ID: Unique request identifier for tracing

pub mod auth;
pub mod logging;
pub mod request_id;

use axum::http::HeaderMap;

/// Common middleware utilities
pub struct MiddlewareUtils;

impl MiddlewareUtils {
    /// Extract client IP from request headers
    pub fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
        // Check common headers for client IP
        if let Some(forwarded) = headers.get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                return Some(forwarded_str.split(',').next()?.trim().to_string());
            }
        }

        if let Some(real_ip) = headers.get("x-real-ip") {
            if let Ok(ip_str) = real_ip.to_str() {
                return Some(ip_str.to_string());
            }
        }

        None
    }

    /// Extract user agent from request headers
    pub fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|ua| ua.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Check if request is from a health check probe
    pub fn is_health_check_request(path: &str, user_agent: Option<&str>) -> bool {
        // Common health check paths
        if matches!(path, "/health" | "/health/live" | "/health/ready" | "/ping") {
            return true;
        }

        // Common health check user agents
        if let Some(ua) = user_agent {
            let ua_lower = ua.to_lowercase();
            return ua_lower.contains("kube-probe")
                || ua_lower.contains("health")
                || ua_lower.contains("ping")
                || ua_lower.contains("monitor");
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_extract_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.168.1.1, 10.0.0.1"),
        );

        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_extract_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static("Mozilla/5.0 Test"));

        let ua = MiddlewareUtils::extract_user_agent(&headers);
        assert_eq!(ua, Some("Mozilla/5.0 Test".to_string()));
    }

    #[test]
    fn test_is_health_check_request() {
        assert!(MiddlewareUtils::is_health_check_request("/health", None));
        assert!(MiddlewareUtils::is_health_check_request(
            "/health/live",
            None
        ));
        assert!(MiddlewareUtils::is_health_check_request(
            "/api/test",
            Some("kube-probe/1.0")
        ));
        assert!(!MiddlewareUtils::is_health_check_request(
            "/api/test",
            Some("Mozilla/5.0")
        ));
    }
}
