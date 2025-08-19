use crate::middleware::request_id::RequestId;
use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::{Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::Value;
use std::sync::Arc;

/// Response transformation layer that automatically injects request context into API responses
pub async fn response_transformer(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Extract request ID before passing the request to the next layer
    let req_id = req
        .extensions()
        .get::<RequestId>()
        .map(|id| id.as_str().to_string())
        .unwrap_or_else(|| {
            tracing::warn!("RequestId not found in extensions during response transformation");
            format!("req_{}", uuid::Uuid::new_v4().simple())
        });

    // Clone req_id for use in the response transformation
    let req_id_for_response = Arc::new(req_id);

    // Process the request
    let response = next.run(req).await;

    // Transform the response
    transform_response(response, req_id_for_response).await
}

/// Transform a response by injecting req_id into ApiResponse structures
async fn transform_response(
    response: Response<Body>,
    req_id: Arc<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let (parts, body) = response.into_parts();

    // Only process JSON responses with 200 OK status
    if parts.status != StatusCode::OK {
        return Ok(Response::from_parts(parts, body));
    }

    // Check if this is a JSON response
    let is_json = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false);

    if !is_json {
        return Ok(Response::from_parts(parts, body));
    }

    // Collect the body
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to read response body: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process response".to_string(),
            ));
        }
    };

    // Try to parse as JSON and inject req_id
    let transformed_body = transform_json_body(&bytes, &req_id)?;

    // Create new body from transformed bytes
    let new_body = Body::from(transformed_body);

    Ok(Response::from_parts(parts, new_body))
}

/// Transform JSON body by injecting req_id
fn transform_json_body(bytes: &Bytes, req_id: &str) -> Result<Bytes, (StatusCode, String)> {
    // Parse JSON
    let mut json_value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => {
            // Not valid JSON or not our structure, return as-is
            return Ok(bytes.clone());
        }
    };

    // Check if this is an ApiResponse by looking for the 'code' field
    if let Some(obj) = json_value.as_object_mut() {
        if obj.contains_key("code") {
            // This looks like our ApiResponse, inject req_id
            obj.insert("req_id".to_string(), Value::String(req_id.to_string()));
        }
    }

    // Serialize back to bytes
    match serde_json::to_vec(&json_value) {
        Ok(vec) => Ok(Bytes::from(vec)),
        Err(e) => {
            tracing::error!("Failed to serialize transformed response: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize response".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transform_json_body_with_api_response() {
        let original = json!({
            "code": "SUCCESS",
            "data": {"id": 1, "name": "Test"},
            "message": null
        });
        let bytes = Bytes::from(serde_json::to_vec(&original).unwrap());
        let req_id = "test-req-123";

        let result = transform_json_body(&bytes, req_id).unwrap();
        let transformed: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(transformed["req_id"], "test-req-123");
        assert_eq!(transformed["code"], "SUCCESS");
        assert_eq!(transformed["data"]["id"], 1);
    }

    #[test]
    fn test_transform_json_body_without_code_field() {
        let original = json!({
            "id": 1,
            "name": "Test"
        });
        let bytes = Bytes::from(serde_json::to_vec(&original).unwrap());
        let req_id = "test-req-123";

        let result = transform_json_body(&bytes, req_id).unwrap();
        let transformed: Value = serde_json::from_slice(&result).unwrap();

        // Should not inject req_id if 'code' field is missing
        assert!(!transformed.as_object().unwrap().contains_key("req_id"));
    }

    #[test]
    fn test_transform_json_body_with_error_response() {
        let original = json!({
            "code": "VALIDATION_ERROR",
            "data": null,
            "message": "Invalid input"
        });
        let bytes = Bytes::from(serde_json::to_vec(&original).unwrap());
        let req_id = "test-req-456";

        let result = transform_json_body(&bytes, req_id).unwrap();
        let transformed: Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(transformed["req_id"], "test-req-456");
        assert_eq!(transformed["code"], "VALIDATION_ERROR");
        assert_eq!(transformed["message"], "Invalid input");
    }

    #[test]
    fn test_transform_json_body_invalid_json() {
        let bytes = Bytes::from("not valid json");
        let req_id = "test-req-789";

        let result = transform_json_body(&bytes, req_id).unwrap();

        // Should return original bytes if not valid JSON
        assert_eq!(result, bytes);
    }
}
