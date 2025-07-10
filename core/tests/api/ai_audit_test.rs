use crate::api::helpers::setup_test_environment;
use crate::common::{generate_user_token, make_request};
use axum::http::StatusCode;
use common::ResponseCode;
use serde_json::json;
use uuid::Uuid;

use delong_core::{
    handlers::ai_audit::AiAuditRequest,
    models::{
        auth::{SendVerificationCodeRequest, VerificationType},
        user::RegisterRequest,
    },
};

#[tokio::test]
async fn test_create_ai_audit_fails_without_service_url() {
    // Ensure the env var is not set for this test.
    unsafe {
        std::env::remove_var("AI_AUDIT_SERVICE_URL");
    }

    let (router, state) = setup_test_environment().await;

    // Step 1: Create a real user to get a valid user ID for the JWT.
    let email = format!("testuser_{}@example.com", Uuid::new_v4());
    let password = "pass1234";
    let send_code_req = SendVerificationCodeRequest {
        email: email.clone(),
        verification_type: VerificationType::AccountActivation,
        language: None,
    };
    make_request(&router, "POST", "/auth/send-code", Some(send_code_req), None).await;
    let verification_code = state.verification_store.get_code(&email).await.unwrap();
    let reg_req_body = RegisterRequest {
        email: email.clone(),
        password: password.to_string(),
        username: format!("testuser_{}", &Uuid::new_v4().to_string()[..8]),
        verification_code: Some(verification_code),
    };
    let (_, _, reg_body) = make_request(&router, "POST", "/auth/register", Some(reg_req_body), None).await;
    let user_id = reg_body["data"]["user"]["id"].as_i64().unwrap().to_string();

    // Step 2: Generate a token for the real user.
    let token = generate_user_token(&user_id);

    // Step 3: Make the request
    let req_body = AiAuditRequest {
        github_url: "https://github.com/user/repo".to_string(),
        commit_hash: "a".repeat(40),
        algorithm_id: Some(1),
        execution_id: Some(1),
    };

    let (status, _body_str, body_json) =
        make_request(&router, "POST", "/api/ai-audit", Some(req_body), Some(&token)).await;

    // The handler should return a 500 when the external service is not configured.
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body_json["code"], json!(ResponseCode::InternalServerError));
} 