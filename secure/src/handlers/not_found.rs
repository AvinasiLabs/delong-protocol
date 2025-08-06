use axum::http::StatusCode;
use axum::response::Json;
use serde_json::json;

/// Handler for routes that are not found
pub async fn handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "The requested resource was not found",
            "status": 404,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    )
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::setup_test_app;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_not_found_handler() {
        let app = setup_test_app().await;

        // Test a non-existent route
        let request = Request::builder()
            .method("GET")
            .uri("/api/non-existent-route")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // Debug: print the actual response
        eprintln!(
            "Response JSON: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Verify response structure
        assert_eq!(json["error"], "not_found");
        assert_eq!(json["message"], "The requested resource was not found");
        assert_eq!(json["status"], 404);
        assert!(json["timestamp"].is_string());

        // Verify timestamp is valid RFC3339
        let timestamp = json["timestamp"].as_str().unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp).is_ok());
    }

    #[tokio::test]
    async fn test_not_found_different_methods() {
        let base_app = setup_test_app().await;

        let methods = vec!["GET", "POST", "PUT", "DELETE", "PATCH"];

        for method in methods {
            let app = base_app.clone();
            let request = Request::builder()
                .method(method)
                .uri("/api/this-does-not-exist")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);

            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let json: Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"], "not_found");
            assert_eq!(json["status"], 404);
        }
    }

    #[tokio::test]
    async fn test_not_found_with_query_params() {
        let app = setup_test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/api/missing-endpoint?param1=value1&param2=value2")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "not_found");
        assert_eq!(json["message"], "The requested resource was not found");
    }

    #[tokio::test]
    async fn test_not_found_with_path_params() {
        let app = setup_test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/api/users/123/posts/456/comments/789")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "not_found");
        assert_eq!(json["status"], 404);
    }

    #[tokio::test]
    async fn test_not_found_response_consistency() {
        let app = setup_test_app().await;

        // Make two requests to verify response is consistent
        let request1 = Request::builder()
            .method("GET")
            .uri("/api/missing1")
            .body(Body::empty())
            .unwrap();

        let response1 = app.clone().oneshot(request1).await.unwrap();
        let body1 = to_bytes(response1.into_body(), usize::MAX).await.unwrap();
        let json1: Value = serde_json::from_slice(&body1).unwrap();

        let request2 = Request::builder()
            .method("GET")
            .uri("/api/missing2")
            .body(Body::empty())
            .unwrap();

        let response2 = app.oneshot(request2).await.unwrap();
        let body2 = to_bytes(response2.into_body(), usize::MAX).await.unwrap();
        let json2: Value = serde_json::from_slice(&body2).unwrap();

        // Verify both responses have the same structure (except timestamp)
        assert_eq!(json1["error"], json2["error"]);
        assert_eq!(json1["message"], json2["message"]);
        assert_eq!(json1["status"], json2["status"]);

        // Timestamps should be different but both valid
        assert_ne!(json1["timestamp"], json2["timestamp"]);
        assert!(chrono::DateTime::parse_from_rfc3339(json1["timestamp"].as_str().unwrap()).is_ok());
        assert!(chrono::DateTime::parse_from_rfc3339(json2["timestamp"].as_str().unwrap()).is_ok());
    }
}
