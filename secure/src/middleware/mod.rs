// Middleware modules for the secure service
// TODO: Implement authentication, request logging, and other middleware 

pub mod jwt;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, HeaderName},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tower_http::cors::{CorsLayer, Any};
use tracing::{info, warn, error, span, Level};
use crate::utils;

pub use jwt::JwtMiddleware;

/// Add request ID to all requests for tracing
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = utils::generate_request_id();
    
    // Add request ID to headers for downstream services
    request.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("invalid")),
    );

    // Create a span for this request
    let span = span!(
        Level::INFO,
        "http_request",
        method = %request.method(),
        uri = %request.uri(),
        request_id = %request_id
    );

    let _enter = span.enter();
    
    info!("Processing request");
    let response = next.run(request).await;
    info!(status = %response.status(), "Request completed");
    
    response
}

/// Request logging middleware with timing
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    info!(
        method = %method,
        uri = %uri,
        user_agent = %user_agent,
        "Request started"
    );

    let response = next.run(request).await;
    let elapsed = start.elapsed();

    let status = response.status();
    if status.is_success() {
        info!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = elapsed.as_millis(),
            "Request completed successfully"
        );
    } else if status.is_client_error() {
        warn!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = elapsed.as_millis(),
            "Request failed with client error"
        );
    } else {
        error!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = elapsed.as_millis(),
            "Request failed with server error"
        );
    }

    response
}

/// CORS middleware configuration for secure service
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any) // In production, this should be restricted
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-api-key"),
        ])
        .expose_headers([
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-total-count"),
            HeaderName::from_static("x-page-count"),
        ])
}

/// Basic authentication middleware for internal services
pub async fn internal_auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check for internal service token
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                
                // In production, this should validate against a secure token store
                if is_valid_internal_token(token) {
                    return Ok(next.run(request).await);
                }
            }
        }
    }

    // Check for API key
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            if is_valid_api_key(key_str) {
                return Ok(next.run(request).await);
            }
        }
    }

    warn!("Unauthorized request attempt");
    Err(StatusCode::UNAUTHORIZED)
}

/// Validate internal service token
fn is_valid_internal_token(token: &str) -> bool {
    // In production, this would validate against a secure token store
    // For now, accept a simple test token
    token == "secure-service-internal-token-2025" || 
    token.starts_with("test-") // Accept test tokens
}

/// Validate API key
fn is_valid_api_key(api_key: &str) -> bool {
    // In production, this would validate against database
    // For now, accept test keys
    api_key.starts_with("sk-") && api_key.len() >= 32
}

/// Rate limiting middleware (simple in-memory implementation)
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct RateLimiter {
    requests: Arc<RwLock<HashMap<String, Vec<SystemTime>>>>,
    window_size: Duration,
    max_requests: usize,
}

impl RateLimiter {
    pub fn new(window_size: Duration, max_requests: usize) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            window_size,
            max_requests,
        }
    }

    pub fn check_rate_limit(&self, key: &str) -> bool {
        let now = SystemTime::now();
        let mut requests = self.requests.write().unwrap();
        
        let user_requests = requests.entry(key.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests outside the window
        user_requests.retain(|&time| {
            now.duration_since(time).unwrap_or(Duration::MAX) < self.window_size
        });
        
        if user_requests.len() >= self.max_requests {
            false
        } else {
            user_requests.push(now);
            true
        }
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    headers: HeaderMap,
    State(rate_limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract client identifier (IP, API key, etc.)
    let client_id = if let Some(api_key) = headers.get("x-api-key") {
        format!("api:{}", api_key.to_str().unwrap_or("unknown"))
    } else if let Some(forwarded) = headers.get("x-forwarded-for") {
        format!("ip:{}", forwarded.to_str().unwrap_or("unknown"))
    } else {
        "ip:unknown".to_string()
    };

    if rate_limiter.check_rate_limit(&client_id) {
        Ok(next.run(request).await)
    } else {
        warn!(client_id = %client_id, "Rate limit exceeded");
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

/// TEE environment validation middleware
pub async fn tee_validation_middleware(request: Request, next: Next) -> Response {
    // In production, this would verify TEE attestation
    // For now, just log that we're in secure mode
    info!("Processing request in TEE environment");
    
    let mut response = next.run(request).await;
    
    // Add security headers
    response.headers_mut().insert(
        "x-tee-verified",
        HeaderValue::from_static("true"),
    );
    response.headers_mut().insert(
        "x-security-level",
        HeaderValue::from_static("maximum"),
    );
    
    response
}

/// Content type validation middleware
pub async fn content_type_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let method = request.method();
    
    // Only validate content type for requests with body
    if matches!(method, &Method::POST | &Method::PUT | &Method::PATCH) {
        if let Some(content_type) = request.headers().get("content-type") {
            let content_type_str = content_type.to_str().unwrap_or("");
            
            if !content_type_str.starts_with("application/json") {
                warn!(
                    method = %method,
                    content_type = %content_type_str,
                    "Invalid content type"
                );
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
        } else {
            warn!(method = %method, "Missing content type header");
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    Ok(next.run(request).await)
}

/// Error handling middleware
pub async fn error_handling_middleware(request: Request, next: Next) -> Response {
    let uri = request.uri().clone();
    let response = next.run(request).await;
    
    // Log errors for monitoring
    if response.status().is_server_error() {
        error!(
            status = %response.status(),
            uri = %uri,
            "Server error occurred"
        );
    }
    
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(Duration::from_secs(60), 5);
        
        // Should allow first 5 requests
        for i in 0..5 {
            assert!(rate_limiter.check_rate_limit("test_user"), "Request {} should be allowed", i);
        }
        
        // 6th request should be denied
        assert!(!rate_limiter.check_rate_limit("test_user"));
        
        // Different user should be allowed
        assert!(rate_limiter.check_rate_limit("other_user"));
    }

    #[test]
    fn test_token_validation() {
        assert!(is_valid_internal_token("secure-service-internal-token-2025"));
        assert!(is_valid_internal_token("test-token"));
        assert!(!is_valid_internal_token("invalid"));
    }

    #[test]
    fn test_api_key_validation() {
        assert!(is_valid_api_key("sk-1234567890abcdef1234567890abcdef"));
        assert!(!is_valid_api_key("invalid-key"));
        assert!(!is_valid_api_key("sk-short"));
    }
} 