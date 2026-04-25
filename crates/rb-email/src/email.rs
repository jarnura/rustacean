/// A fully-rendered email ready for delivery.
#[derive(Debug, Clone)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: String,
}

/// SMTP connection parameters.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
}
