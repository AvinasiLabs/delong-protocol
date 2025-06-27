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
    /// Extract client IP with fallback strategy
    /// First tries proxy headers, then falls back to direct connection IP
    pub fn extract_client_ip_with_fallback(
        headers: &HeaderMap,
        connect_info: Option<std::net::SocketAddr>,
    ) -> Option<String> {
        // First try proxy headers (for load balancer/proxy scenarios)
        if let Some(ip) = Self::extract_client_ip(headers) {
            return Some(ip);
        }

        // Fallback to direct connection IP (for direct exposure scenarios)
        connect_info.map(|addr| addr.ip().to_string())
    }

    /// Extract client IP from proxy headers only
    pub fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
        // Check X-Forwarded-For header (most common)
        if let Some(forwarded) = headers.get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                // Take the first IP in the chain (original client)
                return Some(forwarded_str.split(',').next()?.trim().to_string());
            }
        }

        // Check X-Real-IP header (nginx style)
        if let Some(real_ip) = headers.get("x-real-ip") {
            if let Ok(ip_str) = real_ip.to_str() {
                return Some(ip_str.to_string());
            }
        }

        // Check CF-Connecting-IP (Cloudflare)
        if let Some(cf_ip) = headers.get("cf-connecting-ip") {
            if let Ok(ip_str) = cf_ip.to_str() {
                return Some(ip_str.to_string());
            }
        }

        None
    }

    /// Determine if we're behind a trusted proxy
    pub fn is_behind_proxy(headers: &HeaderMap) -> bool {
        headers.get("x-forwarded-for").is_some()
            || headers.get("x-real-ip").is_some()
            || headers.get("cf-connecting-ip").is_some()
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
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_extract_client_ip_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.168.1.1, 10.0.0.1"),
        );

        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.1"));

        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_cloudflare() {
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("198.51.100.1"));

        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("198.51.100.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.1"));
        headers.insert("cf-connecting-ip", HeaderValue::from_static("198.51.100.1"));

        // x-forwarded-for should have priority
        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_with_fallback_proxy_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, Some(socket_addr));

        // Should use proxy header, not socket addr
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_with_fallback_direct_connection() {
        let headers = HeaderMap::new(); // No proxy headers

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 8080);
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, Some(socket_addr));

        // Should use socket addr as fallback
        assert_eq!(ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_with_fallback_no_info() {
        let headers = HeaderMap::new(); // No proxy headers
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);

        // Should return None when no information available
        assert_eq!(ip, None);
    }

    #[test]
    fn test_is_behind_proxy() {
        let mut headers = HeaderMap::new();
        assert!(!MiddlewareUtils::is_behind_proxy(&headers));

        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));

        headers.clear();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));

        headers.clear();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("198.51.100.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));
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

    #[test]
    fn test_multiple_forwarded_ips() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.195, 70.41.3.18, 150.172.238.178"),
        );

        // Should extract the first (original client) IP
        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("203.0.113.195".to_string()));
    }

    #[test]
    fn test_malformed_headers() {
        let mut headers = HeaderMap::new();

        // Test with malformed X-Forwarded-For
        headers.insert("x-forwarded-for", HeaderValue::from_static("invalid-ip"));
        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("invalid-ip".to_string())); // Should still extract, validation is elsewhere

        // Test with empty X-Forwarded-For
        headers.insert("x-forwarded-for", HeaderValue::from_static(""));
        let ip = MiddlewareUtils::extract_client_ip(&headers);
        assert_eq!(ip, Some("".to_string()));
    }

    #[test]
    fn test_proxy_header_takes_priority_over_socket() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));

        let socket_addr = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            8080,
        ));

        // Proxy header should take priority over socket address
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, socket_addr);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_proxy_detection_comprehensive() {
        let mut headers = HeaderMap::new();

        // Test with no headers
        assert!(!MiddlewareUtils::is_behind_proxy(&headers));

        // Test with X-Forwarded-For
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));

        headers.clear();

        // Test with X-Real-IP
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));

        headers.clear();

        // Test with CF-Connecting-IP
        headers.insert("cf-connecting-ip", HeaderValue::from_static("198.51.100.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));
    }

    #[test]
    fn test_fallback_priority() {
        let mut headers = HeaderMap::new();
        let socket_addr = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            8080,
        ));

        // With proxy header - should use proxy
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, socket_addr);
        assert_eq!(ip, Some("192.168.1.1".to_string()));

        // Without proxy header - should use socket addr
        headers.clear();
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, socket_addr);
        assert_eq!(ip, Some("127.0.0.1".to_string()));

        // Without either - should return None
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        assert_eq!(ip, None);
    }
}
