use std::sync::Arc;

use rb_auth::PasswordHasher;
use rb_email::EmailSender;
use sqlx::PgPool;

use crate::config::Config;

/// Shared application state injected into every request handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub email_sender: Arc<dyn EmailSender>,
    pub hasher: Arc<PasswordHasher>,
    pub config: Arc<Config>,
}
