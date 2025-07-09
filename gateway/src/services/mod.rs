use axum::http::HeaderMap;

pub mod http_client;

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
