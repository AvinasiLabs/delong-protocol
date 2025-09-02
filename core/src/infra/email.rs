//! Email service module for sending verification codes and notifications
//!
//! This module provides email sending functionality using SMTP.

use crate::{AppResult, config::EmailConfig};
use mail_builder::MessageBuilder;
use mail_send::SmtpClientBuilder;
use tracing::{error, info};

/// Email service for sending emails via SMTP
#[derive(Clone)]
pub struct EmailService {
    config: EmailConfig,
}

/// Email template for rendering
#[derive(Debug, Clone)]
pub struct EmailTemplate {
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
}

impl EmailService {
    /// Create a new EmailService instance
    pub async fn new(config: EmailConfig) -> AppResult<Self> {
        if !config.enabled {
            info!("Email service is disabled");
        }
        Ok(Self { config })
    }

    /// Send verification code email
    pub async fn send_verification_code(
        config: &EmailConfig,
        email: &str,
        code: &str,
    ) -> AppResult<()> {
        let template = Self::create_verification_code_template(code);
        Self::send_email_with_config(config, email, &template).await
    }

    /// Send test email
    pub async fn send_test_email(config: &EmailConfig, email: &str) -> AppResult<()> {
        let template = Self::create_test_email_template();
        Self::send_email_with_config(config, email, &template).await
    }

    /// Send welcome email
    pub async fn send_welcome_email(
        config: &EmailConfig,
        email: &str,
        username: &str,
    ) -> AppResult<()> {
        let template = Self::create_welcome_email_template(username);
        Self::send_email_with_config(config, email, &template).await
    }

    /// Send password reset email
    pub async fn send_password_reset_email(
        config: &EmailConfig,
        email: &str,
        reset_token: &str,
    ) -> AppResult<()> {
        let template = Self::create_password_reset_template(reset_token);
        Self::send_email_with_config(config, email, &template).await
    }

    /// Send notification email
    pub async fn send_notification_email(
        config: &EmailConfig,
        email: &str,
        subject: &str,
        message: &str,
    ) -> AppResult<()> {
        let template = Self::create_notification_template(subject, message);
        Self::send_email_with_config(config, email, &template).await
    }

    /// Send AI audit completion email
    pub async fn send_audit_completion_email(
        config: &EmailConfig,
        email: &str,
        algorithm_name: &str,
        score: u32,
        passed: bool,
    ) -> AppResult<()> {
        let template = Self::create_audit_completion_template(algorithm_name, score, passed);
        Self::send_email_with_config(config, email, &template).await
    }

    /// Send email using configuration
    pub async fn send_email_with_config(
        config: &EmailConfig,
        to_email: &str,
        template: &EmailTemplate,
    ) -> AppResult<()> {
        // Check if email is disabled
        if !config.enabled {
            info!(
                "Email service disabled - simulating send to {}: {}",
                to_email, template.subject
            );
            return Ok(());
        }

        // Create email service and send
        let email_service = Self::new(config.clone()).await?;
        email_service.send_email(to_email, template).await
    }

    /// Send email via SMTP
    pub async fn send_email(&self, to_email: &str, template: &EmailTemplate) -> AppResult<()> {
        if !self.config.enabled {
            info!(
                "Email service disabled - simulating send to {}: {}",
                to_email, template.subject
            );
            return Ok(());
        }

        // Build a simple multipart message
        let mut message = MessageBuilder::new()
            .from((
                self.config.from_name.as_str(),
                self.config.from_email.as_str(),
            ))
            .to(to_email)
            .subject(&template.subject)
            .text_body(&template.body_text);

        // Add HTML body if available
        if let Some(html) = &template.body_html {
            message = message.html_body(html);
        }

        // Try to send email
        let send_result = async {
            // Connect to the SMTP submissions port, upgrade to TLS and
            // authenticate using the provided credentials.
            SmtpClientBuilder::new(self.config.smtp_host.as_str(), 587)
                .implicit_tls(false)
                .credentials((
                    self.config.smtp_username.as_str(),
                    self.config.smtp_password.as_str(),
                ))
                .connect()
                .await?
                .send(message)
                .await
        }
        .await;

        match send_result {
            Ok(_) => {
                info!(
                    "Email sent successfully to {}: {}",
                    to_email, template.subject
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to send email to {}: {}", to_email, e);
                Err(crate::AppError::Internal(format!(
                    "Failed to send email: {}",
                    e
                )))
            }
        }
    }

    /// Create verification code email template
    pub fn create_verification_code_template(code: &str) -> EmailTemplate {
        let subject = "DeLong Hub - Verification Code".to_string();
        let body_text = format!(
            "Your DeLong Hub verification code is: {}\n\n\
             This code will expire in 15 minutes.\n\n\
             If you didn't request this verification, please ignore this email.",
            code
        );
        let body_html = Some(format!(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <meta charset="UTF-8">
                <title>Verification Code</title>
            </head>
            <body style="font-family: Arial, sans-serif; line-height: 1.6; color: #333;">
                <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
                    <h2>DeLong Hub - Verification Code</h2>
                    <p>Your verification code is:</p>
                    <div style="font-size: 24px; font-weight: bold; color: #007bff; padding: 10px; background: #f8f9fa; text-align: center; margin: 20px 0;">
                        {}
                    </div>
                    <p>This code will expire in 15 minutes.</p>
                    <p style="color: #666; font-size: 14px;">If you didn't request this verification, please ignore this email.</p>
                </div>
            </body>
            </html>"#,
            code
        ));
        EmailTemplate {
            subject,
            body_text,
            body_html,
        }
    }

    /// Create test email template
    pub fn create_test_email_template() -> EmailTemplate {
        EmailTemplate {
            subject: "DeLong Hub - Test Email".to_string(),
            body_text: "This is a test email from DeLong Hub.\n\n\
                       If you received this email, the email service is working correctly."
                .to_string(),
            body_html: Some(
                r#"<!DOCTYPE html>
                <html>
                <body style="font-family: Arial, sans-serif;">
                    <h2>Test Email</h2>
                    <p>This is a test email from DeLong Hub.</p>
                    <p>If you received this email, the email service is working correctly.</p>
                </body>
                </html>"#
                    .to_string(),
            ),
        }
    }

    /// Create welcome email template
    pub fn create_welcome_email_template(username: &str) -> EmailTemplate {
        EmailTemplate {
            subject: "Welcome to DeLong Hub".to_string(),
            body_text: format!(
                "Welcome {}, \n\n\
                 Thank you for joining DeLong Hub, the decentralized AI audit platform.\n\n\
                 You can now submit AI algorithms for audit and participate in the committee voting process.\n\n\
                 Best regards,\n\
                 The DeLong Hub Team",
                username
            ),
            body_html: Some(format!(
                r#"<!DOCTYPE html>
                <html>
                <body style="font-family: Arial, sans-serif;">
                    <h2>Welcome to DeLong Hub</h2>
                    <p>Dear {},</p>
                    <p>Thank you for joining DeLong Hub, the decentralized AI audit platform.</p>
                    <p>You can now:</p>
                    <ul>
                        <li>Submit AI algorithms for audit</li>
                        <li>Participate in the committee voting process</li>
                        <li>View audit reports and results</li>
                    </ul>
                    <p>Best regards,<br/>The DeLong Hub Team</p>
                </body>
                </html>"#,
                username
            )),
        }
    }

    /// Create password reset email template
    pub fn create_password_reset_template(reset_token: &str) -> EmailTemplate {
        EmailTemplate {
            subject: "DeLong Hub - Password Reset Request".to_string(),
            body_text: format!(
                "You requested a password reset for your DeLong Hub account.\n\n\
                 Your password reset code is: {}\n\n\
                 This code will expire in 15 minutes.\n\n\
                 If you didn't request this reset, please ignore this email and your password will remain unchanged.",
                reset_token
            ),
            body_html: Some(format!(
                r#"<!DOCTYPE html>
                <html>
                <body style="font-family: Arial, sans-serif;">
                    <h2>Password Reset Request</h2>
                    <p>You requested a password reset for your DeLong Hub account.</p>
                    <p>Your password reset code is:</p>
                    <div style="font-size: 24px; font-weight: bold; color: #dc3545; padding: 10px; background: #f8f9fa; text-align: center;">
                        {}
                    </div>
                    <p>This code will expire in 15 minutes.</p>
                    <p style="color: #666;">If you didn't request this reset, please ignore this email.</p>
                </body>
                </html>"#,
                reset_token
            )),
        }
    }

    /// Create notification email template
    pub fn create_notification_template(subject: &str, message: &str) -> EmailTemplate {
        EmailTemplate {
            subject: subject.to_string(),
            body_text: message.to_string(),
            body_html: Some(format!(
                r#"<!DOCTYPE html>
                <html>
                <body style="font-family: Arial, sans-serif;">
                    <h2>{}</h2>
                    <div style="padding: 20px; background: #f8f9fa;">
                        <p>{}</p>
                    </div>
                </body>
                </html>"#,
                subject, message
            )),
        }
    }

    /// Create AI audit completion email template
    pub fn create_audit_completion_template(
        algorithm_name: &str,
        score: u32,
        passed: bool,
    ) -> EmailTemplate {
        let status = if passed { "PASSED" } else { "FAILED" };
        let color = if passed { "#28a745" } else { "#dc3545" };

        EmailTemplate {
            subject: format!("AI Audit Complete - {}", algorithm_name),
            body_text: format!(
                "Your AI algorithm '{}' has completed the audit process.\n\n\
                 Audit Score: {}%\n\
                 Status: {}\n\n\
                 You can view the detailed audit report in your dashboard.",
                algorithm_name, score, status
            ),
            body_html: Some(format!(
                r#"<!DOCTYPE html>
                <html>
                <body style="font-family: Arial, sans-serif;">
                    <h2>AI Audit Complete</h2>
                    <p>Your AI algorithm '<strong>{}</strong>' has completed the audit process.</p>
                    <div style="padding: 20px; background: #f8f9fa; margin: 20px 0;">
                        <p><strong>Audit Score:</strong> {}%</p>
                        <p><strong>Status:</strong> <span style="color: {};">{}</span></p>
                    </div>
                    <p>You can view the detailed audit report in your dashboard.</p>
                </body>
                </html>"#,
                algorithm_name, score, color, status
            )),
        }
    }

    /// Test SMTP connection
    pub async fn test_connection(&self) -> AppResult<bool> {
        if !self.config.enabled {
            info!("Email service is disabled");
            return Ok(false);
        }

        info!("Testing SMTP connection");

        // Try to connect without sending
        match SmtpClientBuilder::new(self.config.smtp_host.as_str(), 587)
            .implicit_tls(false)
            .credentials((
                self.config.smtp_username.as_str(),
                self.config.smtp_password.as_str(),
            ))
            .connect()
            .await
        {
            Ok(_) => {
                info!("SMTP connection test successful");
                Ok(true)
            }
            Err(e) => {
                error!("SMTP connection test failed: {}", e);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_verification_code_template() {
        let template = EmailService::create_verification_code_template("123456");
        assert!(template.subject.contains("Verification Code"));
        assert!(template.body_text.contains("123456"));
        assert!(template.body_html.is_some());
    }

    #[test]
    fn test_create_test_email_template() {
        let template = EmailService::create_test_email_template();
        assert!(template.subject.contains("Test Email"));
        assert!(template.body_text.contains("test email"));
    }

    #[test]
    fn test_create_welcome_email_template() {
        let template = EmailService::create_welcome_email_template("TestUser");
        assert!(template.subject.contains("Welcome"));
        assert!(template.body_text.contains("TestUser"));
    }

    #[test]
    fn test_create_password_reset_template() {
        let template = EmailService::create_password_reset_template("reset123");
        assert!(template.subject.contains("Password Reset"));
        assert!(template.body_text.contains("reset123"));
    }

    #[test]
    fn test_create_notification_template() {
        let template = EmailService::create_notification_template("Test Subject", "Test Message");
        assert_eq!(template.subject, "Test Subject");
        assert_eq!(template.body_text, "Test Message");
    }

    #[test]
    fn test_create_audit_completion_template_passed() {
        let template = EmailService::create_audit_completion_template("TestAlgo", 85, true);
        assert!(template.subject.contains("TestAlgo"));
        assert!(template.body_text.contains("PASSED"));
    }

    #[test]
    fn test_create_audit_completion_template_failed() {
        let template = EmailService::create_audit_completion_template("TestAlgo", 45, false);
        assert!(template.subject.contains("TestAlgo"));
        assert!(template.body_text.contains("FAILED"));
    }

    #[tokio::test]
    async fn test_send_email_disabled() {
        let config = EmailConfig {
            enabled: false,
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            from_email: "test@example.com".to_string(),
            from_name: "Test".to_string(),
        };

        let result =
            EmailService::send_verification_code(&config, "test@example.com", "123456").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_email_service_new_enabled() {
        let config = EmailConfig {
            enabled: true,
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            smtp_username: "test@example.com".to_string(),
            smtp_password: "password".to_string(),
            from_email: "test@example.com".to_string(),
            from_name: "Test".to_string(),
        };

        let service = EmailService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_email_service_new_disabled() {
        let config = EmailConfig {
            enabled: false,
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            from_email: "test@example.com".to_string(),
            from_name: "Test".to_string(),
        };

        let service = EmailService::new(config).await;
        assert!(service.is_ok());
    }
}
