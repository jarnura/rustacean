// The utoipa OpenApi derive macro generates code that triggers
// clippy::needless_for_each internally. Suppress at file scope since this is
// a macro code-generation artefact we cannot control.
#![allow(clippy::needless_for_each)]

use utoipa::OpenApi;

use crate::routes::health;

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health_check,
        health::ready_check,
    ),
    info(
        title = "rust-brain control API",
        version = "0.1.0",
        description = "Control-plane API for rust-brain. \
            Auth, tenant management, and API key endpoints are added in subsequent waves.",
        contact(
            name = "rust-brain",
            url = "https://github.com/jarnura/rustacean",
        ),
    ),
    tags(
        (name = "health", description = "Liveness and readiness probes"),
    ),
)]
pub struct ApiDoc;
