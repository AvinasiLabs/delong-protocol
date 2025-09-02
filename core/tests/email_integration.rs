//! Email Service Integration Tests
//!
//! Test real email sending functionality

mod common;

use delong_core::infra::email::EmailService;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test direct email sending using EmailService
    /// Run with: EMAIL_ENABLED=true cargo test test_direct_email_sending -- --nocapture
    #[tokio::test]
    #[ignore = "skip in ci/cd"]
    async fn test_direct_email_sending() {
        // Load config and enable email for testing
        let mut config = delong_core::Config::load().expect("Failed to load config");

        // Force enable email
        config.email.enabled = true;

        println!("📧 Email Configuration:");
        println!("   Enabled: {}", config.email.enabled);
        println!("   SMTP Host: {}", config.email.smtp_host);
        println!("   SMTP Port: {}", config.email.smtp_port);
        println!("   SMTP Username: {}", config.email.smtp_username);
        println!("   SMTP Password: {}", config.email.smtp_password);
        println!("   From Email: {}", config.email.from_email);
        println!("   From Name: {}", config.email.from_name);

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
