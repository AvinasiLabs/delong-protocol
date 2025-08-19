use avinapi::transport::response::{ApiResponse, ResponseCode};
use axum::{
    body::to_bytes,
    extract::{ConnectInfo, FromRequestParts, Request},
    http::{HeaderValue, StatusCode, request::Parts},
    middleware::Next,
    response::IntoResponse,
};
use std::net::SocketAddr;
use std::time::Instant;
use uuid::Uuid;

/// Header name for request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Request ID stored in request extensions
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

/// Client IP address extracted from request
#[derive(Clone, Debug)]
pub struct ClientIp(pub String);

impl ClientIp {
    /// Extract client IP from headers, considering proxy headers
    pub fn from_headers(headers: &axum::http::HeaderMap, connection_info: Option<&str>) -> Self {
        // Check X-Forwarded-For header (can contain multiple IPs)
        if let Some(forwarded_for) = headers.get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded_for.to_str() {
                // X-Forwarded-For can contain multiple IPs separated by commas
                // The first one is the original client IP
                if let Some(first_ip) = forwarded_str.split(',').next() {
                    return Self(first_ip.trim().to_string());
                }
            }
        }

        // Check X-Real-IP header (usually set by nginx)
        if let Some(real_ip) = headers.get("x-real-ip") {
            if let Ok(ip_str) = real_ip.to_str() {
                return Self(ip_str.to_string());
            }
        }

        // Fallback to connection info if no proxy headers found
        Self(connection_info.unwrap_or("unknown").to_string())
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RequestId {
    /// Create a new request ID
    pub fn new() -> Self {
        Self(format!("req_{}", Uuid::new_v4().simple()))
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Implement extractor for RequestId
impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Request ID not found"))
    }
}

/// Check if a path should be excluded from request logging
fn should_exclude_from_logging(path: &str) -> bool {
    matches!(
        path,
        "/docs/openapi.json" | "/docs/openapi.yaml" | "/scalar"
    )
}

/// Middleware to add request ID and enrich span context
/// Tracing automatically handles span timing - no manual time tracking needed
#[tracing::instrument(
    name = "request",
    fields(request_id, method, path, client_ip),
    skip_all
)]
pub async fn request_id_middleware(mut req: Request, next: Next) -> impl IntoResponse {
    let start_time = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Check if request already has a request ID header
    let request_id = if let Some(header_value) = req.headers().get(REQUEST_ID_HEADER) {
        // Use existing request ID if valid
        header_value
            .to_str()
            .ok()
            .map(|s| RequestId(s.to_string()))
            .unwrap_or_else(RequestId::new)
    } else {
        // Generate new request ID
        RequestId::new()
    };

    // Extract client IP address
    // Try to get connection info for direct connections
    let connection_info = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string());

    let client_ip = ClientIp::from_headers(req.headers(), connection_info.as_deref());

    // Add request ID and client IP to extensions for handlers to access
    req.extensions_mut().insert(request_id.clone());
    req.extensions_mut().insert(client_ip.clone());

    // Record request information in the current span
    let current_span = tracing::Span::current();
    current_span.record("id", &tracing::field::display(&request_id));
    current_span.record("method", &tracing::field::display(&method));
    current_span.record("path", &tracing::field::display(&path));
    current_span.record("ip", &tracing::field::display(&client_ip.0));

    // Process the request
    let mut response = next.run(req).await;
    let processing_time = start_time.elapsed();
    let status_code = response.status();

    // Smart logging: only log success when business code is SUCCESS to avoid duplication
    if !should_exclude_from_logging(&path) {
        match status_code.as_u16() {
            200..=299 => {
                // Check if this is a true business success
                let (parts, body) = response.into_parts();
                if let Ok(body_bytes) = to_bytes(body, usize::MAX).await {
                    response =
                        axum::response::Response::from_parts(parts, body_bytes.clone().into());

                    if let Ok(body_str) = String::from_utf8(body_bytes.to_vec()) {
                        if let Ok(api_response) =
                            serde_json::from_str::<ApiResponse<serde_json::Value>>(&body_str)
                        {
                            // Only log success when business code is SUCCESS
                            // Business errors already log themselves
                            if api_response.code == ResponseCode::Success {
                                tracing::info!(
                                    client_ip = %client_ip.0,
                                    "Request completed successfully"
                                );
                            }
                        } else {
                            // Non-JSON responses (static files, etc.) are considered successful
                            tracing::info!(
                                client_ip = %client_ip.0,
                                "Request completed successfully"
                            );
                        }
                    }
                } else {
                    // Failed to read body, reconstruct empty response
                    response =
                        axum::response::Response::from_parts(parts, axum::body::Body::empty());
                }
            }
            400..=499 if status_code.as_u16() != 404 => {
                tracing::warn!(
                    status_code = status_code.as_u16(),
                    status_text = %status_code.canonical_reason().unwrap_or("Unknown"),
                    processing_time_ms = processing_time.as_millis(),
                    method = %method,
                    path = %path,
                    client_ip = %client_ip.0,
                    "Client error encountered"
                );
            }
            500..=599 => {
                tracing::error!(
                    status_code = status_code.as_u16(),
                    status_text = %status_code.canonical_reason().unwrap_or("Unknown"),
                    processing_time_ms = processing_time.as_millis(),
                    method = %method,
                    path = %path,
                    client_ip = %client_ip.0,
                    "Server error encountered"
                );
            }
            _ => {
                // 404s are handled by fallback handler
            }
        }
    }

    // Log slow requests - valuable for performance monitoring
    if processing_time.as_millis() > 1000 && !should_exclude_from_logging(&path) {
        tracing::warn!(
            processing_time_ms = processing_time.as_millis(),
            client_ip = %client_ip.0,
            "Slow request detected"
        );
    }

    // Add request ID to response headers
    if let Ok(header_value) = HeaderValue::from_str(&request_id.0) {
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER, header_value);
    }

    response
}
