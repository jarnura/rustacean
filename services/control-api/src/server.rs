use std::sync::Arc;

use anyhow::{Context as _, Result};
use base64::Engine as _;
use jsonwebtoken::EncodingKey;
use rb_auth::{LoginRateLimiter, PasswordHasher};
use rb_email::{SmtpConfig, from_transport};
use rb_github::{GhApp, Secret};
use sqlx::postgres::PgPoolOptions;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::{config::Config, routes, state::AppState};

/// Connects to Postgres, builds [`AppState`], and drives the server until shutdown.
///
/// # Errors
///
/// Returns an error if the database connection fails, the TCP listener cannot
/// bind, or axum returns an IO error during serving.
pub async fn run(config: Config) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .context("failed to connect to Postgres")?;

    let smtp_config = SmtpConfig {
        host: std::env::var("RB_SMTP_HOST").unwrap_or_default(),
        port: std::env::var("RB_SMTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(587),
        username: std::env::var("RB_SMTP_USER").unwrap_or_default(),
        password: std::env::var("RB_SMTP_PASS").unwrap_or_default(),
        from_address: std::env::var("RB_SMTP_FROM")
            .unwrap_or_else(|_| "noreply@rust-brain.app".to_owned()),
    };
    let email_sender = from_transport(&config.email_transport, &smtp_config)
        .context("failed to build email sender")?;

    let hasher = PasswordHasher::from_config(
        config.argon2_memory_kb,
        config.argon2_time_cost,
        config.argon2_parallelism,
    )
    .context("invalid argon2 parameters")?;

    let gh = build_gh_app(&config)?;

    let state = AppState {
        pool,
        email_sender: Arc::from(email_sender),
        hasher: Arc::new(hasher),
        login_rate_limiter: Arc::new(LoginRateLimiter::new()),
        config: Arc::new(config.clone()),
        gh: gh.map(Arc::new),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::build(state)
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(cors);

    let addr: std::net::SocketAddr = config.listen_addr.parse()?;
    tracing::info!(addr = %addr, "control-api listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Constructs a [`GhApp`] from config, or returns `None` when the GitHub App
/// env vars are not set (feature is disabled; GitHub routes return 503).
///
/// Fails fast at startup if keys are present but malformed — an operator
/// mistake should surface immediately, not at first API call.
fn build_gh_app(config: &Config) -> Result<Option<GhApp>> {
    let (Some(app_id), Some(pem_b64)) = (config.gh_app_id, config.gh_app_private_key_b64.as_deref()) else {
        tracing::info!("RB_GH_APP_ID / RB_GH_APP_PRIVATE_KEY not set — GitHub App disabled");
        return Ok(None);
    };

    let pem = base64::engine::general_purpose::STANDARD
        .decode(pem_b64)
        .context("RB_GH_APP_PRIVATE_KEY must be base64-encoded PEM")?;

    let encoding_key = EncodingKey::from_rsa_pem(&pem)
        .context("RB_GH_APP_PRIVATE_KEY must be a valid RSA PEM private key")?;

    // Zeroize raw PEM bytes now that the opaque key has been derived.
    drop(pem);

    let webhook_secret_bytes = config
        .gh_app_webhook_secret
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "RB_GH_APP_WEBHOOK_SECRET must be set when GitHub App is enabled. \
                 An absent or empty webhook secret allows any caller to forge webhook \
                 payloads — set this env var before enabling real webhook delivery."
            )
        })?
        .as_bytes()
        .to_vec();
    let webhook_secret = Secret::new(webhook_secret_bytes);

    Ok(Some(GhApp::new(app_id, encoding_key, webhook_secret)))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
