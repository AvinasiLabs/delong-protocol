//! HTTP client utilities for forwarding requests to backend services
//!
//! This module provides utilities for forwarding HTTP requests from the gateway
//! to backend services (Core and Secure). It handles request serialization,
//! response deserialization, and error mapping.

use async_trait::async_trait;
use reqwest::{Client, Method, Response};
use serde::{Serialize, de::DeserializeOwned};
use serde_urlencoded;
use std::collections::HashMap;

use tracing::{error, info, instrument};

/// HTTP client error types
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Serialization failed: {0}")]
    SerializationFailed(#[from] serde_json::Error),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("HTTP error {status}: {message}")]
    HttpError { status: u16, message: String },
}

/// Request builder for chaining HTTP operations
pub struct RequestBuilder {
    client: Client,
    url: String,
    method: Method,
    json_payload: Option<serde_json::Value>,
    headers: HashMap<String, String>,
}

impl RequestBuilder {
    pub fn new(client: Client, url: String, method: Method) -> Self {
        Self {
            client,
            url,
            method,
            json_payload: None,
            headers: HashMap::new(),
        }
    }

    pub fn json<T: Serialize>(mut self, payload: &T) -> Self {
        self.json_payload = Some(serde_json::to_value(payload).unwrap());
        self
    }

    pub fn query<T: Serialize>(mut self, query: &T) -> Self {
        if let Ok(query_string) = serde_urlencoded::to_string(query) {
            if self.url.contains('?') {
                self.url.push_str(&format!("&{}", query_string));
            } else {
                self.url.push_str(&format!("?{}", query_string));
            }
        }
        self
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub async fn send(self) -> Result<ResponseWrapper, HttpClientError> {
        let mut request = self.client.request(self.method, &self.url);

        if let Some(json_payload) = self.json_payload {
            request = request.json(&json_payload);
        }

        for (key, value) in self.headers {
            request = request.header(&key, &value);
        }

        let response = request.send().await?;
        Ok(ResponseWrapper { response })
    }
}

/// Response wrapper for deserializing responses
pub struct ResponseWrapper {
    response: Response,
}

impl ResponseWrapper {
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, HttpClientError> {
        let response = self.response.json::<T>().await?;
        Ok(response)
    }
}

/// Trait for backend HTTP client operations
/// Uses JSON values to maintain dyn compatibility
#[async_trait]
pub trait BackendClient: Send + Sync {
    /// Forward a request to a backend service with JSON payload and response
    async fn forward_request_json(
        &self,
        url: &str,
        method: Method,
        payload: Option<serde_json::Value>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, HttpClientError>;

    /// Create a POST request builder
    fn post(&self, url: &str) -> RequestBuilder;

    /// Create a GET request builder
    fn get(&self, url: &str) -> RequestBuilder;

    /// Create a PUT request builder
    fn put(&self, url: &str) -> RequestBuilder;

    /// Create a DELETE request builder
    fn delete(&self, url: &str) -> RequestBuilder;
}

/// Concrete HTTP client implementation for backend service communication
#[derive(Clone)]
pub struct HttpBackendClient {
    client: Client,
    timeout_seconds: u64,
}

impl HttpBackendClient {
    /// Create a new HTTP backend client
    pub fn new(timeout_seconds: u64) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            timeout_seconds,
        }
    }

    /// Get the configured timeout in seconds
    pub fn timeout_seconds(&self) -> u64 {
        self.timeout_seconds
    }

    /// Handle HTTP response and deserialize
    async fn handle_response(
        &self,
        response: Response,
    ) -> Result<serde_json::Value, HttpClientError> {
        let status = response.status();
        let url = response.url().clone();

        if status.is_success() {
            info!("Received successful response from {}", url);
            let response_data = response.json::<serde_json::Value>().await?;
            Ok(response_data)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            error!(
                "Received error response from {}: {} - {}",
                url, status, error_text
            );

            Err(HttpClientError::HttpError {
                status: status.as_u16(),
                message: error_text,
            })
        }
    }

    /// Create a POST request builder
    #[allow(dead_code)]
    fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::POST)
    }

    /// Create a GET request builder
    #[allow(dead_code)]
    fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::GET)
    }

    /// Create a PUT request builder
    #[allow(dead_code)]
    fn put(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::PUT)
    }

    /// Create a DELETE request builder
    #[allow(dead_code)]
    fn delete(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::DELETE)
    }
}

#[async_trait]
impl BackendClient for HttpBackendClient {
    /// Forward a request to a backend service
    #[instrument(skip(self, payload))]
    async fn forward_request_json(
        &self,
        url: &str,
        method: Method,
        payload: Option<serde_json::Value>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, HttpClientError> {
        info!("Forwarding {} request to {}", method, url);

        let mut request_builder = self.client.request(method, url);

        // Add custom headers
        if let Some(headers) = headers {
            for (key, value) in headers {
                request_builder = request_builder.header(key, value);
            }
        }

        // Add JSON payload if provided
        if let Some(payload) = payload {
            request_builder = request_builder.json(&payload);
        }

        let response = request_builder.send().await?;
        self.handle_response(response).await
    }

    /// Create a POST request builder
    fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::POST)
    }

    /// Create a GET request builder
    fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::GET)
    }

    /// Create a PUT request builder
    fn put(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::PUT)
    }

    /// Create a DELETE request builder
    fn delete(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(self.client.clone(), url.to_string(), Method::DELETE)
    }
}

/// Type-safe wrapper for forwarding requests with automatic serialization/deserialization
pub async fn forward_request<T, R>(
    client: &dyn BackendClient,
    url: &str,
    method: Method,
    payload: Option<T>,
    headers: Option<HashMap<String, String>>,
) -> Result<R, HttpClientError>
where
    T: Serialize,
    R: DeserializeOwned,
{
    // Serialize payload to JSON if provided
    let json_payload = if let Some(payload) = payload {
        Some(serde_json::to_value(payload)?)
    } else {
        None
    };

    // Forward request
    let json_response = client
        .forward_request_json(url, method, json_payload, headers)
        .await?;

    // Deserialize response
    let response: R = serde_json::from_value(json_response)?;
    Ok(response)
}

/// Forward GET request using provided client
pub async fn forward_get<R>(
    client: &dyn BackendClient,
    url: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<R, HttpClientError>
where
    R: DeserializeOwned,
{
    forward_request::<(), R>(client, url, Method::GET, None, headers).await
}

/// Forward POST request using provided client
pub async fn forward_post<T, R>(
    client: &dyn BackendClient,
    url: &str,
    payload: T,
    headers: Option<HashMap<String, String>>,
) -> Result<R, HttpClientError>
where
    T: Serialize,
    R: DeserializeOwned,
{
    forward_request(client, url, Method::POST, Some(payload), headers).await
}

/// Forward PUT request using provided client
pub async fn forward_put<T, R>(
    client: &dyn BackendClient,
    url: &str,
    payload: T,
    headers: Option<HashMap<String, String>>,
) -> Result<R, HttpClientError>
where
    T: Serialize,
    R: DeserializeOwned,
{
    forward_request(client, url, Method::PUT, Some(payload), headers).await
}

/// Forward DELETE request using provided client
pub async fn forward_delete<R>(
    client: &dyn BackendClient,
    url: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<R, HttpClientError>
where
    R: DeserializeOwned,
{
    forward_request::<(), R>(client, url, Method::DELETE, None, headers).await
}

/// Build query string from parameters
pub fn build_query_string(params: &HashMap<String, String>) -> String {
    if params.is_empty() {
        return String::new();
    }

    let query_parts: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect();

    format!("?{}", query_parts.join("&"))
}

/// Create authentication headers
pub fn create_auth_headers(
    api_key: Option<&str>,
    jwt_token: Option<&str>,
) -> HashMap<String, String> {
    let mut headers = HashMap::new();

    if let Some(api_key) = api_key {
        headers.insert("Authorization".to_string(), format!("ApiKey {}", api_key));
    } else if let Some(jwt_token) = jwt_token {
        headers.insert("Authorization".to_string(), format!("Bearer {}", jwt_token));
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    /// Mock backend client for testing
    pub struct MockBackendClient {
        responses: HashMap<String, serde_json::Value>,
        should_fail: bool,
    }

    impl MockBackendClient {
        pub fn new() -> Self {
            Self {
                responses: HashMap::new(),
                should_fail: false,
            }
        }

        pub fn with_response(mut self, url: &str, response: serde_json::Value) -> Self {
            self.responses.insert(url.to_string(), response);
            self
        }

        pub fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }
    }

    #[async_trait]
    impl BackendClient for MockBackendClient {
        async fn forward_request_json(
            &self,
            url: &str,
            _method: Method,
            _payload: Option<serde_json::Value>,
            _headers: Option<HashMap<String, String>>,
        ) -> Result<serde_json::Value, HttpClientError> {
            if self.should_fail {
                return Err(HttpClientError::HttpError {
                    status: 500,
                    message: "Mock error".to_string(),
                });
            }

            if let Some(response_data) = self.responses.get(url) {
                Ok(response_data.clone())
            } else {
                Err(HttpClientError::HttpError {
                    status: 404,
                    message: "Not found in mock".to_string(),
                })
            }
        }

        /// Create a POST request builder for mock testing
        fn post(&self, url: &str) -> RequestBuilder {
            // For mock client, create a dummy request builder
            // In real usage, tests should use forward_request_json directly
            RequestBuilder::new(Client::new(), url.to_string(), Method::POST)
        }

        /// Create a GET request builder for mock testing
        fn get(&self, url: &str) -> RequestBuilder {
            RequestBuilder::new(Client::new(), url.to_string(), Method::GET)
        }

        /// Create a PUT request builder for mock testing
        fn put(&self, url: &str) -> RequestBuilder {
            RequestBuilder::new(Client::new(), url.to_string(), Method::PUT)
        }

        /// Create a DELETE request builder for mock testing
        fn delete(&self, url: &str) -> RequestBuilder {
            RequestBuilder::new(Client::new(), url.to_string(), Method::DELETE)
        }
    }

    #[test]
    fn test_http_backend_client_creation() {
        let client = HttpBackendClient::new(30);
        assert_eq!(client.timeout_seconds(), 30);
    }

    #[test]
    fn test_build_query_string_empty() {
        let params = HashMap::new();
        let query = build_query_string(&params);
        assert_eq!(query, "");
    }

    #[test]
    fn test_build_query_string_single_param() {
        let mut params = HashMap::new();
        params.insert("page".to_string(), "1".to_string());
        let query = build_query_string(&params);
        assert_eq!(query, "?page=1");
    }

    #[test]
    fn test_build_query_string_multiple_params() {
        let mut params = HashMap::new();
        params.insert("page".to_string(), "1".to_string());
        params.insert("limit".to_string(), "10".to_string());
        let query = build_query_string(&params);
        // Note: HashMap iteration order is not guaranteed
        assert!(query.contains("page=1"));
        assert!(query.contains("limit=10"));
        assert!(query.starts_with('?'));
    }

    #[test]
    fn test_build_query_string_encoding() {
        let mut params = HashMap::new();
        params.insert("search".to_string(), "hello world".to_string());
        let query = build_query_string(&params);
        assert_eq!(query, "?search=hello%20world");
    }

    #[test]
    fn test_create_auth_headers_api_key() {
        let headers = create_auth_headers(Some("test-api-key"), None);
        assert_eq!(
            headers.get("Authorization"),
            Some(&"ApiKey test-api-key".to_string())
        );
    }

    #[test]
    fn test_create_auth_headers_jwt() {
        let headers = create_auth_headers(None, Some("jwt-token"));
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer jwt-token".to_string())
        );
    }

    #[test]
    fn test_create_auth_headers_api_key_priority() {
        // API key should take priority over JWT
        let headers = create_auth_headers(Some("test-api-key"), Some("jwt-token"));
        assert_eq!(
            headers.get("Authorization"),
            Some(&"ApiKey test-api-key".to_string())
        );
    }

    #[test]
    fn test_create_auth_headers_empty() {
        let headers = create_auth_headers(None, None);
        assert!(headers.is_empty());
    }

    #[test]
    fn test_http_client_error_display() {
        let error = HttpClientError::InvalidUrl("test".to_string());
        assert_eq!(error.to_string(), "Invalid URL: test");

        let error = HttpClientError::HttpError {
            status: 404,
            message: "Not Found".to_string(),
        };
        assert_eq!(error.to_string(), "HTTP error 404: Not Found");
    }

    #[tokio::test]
    async fn test_mock_backend_client_success() {
        let mock_response = json!({"id": 1, "name": "test"});
        let client = MockBackendClient::new()
            .with_response("http://test.com/api/test", mock_response.clone());

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct TestResponse {
            id: u32,
            name: String,
        }

        let result: Result<TestResponse, _> = forward_request(
            &client,
            "http://test.com/api/test",
            Method::GET,
            None::<()>,
            None,
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(response.name, "test");
    }

    #[tokio::test]
    async fn test_mock_backend_client_failure() {
        let client = MockBackendClient::new().with_failure();

        let result: Result<serde_json::Value, _> = forward_request(
            &client,
            "http://test.com/api/test",
            Method::GET,
            None::<()>,
            None,
        )
        .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            HttpClientError::HttpError { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "Mock error");
            }
            _ => panic!("Expected HttpError"),
        }
    }

    #[tokio::test]
    async fn test_forward_get_convenience_function() {
        let mock_response = json!({"success": true});
        let client =
            MockBackendClient::new().with_response("http://test.com/api/get", mock_response);

        #[derive(serde::Deserialize)]
        struct TestResponse {
            success: bool,
        }

        let result: Result<TestResponse, _> =
            forward_get(&client, "http://test.com/api/get", None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_forward_post_convenience_function() {
        let mock_response = json!({"created": true});
        let client =
            MockBackendClient::new().with_response("http://test.com/api/post", mock_response);

        #[derive(serde::Serialize)]
        struct TestPayload {
            name: String,
        }

        #[derive(serde::Deserialize)]
        struct TestResponse {
            created: bool,
        }

        let payload = TestPayload {
            name: "test".to_string(),
        };

        let result: Result<TestResponse, _> =
            forward_post(&client, "http://test.com/api/post", payload, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().created);
    }

    #[tokio::test]
    async fn test_json_serialization_roundtrip() {
        let mock_response = json!({"id": 42, "message": "hello"});
        let client =
            MockBackendClient::new().with_response("http://test.com/api/test", mock_response);

        #[derive(serde::Serialize)]
        struct TestRequest {
            name: String,
            value: i32,
        }

        #[derive(serde::Deserialize)]
        struct TestResponse {
            id: i32,
            message: String,
        }

        let request = TestRequest {
            name: "test".to_string(),
            value: 123,
        };

        let result: Result<TestResponse, _> = forward_request(
            &client,
            "http://test.com/api/test",
            Method::POST,
            Some(request),
            None,
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.id, 42);
        assert_eq!(response.message, "hello");
    }
}
