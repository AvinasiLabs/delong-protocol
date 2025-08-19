//! Email Service Module
//!
//! This module provides functionality for sending emails including verification codes,
//! notifications, and system alerts. It supports both SMTP and mock email providers
//! for testing environments.

use crate::{AppError, AppResult};

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message, header::ContentType},
    transport::smtp::{
        PoolConfig,
        authentication::{Credentials, Mechanism},
        client::TlsParameters,
    },
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

/// Email service configuration
#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_email: String,
    pub from_name: String,
    pub use_tls: bool,
    pub use_mock: bool,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: "localhost".to_string(),
            smtp_port: 587,
            smtp_username: "".to_string(),
            smtp_password: "".to_string(),
            from_email: "noreply@example.com".to_string(),
            from_name: "DeLong Protocol".to_string(),
            use_tls: true,
            use_mock: true,
        }
    }
}

/// Email service for sending various types of emails
pub struct EmailService {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    config: EmailConfig,
}

/// Email template data
#[derive(Debug, Serialize)]
pub struct EmailTemplate {
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
}

/// Email send result
#[derive(Debug, Serialize)]
pub struct EmailSendResult {
    pub success: bool,
    pub message_id: Option<String>,
    pub error: Option<String>,
}

impl EmailService {
    /// Create a new email service instance
    pub async fn new(config: EmailConfig) -> AppResult<Self> {
        if !config.enabled {
            info!("Email service is disabled");
            return Ok(Self {
                transport: None,
                config,
            });
        }

        if config.use_mock {
            info!("Email service using mock transport");
            return Ok(Self {
                transport: None,
                config,
            });
        }

        // Create SMTP transport
        let transport = Self::create_smtp_transport(&config).await?;

        info!("Email service initialized with SMTP transport");
        Ok(Self {
            transport: Some(transport),
            config,
        })
    }

    /// Create SMTP transport with configuration
    async fn create_smtp_transport(
        config: &EmailConfig,
    ) -> AppResult<AsyncSmtpTransport<Tokio1Executor>> {
        let mut transport_builder =
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
                .port(config.smtp_port)
                .timeout(Some(Duration::from_secs(30)))
                .pool_config(PoolConfig::new().max_size(10));

        // Configure TLS
        if config.use_tls {
            let tls_params = TlsParameters::builder(config.smtp_host.clone())
                .build()
                .unwrap();
            transport_builder =
                transport_builder.tls(lettre::transport::smtp::client::Tls::Required(tls_params));
        }

        // Configure authentication
        if !config.smtp_username.is_empty() && !config.smtp_password.is_empty() {
            let credentials =
                Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());
            transport_builder = transport_builder
                .credentials(credentials)
                .authentication(vec![Mechanism::Login, Mechanism::Plain]);
        }

        let transport = transport_builder.build();

        // Test connection
        match transport.test_connection().await {
            Ok(_) => {
                info!("SMTP connection test successful");
                Ok(transport)
            }
            Err(e) => {
                error!("SMTP connection test failed: {}", e);
                Err(AppError::Internal(format!("Failed to connect to SMTP server: {}", e)))
            }
        }
    }

    /// Send verification code email
    pub async fn send_verification_code(email: &str, code: &str) -> AppResult<EmailSendResult> {
        let template = Self::create_verification_code_template(code);
        Self::send_email_static(email, &template).await
    }

    /// Send test email
    pub async fn send_test_email(email: &str) -> AppResult<EmailSendResult> {
        let template = Self::create_test_email_template();
        Self::send_email_static(email, &template).await
    }

    /// Send welcome email
    pub async fn send_welcome_email(email: &str, name: &str) -> AppResult<EmailSendResult> {
        let template = Self::create_welcome_email_template(name);
        Self::send_email_static(email, &template).await
    }

    /// Send password reset email
    pub async fn send_password_reset_email(
        email: &str,
        reset_token: &str,
    ) -> AppResult<EmailSendResult> {
        let template = Self::create_password_reset_template(reset_token);
        Self::send_email_static(email, &template).await
    }

    /// Send notification email
    pub async fn send_notification_email(
        email: &str,
        subject: &str,
        message: &str,
    ) -> AppResult<EmailSendResult> {
        let template = Self::create_notification_template(subject, message);
        Self::send_email_static(email, &template).await
    }

    /// Send AI audit completion notification
    pub async fn send_audit_completion_email(
        email: &str,
        algorithm_name: &str,
        score: i32,
        passed: bool,
    ) -> AppResult<EmailSendResult> {
        let template = Self::create_audit_completion_template(algorithm_name, score, passed);
        Self::send_email_static(email, &template).await
    }

    /// Send email using static configuration
    async fn send_email_static(
        email: &str,
        template: &EmailTemplate,
    ) -> AppResult<EmailSendResult> {
        // Try to get email configuration from environment
        let config = EmailConfig {
            enabled: std::env::var("EMAIL_ENABLED")
                .map(|v| v.parse().unwrap_or(false))
                .unwrap_or(false),
            smtp_host: std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string()),
            smtp_port: std::env::var("SMTP_PORT")
                .map(|v| v.parse().unwrap_or(587))
                .unwrap_or(587),
            smtp_username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
            smtp_password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
            from_email: std::env::var("FROM_EMAIL")
                .unwrap_or_else(|_| "noreply@delong.com".to_string()),
            from_name: std::env::var("FROM_NAME").unwrap_or_else(|_| "DeLong Protocol".to_string()),
            use_tls: std::env::var("SMTP_USE_TLS")
                .map(|v| v.parse().unwrap_or(true))
                .unwrap_or(true),
            use_mock: std::env::var("EMAIL_USE_MOCK")
                .map(|v| v.parse().unwrap_or(true))
                .unwrap_or(true),
        };

        if !config.enabled || config.use_mock {
            info!("Mock email sent to {}: {}", email, template.subject);
            return Ok(EmailSendResult {
                success: true,
                message_id: Some(format!("mock-{}", Uuid::new_v4())),
                error: None,
            });
        }

        // Create email service and send
        let email_service = Self::new(config).await?;
        email_service.send_email(email, template).await
    }

    /// Send email using instance configuration
    pub async fn send_email(
        &self,
        to_email: &str,
        template: &EmailTemplate,
    ) -> AppResult<EmailSendResult> {
        if !self.config.enabled {
            return Err(AppError::Internal("Email service is disabled".to_string()));
        }

        if self.config.use_mock {
            info!("Mock email sent to {}: {}", to_email, template.subject);
            return Ok(EmailSendResult {
                success: true,
                message_id: Some(format!("mock-{}", Uuid::new_v4())),
                error: None,
            });
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| AppError::Internal("SMTP transport not available".to_string()))?;

        // Create message
        let message = self.create_message(to_email, template)?;

        // Send email via SMTP
        match transport.send(message).await {
            Ok(_response) => {
                info!(
                    "Email sent successfully to {}: {}",
                    to_email, template.subject
                );
                Ok(EmailSendResult {
                    success: true,
                    message_id: Some(format!("sent-{}", uuid::Uuid::new_v4())),
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to send email to {}: {}", to_email, e);
                Ok(EmailSendResult {
                    success: false,
                    message_id: None,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Create email message
    fn create_message(&self, to_email: &str, template: &EmailTemplate) -> AppResult<Message> {
        let from_mailbox = format!("{} <{}>", self.config.from_name, self.config.from_email)
            .parse::<Mailbox>()
            .map_err(|_| AppError::Validation("Invalid from email format".to_string()))?;

        let to_mailbox = to_email
            .parse::<Mailbox>()
            .map_err(|_| AppError::Validation("Invalid to email format".to_string()))?;

        let message_builder = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(&template.subject);

        // Add message body
        let message = if let Some(html_body) = &template.body_html {
            message_builder
                .multipart(
                    lettre::message::MultiPart::alternative()
                        .singlepart(
                            lettre::message::SinglePart::builder()
                                .header(ContentType::TEXT_PLAIN)
                                .body(template.body_text.clone()),
                        )
                        .singlepart(
                            lettre::message::SinglePart::builder()
                                .header(ContentType::TEXT_HTML)
                                .body(html_body.clone()),
                        ),
                )
                .map_err(|_| AppError::Validation("Invalid multipart message".to_string()))?
        } else {
            message_builder
                .body(template.body_text.clone())
                .map_err(|_| AppError::Validation("Invalid message body".to_string()))?
        };

        Ok(message)
    }

    /// Create verification code email template
    fn create_verification_code_template(code: &str) -> EmailTemplate {
        let subject = "DeLong Protocol - Email Verification Code".to_string();
        let body_text = format!(
            "Your verification code is: {}\n\nThis code will expire in 10 minutes.\n\nIf you didn't request this code, please ignore this email.",
            code
        );
        let body_html = Some(format!(
            r#"
            <html>
            <body>
                <h2>DeLong Protocol - Email Verification</h2>
                <p>Your verification code is:</p>
                <h1 style="color: #007bff; font-size: 32px; text-align: center; background-color: #f8f9fa; padding: 20px; border-radius: 5px;">{}</h1>
                <p>This code will expire in 10 minutes.</p>
                <p>If you didn't request this code, please ignore this email.</p>
                <hr>
                <p><small>This is an automated message from DeLong Protocol.</small></p>
            </body>
            </html>
            "#,
            code
        ));

        EmailTemplate {
            subject,
            body_text,
            body_html,
        }
    }

    /// Create test email template
    fn create_test_email_template() -> EmailTemplate {
        let subject = "DeLong Protocol - Test Email".to_string();
        let body_text = "This is a test email from DeLong Protocol.\n\nIf you received this email, the email service is working correctly.".to_string();
        let body_html = Some(
            r#"
            <html>
            <body>
                <h2>DeLong Protocol - Test Email</h2>
                <p>This is a test email from DeLong Protocol.</p>
                <p>If you received this email, the email service is working correctly.</p>
                <hr>
                <p><small>This is an automated test message.</small></p>
            </body>
            </html>
            "#
            .to_string(),
        );

        EmailTemplate {
            subject,
            body_text,
            body_html,
        }
    }

    /// Create welcome email template
    fn create_welcome_email_template(name: &str) -> EmailTemplate {
        let subject = "Welcome to DeLong Protocol!".to_string();
        let body_text = format!(
            "Welcome to DeLong Protocol, {}!\n\nThank you for joining our community. You can now start uploading algorithms, participating in votes, and contributing to the ecosystem.\n\nBest regards,\nThe DeLong Protocol Team",
            name
        );
        let body_html = Some(format!(
            r#"
            <html>
            <body>
                <h2>Welcome to DeLong Protocol!</h2>
                <p>Welcome to DeLong Protocol, {}!</p>
                <p>Thank you for joining our community. You can now start:</p>
                <ul>
                    <li>Uploading algorithms</li>
                    <li>Participating in votes</li>
                    <li>Contributing to the ecosystem</li>
                </ul>
                <p>Best regards,<br>The DeLong Protocol Team</p>
                <hr>
                <p><small>This is an automated welcome message.</small></p>
            </body>
            </html>
            "#,
            name
        ));

        EmailTemplate {
            subject,
            body_text,
            body_html,
        }
    }

    /// Create password reset email template
    fn create_password_reset_template(reset_token: &str) -> EmailTemplate {
        let subject = "DeLong Protocol - Password Reset".to_string();
        let body_text = format!(
            "You requested a password reset for your DeLong Protocol account.\n\nYour reset token is: {}\n\nThis token will expire in 1 hour.\n\nIf you didn't request this reset, please ignore this email.",
            reset_token
        );
        let body_html = Some(format!(
            r#"
            <html>
            <body>
                <h2>DeLong Protocol - Password Reset</h2>
                <p>You requested a password reset for your DeLong Protocol account.</p>
                <p>Your reset token is:</p>
                <p style="font-family: monospace; font-size: 18px; background-color: #f8f9fa; padding: 10px; border-radius: 5px;">{}</p>
                <p>This token will expire in 1 hour.</p>
                <p>If you didn't request this reset, please ignore this email.</p>
                <hr>
                <p><small>This is an automated message from DeLong Protocol.</small></p>
            </body>
            </html>
            "#,
            reset_token
        ));

        EmailTemplate {
            subject,
            body_text,
            body_html,
        }
    }

    /// Create notification email template
    fn create_notification_template(subject: &str, message: &str) -> EmailTemplate {
        let body_text = format!(
            "{}\n\n---\nThis is an automated notification from DeLong Protocol.",
            message
        );
        let body_html = Some(format!(
            r#"
            <html>
            <body>
                <h2>DeLong Protocol - Notification</h2>
                <p>{}</p>
                <hr>
                <p><small>This is an automated notification from DeLong Protocol.</small></p>
            </body>
            </html>
            "#,
            message.replace('\n', "<br>")
        ));

        EmailTemplate {
            subject: subject.to_string(),
            body_text,
            body_html,
        }
    }

    /// Create audit completion email template
    fn create_audit_completion_template(
        algorithm_name: &str,
        score: i32,
        passed: bool,
    ) -> EmailTemplate {
        let status = if passed { "PASSED" } else { "FAILED" };
        let subject = format!("AI Audit Complete: {} - {}", algorithm_name, status);

        let body_text = format!(
            "AI Audit Complete for: {}\n\nScore: {}/100\nStatus: {}\n\nYou can view the detailed audit report in your dashboard.\n\nBest regards,\nThe DeLong Protocol Team",
            algorithm_name, score, status
        );

        let status_color = if passed { "#28a745" } else { "#dc3545" };
        let body_html = Some(format!(
            r#"
            <html>
            <body>
                <h2>AI Audit Complete</h2>
                <p><strong>Algorithm:</strong> {}</p>
                <p><strong>Score:</strong> {}/100</p>
                <p><strong>Status:</strong> <span style="color: {}; font-weight: bold;">{}</span></p>
                <p>You can view the detailed audit report in your dashboard.</p>
                <p>Best regards,<br>The DeLong Protocol Team</p>
                <hr>
                <p><small>This is an automated notification from DeLong Protocol.</small></p>
            </body>
            </html>
            "#,
            algorithm_name, score, status_color, status
        ));

        EmailTemplate {
            subject,
            body_text,
            body_html,
        }
    }

    /// Test email service connectivity
    pub async fn test_connection(&self) -> AppResult<()> {
        if !self.config.enabled {
            return Err(AppError::Internal("Email service is disabled".to_string()));
        }

        if self.config.use_mock {
            info!("Mock email service connection test successful");
            return Ok(());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| AppError::Internal("SMTP transport not available".to_string()))?;

        transport
            .test_connection()
            .await
            .map_err(|e| AppError::Internal(format!("SMTP connection test failed: {}", e)))?;

        info!("Email service connection test successful");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_default() {
        let config = EmailConfig::default();
        assert!(!config.enabled);
        assert!(config.use_mock);
        assert_eq!(config.smtp_port, 587);
        assert!(config.use_tls);
    }

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
        assert!(template.body_html.is_some());
    }

    #[test]
    fn test_create_welcome_email_template() {
        let template = EmailService::create_welcome_email_template("John Doe");
        assert!(template.subject.contains("Welcome"));
        assert!(template.body_text.contains("John Doe"));
        assert!(template.body_html.is_some());
    }

    #[test]
    fn test_create_password_reset_template() {
        let template = EmailService::create_password_reset_template("reset123");
        assert!(template.subject.contains("Password Reset"));
        assert!(template.body_text.contains("reset123"));
        assert!(template.body_html.is_some());
    }

    #[test]
    fn test_create_notification_template() {
        let template = EmailService::create_notification_template("Test Subject", "Test Message");
        assert_eq!(template.subject, "Test Subject");
        assert!(template.body_text.contains("Test Message"));
        assert!(template.body_html.is_some());
    }

    #[test]
    fn test_create_audit_completion_template_passed() {
        let template = EmailService::create_audit_completion_template("MyAlgorithm", 85, true);
        assert!(template.subject.contains("PASSED"));
        assert!(template.body_text.contains("85/100"));
        assert!(template.body_html.is_some());
    }

    #[test]
    fn test_create_audit_completion_template_failed() {
        let template = EmailService::create_audit_completion_template("MyAlgorithm", 45, false);
        assert!(template.subject.contains("FAILED"));
        assert!(template.body_text.contains("45/100"));
        assert!(template.body_html.is_some());
    }

    #[tokio::test]
    async fn test_send_email_static_mock() {
        // Set mock environment
        unsafe {
            std::env::set_var("EMAIL_USE_MOCK", "true");
        }

        let template = EmailService::create_test_email_template();
        let result = EmailService::send_email_static("test@example.com", &template).await;

        assert!(result.is_ok());
        let email_result = result.unwrap();
        assert!(email_result.success);
        assert!(email_result.message_id.is_some());
        assert!(email_result.error.is_none());
    }

    #[tokio::test]
    async fn test_email_service_new_mock() {
        let config = EmailConfig {
            enabled: true,
            use_mock: true,
            ..Default::default()
        };

        let service = EmailService::new(config).await;
        assert!(service.is_ok());

        let service = service.unwrap();
        assert!(service.transport.is_none());
    }

    #[tokio::test]
    async fn test_email_service_new_disabled() {
        let config = EmailConfig {
            enabled: false,
            ..Default::default()
        };

        let service = EmailService::new(config).await;
        assert!(service.is_ok());

        let service = service.unwrap();
        assert!(service.transport.is_none());
    }
}
