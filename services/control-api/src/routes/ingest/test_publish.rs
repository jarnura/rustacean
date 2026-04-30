use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use rb_schemas::TenantId;
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    middleware::auth::{AuthContext, require_verified_session},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct TestPublishRequest {
    /// SSE event name to emit (e.g. `"ingest.status"`).
    pub event: String,
    /// Arbitrary JSON data to include in the SSE `data:` field.
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct TestPublishResponse {
    pub ok: bool,
}

/// Emit a synthetic SSE event for the caller's tenant.
///
/// **Dev-only** — only active when `RB_DEV_TEST_ROUTES=1`.
/// Used by `make ingest-smoke` to exercise the SSE end-to-end path
/// without a real Kafka producer.
pub async fn test_publish(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<TestPublishRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.config.dev_test_routes {
        return Err(AppError::NotFound);
    }

    let session = require_verified_session(auth)?;
    let tenant_id = TenantId::from(session.tenant_id);

    let data = serde_json::to_string(&body.data).unwrap_or_else(|_| "{}".to_owned());
    state.sse_bus.publish_raw(&tenant_id, &body.event, data);

    Ok((StatusCode::OK, Json(TestPublishResponse { ok: true })))
}
