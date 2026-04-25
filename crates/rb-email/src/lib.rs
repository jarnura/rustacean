mod email;
mod error;
mod sender;
mod templates;

pub use email::{Email, SmtpConfig};
pub use error::EmailError;
pub use sender::{ConsoleSender, EmailSender, NoopSender, SmtpSender, from_transport};
pub use templates::EmailTemplate;

#[cfg(test)]
mod tests {
    use super::*;

    fn smtp_config() -> SmtpConfig {
        SmtpConfig {
            host: "localhost".to_string(),
            port: 587,
            username: "user@example.com".to_string(),
            password: "secret".to_string(),
            from_address: "noreply@example.com".to_string(),
        }
    }

    fn sample_email() -> Email {
        Email {
            to: "user@example.com".to_string(),
            subject: "Test".to_string(),
            text_body: "Hello".to_string(),
            html_body: "<p>Hello</p>".to_string(),
        }
    }

    // --- sender dispatch ---

    #[tokio::test]
    async fn noop_sender_returns_ok() {
        let sender = NoopSender;
        assert!(sender.send(sample_email()).await.is_ok());
    }

    #[tokio::test]
    async fn console_sender_returns_ok() {
        let sender = ConsoleSender;
        assert!(sender.send(sample_email()).await.is_ok());
    }

    #[test]
    fn from_transport_noop_succeeds() {
        let result = from_transport("noop", &smtp_config());
        assert!(result.is_ok());
    }

    #[test]
    fn from_transport_console_succeeds() {
        let result = from_transport("console", &smtp_config());
        assert!(result.is_ok());
    }

    #[test]
    fn from_transport_smtp_succeeds() {
        let result = from_transport("smtp", &smtp_config());
        assert!(result.is_ok());
    }

    #[test]
    fn from_transport_unknown_returns_err() {
        let result = from_transport("fax", &smtp_config());
        assert!(matches!(result, Err(EmailError::UnknownTransport(_))));
    }

    // --- template rendering ---

    #[test]
    fn verify_email_txt_contains_link() {
        let tpl = EmailTemplate::VerifyEmail {
            link: "https://example.com/verify/abc123".to_string(),
        };
        let (txt, _) = tpl.render().unwrap();
        assert!(txt.contains("https://example.com/verify/abc123"));
    }

    #[test]
    fn verify_email_html_contains_link() {
        let tpl = EmailTemplate::VerifyEmail {
            link: "https://example.com/verify/abc123".to_string(),
        };
        let (_, html) = tpl.render().unwrap();
        assert!(html.contains("https://example.com/verify/abc123"));
    }

    #[test]
    fn reset_password_txt_mentions_expiry() {
        let tpl = EmailTemplate::ResetPassword {
            link: "https://example.com/reset/tok".to_string(),
        };
        let (txt, _) = tpl.render().unwrap();
        assert!(txt.contains("15 minutes"));
    }

    #[test]
    fn reset_password_html_contains_link() {
        let tpl = EmailTemplate::ResetPassword {
            link: "https://example.com/reset/tok".to_string(),
        };
        let (_, html) = tpl.render().unwrap();
        assert!(html.contains("https://example.com/reset/tok"));
    }

    #[test]
    fn tenant_invite_txt_contains_tenant_name() {
        let tpl = EmailTemplate::TenantInvite {
            link: "https://example.com/invite/xyz".to_string(),
            tenant_name: "Acme Corp".to_string(),
        };
        let (txt, _) = tpl.render().unwrap();
        assert!(txt.contains("Acme Corp"));
    }

    #[test]
    fn tenant_invite_html_contains_tenant_name_and_link() {
        let tpl = EmailTemplate::TenantInvite {
            link: "https://example.com/invite/xyz".to_string(),
            tenant_name: "Acme Corp".to_string(),
        };
        let (_, html) = tpl.render().unwrap();
        assert!(html.contains("Acme Corp"));
        assert!(html.contains("https://example.com/invite/xyz"));
    }
}
