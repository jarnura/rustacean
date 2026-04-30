use axum::{extract::State, http::HeaderMap, response::IntoResponse};
use rb_schemas::TenantId;
use rb_sse::EventId;

use crate::{
    error::AppError,
    middleware::auth::{AuthContext, require_verified_session},
    state::AppState,
};

/// Stream live ingest-status events for the caller's active tenant.
///
/// SSE wire format per ADR-006 §8.2:
/// ```text
/// id: <event-id>
/// event: ingest.status
/// retry: 5000
/// data: <json>
/// ```
///
/// Reconnect with `Last-Event-Id` to replay events ≤ 5 min old from the
/// ring buffer.  An unknown or stale ID emits a `stream-reset` event.
#[utoipa::path(
    get,
    path = "/v1/ingest/events",
    responses(
        (status = 200, description = "SSE stream; Content-Type: text/event-stream"),
        (status = 401, description = "Not authenticated or session expired"),
        (status = 403, description = "Email not verified"),
    ),
    tag = "ingest"
)]
pub async fn events_stream(
    State(state): State<AppState>,
    auth: AuthContext,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let session = require_verified_session(auth)?;

    let tenant_id = TenantId::from(session.tenant_id);

    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| EventId::from(s.to_owned()));

    Ok(state.sse_bus.subscribe(&tenant_id, last_event_id.as_ref()))
}
