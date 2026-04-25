pub mod health;

use axum::{
    Router,
    routing::get,
};

use crate::routes::health::{health_check, openapi_json, ready_check};

/// Assembles the full application router.
pub fn build() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/openapi.json", get(openapi_json))
}
