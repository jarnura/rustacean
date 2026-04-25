# rb-email

Transactional email for rust-brain. Provides an `EmailSender` trait with three transports and minijinja-powered HTML + plain-text templates.

## Transports

| Transport | Env value | Behaviour |
|-----------|-----------|-----------|
| `SmtpSender` | `smtp` | Delivers via SMTP with rustls TLS |
| `ConsoleSender` | `console` | Prints `[EMAIL]` banner to stdout (default for local dev) |
| `NoopSender` | `noop` | Silently discards (useful in tests) |

Select the transport via `RB_EMAIL_TRANSPORT` (default: `console`).

## Quick start

```rust
use rb_email::{EmailTemplate, SmtpConfig, from_transport};

let transport = std::env::var("RB_EMAIL_TRANSPORT")
    .unwrap_or_else(|_| "console".to_string());

let smtp = SmtpConfig {
    host: "smtp.example.com".to_string(),
    port: 587,
    username: "user".to_string(),
    password: "secret".to_string(),
    from_address: "noreply@example.com".to_string(),
};

let sender = from_transport(&transport, &smtp)?;

let email = EmailTemplate::VerifyEmail {
    link: "https://app.example.com/verify/TOKEN".to_string(),
}.to_email("new-user@example.com")?;

sender.send(email).await?;
```

## Templates

All templates live in `crates/rb-email/templates/` and are embedded at compile time.

| Template | Variables |
|----------|-----------|
| `verify-email.{txt,html}` | `link` |
| `reset-password.{txt,html}` | `link` |
| `tenant-invite.{txt,html}` | `link`, `tenant_name` |

## Dependencies

Standalone — no other `rb-*` crates required.
