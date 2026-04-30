use axum::Json;
use serde::Serialize;
use utoipa::{OpenApi as _, ToSchema};

use crate::openapi::ApiDoc;

#[derive(Serialize, ToSchema)]
pub struct ProbeResponse {
    pub status: &'static str,
}

/// Liveness probe — always returns 200 while the process is running.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is alive and healthy", body = ProbeResponse)
    ),
    tag = "health"
)]
pub async fn health_check() -> Json<ProbeResponse> {
    Json(ProbeResponse { status: "ok" })
}

/// Readiness probe — returns 200 when the service is ready to serve traffic.
///
/// Full DB connectivity check is wired in RUSAA-38; currently mirrors the
/// liveness probe so health and ready endpoints can be tested immediately.
#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Service is ready", body = ProbeResponse),
        (status = 503, description = "Service is not ready")
    ),
    tag = "health"
)]
pub async fn ready_check() -> Json<ProbeResponse> {
    Json(ProbeResponse { status: "ok" })
}

/// Returns the `OpenAPI` 3.1 spec as JSON.
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
