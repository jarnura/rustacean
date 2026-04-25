pub mod auth;
pub mod health;

use axum::{Router, routing::{get, post}};

use crate::routes::{
    auth::signup,
    health::{health_check, openapi_json, ready_check},
};
use crate::state::AppState;

/// Assembles the full application router.
pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/openapi.json", get(openapi_json))
        .route("/v1/auth/signup", post(signup))
        .with_state(state)
}
