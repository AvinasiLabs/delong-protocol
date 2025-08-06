use axum::response::Json;
use serde_json::json;

/// Basic health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "secure",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use chrono::DateTime;
    use serde_json::Value;
    use tower::ServiceExt;

    async fn app() -> Router {
        Router::new().route("/health", axum::routing::get(health_check))
    }

    #[tokio::test]
    async fn test_health_check_returns_200() {
        // Arrange
        let app = app().await;
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        // Act
        let response = app.oneshot(request).await.unwrap();

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check_response_format() {
        // Arrange
        let app = app().await;
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        // Act
        let response = app.oneshot(request).await.unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // Assert
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["service"], "secure");
        assert!(json.get("timestamp").is_some());
    }

    #[tokio::test]
    async fn test_health_check_timestamp_is_valid() {
        // Arrange
        let app = app().await;
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        // Act
        let response = app.oneshot(request).await.unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // Assert - verify timestamp can be parsed as RFC3339
        let timestamp = json["timestamp"].as_str().unwrap();
        let parsed = DateTime::parse_from_rfc3339(timestamp);
        assert!(parsed.is_ok(), "Timestamp should be valid RFC3339 format");
    }

    #[tokio::test]
    async fn test_health_check_content_type() {
        // Arrange
        let app = app().await;
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        // Act
        let response = app.oneshot(request).await.unwrap();

        // Assert
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok());

        assert_eq!(content_type, Some("application/json"));
    }

    #[tokio::test]
    async fn test_health_check_concurrent_requests() {
        use futures::future::join_all;

        // Arrange
        let base_app = app().await;

        // Create multiple requests
        let requests: Vec<_> = (0..10)
            .map(|_| {
                let app = base_app.clone();
                tokio::spawn(async move {
                    let request = Request::builder()
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap();

                    let response = app.oneshot(request).await.unwrap();

                    response.status()
                })
            })
            .collect();

        // Act
        let results = join_all(requests).await;

        // Assert - all requests should succeed
        for result in results {
            assert_eq!(result.unwrap(), StatusCode::OK);
        }
    }
}
