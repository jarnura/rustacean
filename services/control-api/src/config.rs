use std::env;

use anyhow::{Context as _, Result};

/// Service configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: String,
    pub database_url: String,
    pub cors_origins: Vec<String>,
    pub base_url: String,
    pub session_ttl_days: i64,
    pub argon2_memory_kb: u32,
    pub argon2_time_cost: u32,
    pub argon2_parallelism: u32,
    pub email_transport: String,
    pub service_name: String,
}

impl Config {
    /// Loads configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if `RB_DATABASE_URL` is absent or if any numeric
    /// environment variable cannot be parsed.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: env::var("RB_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
            database_url: env::var("RB_DATABASE_URL")
                .context("RB_DATABASE_URL is required")?,
            cors_origins: env::var("RB_CORS_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:5173".to_owned())
                .split(',')
                .map(|s| s.trim().to_owned())
                .collect(),
            base_url: env::var("RB_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_owned()),
            session_ttl_days: env::var("RB_SESSION_TTL_DAYS")
                .unwrap_or_else(|_| "30".to_owned())
                .parse()
                .context("RB_SESSION_TTL_DAYS must be a positive integer")?,
            argon2_memory_kb: env::var("RB_ARGON2_MEMORY_KB")
                .unwrap_or_else(|_| "19456".to_owned())
                .parse()
                .context("RB_ARGON2_MEMORY_KB must be a positive integer")?,
            argon2_time_cost: env::var("RB_ARGON2_TIME_COST")
                .unwrap_or_else(|_| "2".to_owned())
                .parse()
                .context("RB_ARGON2_TIME_COST must be a positive integer")?,
            argon2_parallelism: env::var("RB_ARGON2_PARALLELISM")
                .unwrap_or_else(|_| "1".to_owned())
                .parse()
                .context("RB_ARGON2_PARALLELISM must be a positive integer")?,
            email_transport: env::var("RB_EMAIL_TRANSPORT")
                .unwrap_or_else(|_| "console".to_owned()),
            service_name: env::var("OTEL_SERVICE_NAME")
                .unwrap_or_else(|_| "control-api".to_owned()),
        })
    }

    /// Creates a minimal config for tests and integration-test harnesses.
    ///
    /// Uses fast argon2id params and noop email transport.
    #[doc(hidden)]
    #[must_use]
    pub fn for_test() -> Self {
        Self {
            listen_addr: "127.0.0.1:0".to_owned(),
            database_url: "postgres://localhost/test".to_owned(),
            cors_origins: vec!["http://localhost:5173".to_owned()],
            base_url: "http://localhost:8080".to_owned(),
            session_ttl_days: 30,
            argon2_memory_kb: 64,
            argon2_time_cost: 1,
            argon2_parallelism: 1,
            email_transport: "noop".to_owned(),
            service_name: "control-api-test".to_owned(),
        }
    }
}
