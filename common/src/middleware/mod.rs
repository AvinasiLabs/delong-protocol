//! Common middleware utilities for DeLong Protocol services
//!
//! This module provides shared middleware functionality that can be used across
//! multiple services including logging, request ID generation, and common utilities.

pub mod logging;
pub mod request_id;

use axum::http::HeaderMap;
use std::net::SocketAddr;

/// Common request ID header name used across all services
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Utility functions for middleware operations
pub struct MiddlewareUtils;

impl MiddlewareUtils {
    /// Extract client IP address with fallback strategy
    /// Prioritizes proxy headers over direct connection info
    pub fn extract_client_ip_with_fallback(
        headers: &HeaderMap,
        socket_addr: Option<SocketAddr>,
    ) -> Option<String> {
        // Try X-Forwarded-For first (most common proxy header)
        if let Some(forwarded_for) = headers.get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded_for.to_str() {
                // Take the first IP in the chain (original client)
                if let Some(first_ip) = forwarded_str.split(',').next() {
                    let trimmed_ip = first_ip.trim();
                    if !trimmed_ip.is_empty() {
                        return Some(trimmed_ip.to_string());
                    }
                }
            }
        }

        // Try X-Real-IP (used by some proxies)
        if let Some(real_ip) = headers.get("x-real-ip") {
            if let Ok(real_ip_str) = real_ip.to_str() {
                let trimmed_ip = real_ip_str.trim();
                if !trimmed_ip.is_empty() {
                    return Some(trimmed_ip.to_string());
                }
            }
        }

        // Try CF-Connecting-IP (Cloudflare)
        if let Some(cf_ip) = headers.get("cf-connecting-ip") {
            if let Ok(cf_ip_str) = cf_ip.to_str() {
                let trimmed_ip = cf_ip_str.trim();
                if !trimmed_ip.is_empty() {
                    return Some(trimmed_ip.to_string());
                }
            }
        }

        // Fallback to direct connection IP
        socket_addr.map(|addr| addr.ip().to_string())
    }

    /// Extract user agent from request headers
    pub fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Check if request appears to be from a proxy or load balancer
    pub fn is_behind_proxy(headers: &HeaderMap) -> bool {
        headers.contains_key("x-forwarded-for")
            || headers.contains_key("x-real-ip")
            || headers.contains_key("x-forwarded-proto")
            || headers.contains_key("cf-connecting-ip")
    }

    /// Check if this is a health check request
    pub fn is_health_check_request(path: &str, user_agent: Option<&str>) -> bool {
        // Check path patterns
        let health_paths = ["/health", "/ping", "/status", "/ready", "/live"];
        let is_health_path = health_paths.iter().any(|&health_path| {
            path == health_path || path.starts_with(&format!("{}/", health_path))
        });

        if is_health_path {
            return true;
        }

        // Check user agent patterns (common health check user agents)
        if let Some(ua) = user_agent {
            let ua_lower = ua.to_lowercase();
            let health_check_patterns = [
                "kube-probe",
                "health",
                "monitor",
                "check",
                "ping",
                "uptime",
                "consul",
                "nomad",
            ];

            return health_check_patterns
                .iter()
                .any(|pattern| ua_lower.contains(pattern));
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
            HeaderValue::from_static("203.0.113.1, 192.168.1.1, 127.0.0.1"),
        );

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        assert_eq!(ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.2"));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        assert_eq!(ip, Some("203.0.113.2".to_string()));
    }

    #[test]
    fn test_extract_client_ip_cloudflare() {
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.3"));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        assert_eq!(ip, Some("203.0.113.3".to_string()));
    }

    #[test]
    fn test_extract_client_ip_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.1"));
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.2"));
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.3"));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        // Should prefer x-forwarded-for
        assert_eq!(ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_extract_client_ip_with_fallback_direct_connection() {
        let headers = HeaderMap::new();
        let socket_addr = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            8080,
        ));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, socket_addr);
        assert_eq!(ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_extract_client_ip_with_fallback_no_info() {
        let headers = HeaderMap::new();
        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        assert_eq!(ip, None);
    }

    #[test]
    fn test_extract_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 (Test Browser)"),
        );

        let ua = MiddlewareUtils::extract_user_agent(&headers);
        assert_eq!(ua, Some("Mozilla/5.0 (Test Browser)".to_string()));
    }

    #[test]
    fn test_is_behind_proxy() {
        let mut headers = HeaderMap::new();
        assert!(!MiddlewareUtils::is_behind_proxy(&headers));

        headers.insert("x-forwarded-for", HeaderValue::from_static("127.0.0.1"));
        assert!(MiddlewareUtils::is_behind_proxy(&headers));
    }

    #[test]
    fn test_is_health_check_request() {
        assert!(MiddlewareUtils::is_health_check_request("/health", None));
        assert!(MiddlewareUtils::is_health_check_request("/ping", None));
        assert!(MiddlewareUtils::is_health_check_request(
            "/status",
            Some("kube-probe/1.0")
        ));
        assert!(!MiddlewareUtils::is_health_check_request("/api/data", None));
    }

    #[test]
    fn test_multiple_forwarded_ips() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("  203.0.113.1  ,  192.168.1.1  "),
        );

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        assert_eq!(ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_malformed_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static(""));

        let socket_addr = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            8080,
        ));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, socket_addr);
        assert_eq!(ip, Some("127.0.0.1".to_string()));
    }

    #[test]
    fn test_proxy_header_takes_priority_over_socket() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.1"));

        let socket_addr = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            8080,
        ));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, socket_addr);
        assert_eq!(ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_fallback_priority() {
        // Test that headers are tried in correct order
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.3"));
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.2"));

        let ip = MiddlewareUtils::extract_client_ip_with_fallback(&headers, None);
        // Should prefer x-real-ip over cf-connecting-ip
        assert_eq!(ip, Some("203.0.113.2".to_string()));
    }

    #[test]
    fn test_proxy_detection_comprehensive() {
        let test_cases = [
            ("x-forwarded-for", "127.0.0.1"),
            ("x-real-ip", "127.0.0.1"),
            ("x-forwarded-proto", "https"),
            ("cf-connecting-ip", "127.0.0.1"),
        ];

        for (header_name, header_value) in &test_cases {
            let mut headers = HeaderMap::new();
            headers.insert(*header_name, HeaderValue::from_static(header_value));
            assert!(
                MiddlewareUtils::is_behind_proxy(&headers),
                "Failed to detect proxy for header: {}",
                header_name
            );
        }
    }
}
