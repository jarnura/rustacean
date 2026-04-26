use std::sync::Arc;

use anyhow::{Context as _, Result};
use rb_auth::{LoginRateLimiter, PasswordHasher};
use rb_email::{SmtpConfig, from_transport};
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

    let state = AppState {
        pool,
        email_sender: Arc::from(email_sender),
        hasher: Arc::new(hasher),
        login_rate_limiter: Arc::new(LoginRateLimiter::new()),
        config: Arc::new(config.clone()),
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
