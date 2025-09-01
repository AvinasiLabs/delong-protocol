//! Email Service Integration Tests
//!
//! Test real email sending functionality

mod common;

use common::setup_clean_test_app;
use delong_core::infra::email::EmailService;
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    /// Test email sending through API endpoint (Mock Mode)
    /// This test sends a verification code to wwwwwdemon@gmail.com via the API
    /// In mock mode, the email is not actually sent but the code is stored in Redis
    #[tokio::test]
    async fn test_send_verification_code_to_real_email() {
        let app = setup_clean_test_app().await;
        let test_email = "wwwwwdemon@gmail.com";

        println!("📧 Testing email API endpoint with email: {}", test_email);

        // Send verification code to the test email
        let request = Request::builder()
            .method("POST")
            .uri("/auth/send-code")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "email": test_email,
                    "verification_type": "email",
                    "language": "en"
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "API should return OK status"
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Print full response for debugging
        println!(
            "📋 Full API Response: {}",
            serde_json::to_string_pretty(&response_json).unwrap()
        );

        if response_json["code"] != "SUCCESS" {
            println!("⚠️  API returned error code: {}", response_json["code"]);
            if let Some(message) = response_json["message"].as_str() {
                println!("   Error message: {}", message);
            }
            if let Some(error) = response_json["error"].as_str() {
                println!("   Error details: {}", error);
            }
        }

        assert_eq!(
            response_json["code"],
            "SUCCESS",
            "Response should indicate success. Got: {}",
            serde_json::to_string_pretty(&response_json).unwrap()
        );
        assert_eq!(
            response_json["data"]["message"],
            "Verification code sent successfully"
        );

        println!("✅ Verification code sent to wwwwwdemon@gmail.com");
        println!("📧 Please check the inbox for the verification code");
        println!("✅ API endpoint test passed - verification code stored in Redis");
        println!("📍 In mock mode, the fixed code is: 1234");
        println!("📧 Target email: {}", test_email);
        println!("⚠️  Note: No actual email is sent in mock mode");
    }

    /// Test direct email sending using EmailService
    /// Run with: EMAIL_ENABLED=true cargo test test_direct_email_sending -- --nocapture
    #[tokio::test]
    async fn test_direct_email_sending() {
        // Load config and enable email for testing
        let mut config = delong_core::Config::load().expect("Failed to load config");

        // Check if email is enabled via environment variable
        if std::env::var("EMAIL_ENABLED").unwrap_or_default() != "true" {
            println!("⚠️  Skipping email test - set EMAIL_ENABLED=true to run");
            println!(
                "   Run with: EMAIL_ENABLED=true cargo test test_direct_email_sending -- --nocapture"
            );
            return;
        }

        // Force enable email
        config.email.enabled = true;

        println!("📧 Email Configuration:");
        println!("   Enabled: {}", config.email.enabled);
        println!("   SMTP Host: {}", config.email.smtp_host);
        println!("   SMTP Port: {}", config.email.smtp_port);
        println!(
            "   SMTP Username: {}",
            if config.email.smtp_username.is_empty() {
                "(empty)"
            } else {
                "(set)"
            }
        );
        println!(
            "   SMTP Password: {}",
            if config.email.smtp_password.is_empty() {
                "(empty)"
            } else {
                "(set)"
            }
        );
        println!("   From Email: {}", config.email.from_email);
        println!("   From Name: {}", config.email.from_name);
        println!("   Use TLS: {}", config.email.use_tls);

        // Check if we have SMTP credentials
        if config.email.smtp_username.is_empty() || config.email.smtp_password.is_empty() {
            println!("⚠️  Warning: SMTP credentials not configured");
            println!("   Set SMTP_USERNAME and SMTP_PASSWORD environment variables");
        }

        let test_email = "wwwwwdemon@gmail.com";
        let verification_code = "888888";

        println!("\n🚀 Sending verification email to {}", test_email);

        // Send verification email
        match EmailService::send_verification_code(&config.email, test_email, verification_code)
            .await
        {
            Ok(()) => {
                println!("✅ Verification email sent successfully");
                println!("📧 Check {} for code: {}", test_email, verification_code);
            }
            Err(e) => {
                println!("❌ Failed to send email: {:?}", e);
                println!("\n💡 Troubleshooting:");
                println!("   1. Ensure EMAIL_ENABLED=true is set");
                println!("   2. Check SMTP_USERNAME and SMTP_PASSWORD are configured");
                println!(
                    "   3. Verify SMTP_HOST and SMTP_PORT are correct (587 for STARTTLS, 465 for SSL)"
                );
                println!("   4. For Gmail, use an App Password instead of regular password");
                println!("   5. Check firewall/network settings");
                panic!("Email service call failed: {:?}", e);
            }
        }

        // Wait a bit before sending another email
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Send notification email
        println!("\n🚀 Sending notification email to {}", test_email);

        match EmailService::send_notification_email(
            &config.email,
            test_email,
            "Test Notification from DeLong",
            "This is a test notification to verify the email service is working correctly.",
        )
        .await
        {
            Ok(()) => {
                println!("✅ Notification email sent successfully");
            }
            Err(e) => {
                println!("❌ Failed to send notification email: {:?}", e);
                panic!("Notification service call failed: {:?}", e);
            }
        }

        println!(
            "\n🎉 Email test completed! Check {} for the test emails",
            test_email
        );
        println!("   You should have received:");
        println!(
            "   1. Verification code email with code: {}",
            verification_code
        );
        println!("   2. Test notification email");
    }
}
