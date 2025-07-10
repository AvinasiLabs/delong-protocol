
use crate::common::*;
use crate::api::helpers::setup_test_environment;
use axum::http::StatusCode;
use common::ResponseCode;
use serde_json::{json, Value};

#[tokio::test]
async fn test_health_check() {
    let (router, _state) = setup_test_environment().await;

    let (status, _body_str, body_json) =
        make_request(&router, "GET", "/health", None::<()>, None).await;

    assert_eq!(status, StatusCode::OK);

    // Assert the overall structure and the success code
    assert_eq!(body_json["code"], json!(ResponseCode::Success));

    // Assert the nested data status
    assert_eq!(body_json["data"]["status"], "healthy");
    assert_eq!(body_json["data"]["service"], "DeLong Protocol Core Service");
} 