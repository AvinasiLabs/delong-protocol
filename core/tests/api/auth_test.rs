
use crate::common::*;
use crate::api::helpers::setup_test_environment;
use axum::http::StatusCode;
use delong_core::models::{
    auth::{SendVerificationCodeRequest, VerificationType},
    user::RegisterRequest,
};
use common::ResponseCode;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn test_send_verification_code() {
    let (router, _state) = setup_test_environment().await;
    let email = format!("testuser_{}@example.com", Uuid::new_v4());

    let req_body = SendVerificationCodeRequest {
        email: email.clone(),
        verification_type: VerificationType::AccountActivation,
        language: None,
    };

    let (status, _body_str, body_json) =
        make_request(&router, "POST", "/auth/send-code", Some(req_body), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json["code"], json!(ResponseCode::Success));
}


#[tokio::test]
async fn test_register_user_success() {
    let (router, state) = setup_test_environment().await;

    let email = format!("testuser_{}@example.com", Uuid::new_v4());
    
    // Step 1: Call the endpoint to store the verification code.
    let send_code_req = SendVerificationCodeRequest { 
        email: email.clone(),
        verification_type: VerificationType::AccountActivation,
        language: None,
    };
    let (status, _, _) = make_request(&router, "POST", "/auth/send-code", Some(send_code_req), None).await;
    assert_eq!(status, StatusCode::OK);

    // Step 2: Retrieve the code directly from the verification store.
    let verification_code = state.verification_store.get_code(&email).await.unwrap();

    // Step 3: Register the user with the correct code.
    let req_body = RegisterRequest {
        email: email.clone(),
        password: "pass1234".to_string(),
        username: format!("testuser_{}", &Uuid::new_v4().to_string()[..8]),
        verification_code: Some(verification_code),
    };

    let (status, _body_str, body_json) =
        make_request(&router, "POST", "/auth/register", Some(req_body), None).await;
    
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json["code"], json!(ResponseCode::Success));
    assert_eq!(body_json["data"]["user"]["email"], email);
}

#[tokio::test]
async fn test_login_user_success_and_fail() {
    let (router, state) = setup_test_environment().await;
    let email = format!("testuser_{}@example.com", Uuid::new_v4());
    let password = "pass1234";

    // Step 1: Create a user to log in with.
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
    let (reg_status, _, _) = make_request(&router, "POST", "/auth/register", Some(reg_req_body), None).await;
    assert_eq!(reg_status, StatusCode::OK);

    // Step 2: Test successful login
    let login_req_body = delong_core::models::user::LoginRequest {
        email: email.clone(),
        password: password.to_string(),
    };
    let (status, _body_str, body_json) =
        make_request(&router, "POST", "/auth/login", Some(login_req_body), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_json["code"], json!(ResponseCode::Success));
    assert_eq!(body_json["data"]["user"]["email"], email);
    assert!(body_json["data"]["access_token"].as_str().is_some());

    // Step 3: Test failed login (wrong password)
    let bad_login_req_body = delong_core::models::user::LoginRequest {
        email: email.clone(),
        password: "wrongpassword".to_string(),
    };
    let (status, _body_str, body_json) =
        make_request(&router, "POST", "/auth/login", Some(bad_login_req_body), None).await;
    
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body_json["code"], json!(ResponseCode::Unauthorized));
} 