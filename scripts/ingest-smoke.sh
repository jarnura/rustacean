#!/usr/bin/env bash
# scripts/ingest-smoke.sh — tracer-bullet ingest smoke harness (ADR-006 §17)
#
# Produces one synthetic IngestStatusEvent to rb.projector.events via Kafka,
# which the control-api ingest consumer fans out to SSE as a sse.publish span.
# Three correlated spans must appear in Tempo under a single trace id:
#   kafka.produce  → kafka.consume  → sse.publish   (all rb.tenant_id tagged)
#
# Prerequisites (compose/full.yml must be running):
#   docker compose -f compose/full.yml up -d
#   cargo build --workspace
#
# Environment overrides:
#   TENANT_ID                — UUID identifying the smoke tenant
#                              (default: 00000000-0000-0000-0000-000000000001)
#   KAFKA_BOOTSTRAP_SERVERS  — Kafka bootstrap address
#                              (default: localhost:9094 — the external host port)
#   KAFKA_TOPIC              — topic to publish to (default: rb.projector.events)
#   OTEL_EXPORTER_OTLP_ENDPOINT — OTLP gRPC endpoint for kafka.produce trace
#                              (default: http://localhost:4317)
#
# Usage:
#   scripts/ingest-smoke.sh
#   TENANT_ID=<uuid> scripts/ingest-smoke.sh

set -euo pipefail

TENANT_ID="${TENANT_ID:-00000000-0000-0000-0000-000000000001}"
TOPIC="${KAFKA_TOPIC:-rb.projector.events}"
# Default to the external Kafka port used when compose runs on the same host.
export KAFKA_BOOTSTRAP_SERVERS="${KAFKA_BOOTSTRAP_SERVERS:-localhost:9094}"

echo "==> ingest-smoke: producing 1 event"
echo "    tenant_id : ${TENANT_ID}"
echo "    topic     : ${TOPIC}"
echo "    bootstrap : ${KAFKA_BOOTSTRAP_SERVERS}"
echo ""

cargo run -p control-api --bin rb-test-producer --quiet -- \
    --tenant-id "${TENANT_ID}" \
    --topic    "${TOPIC}"      \
    --count    1

echo ""
echo "==> Event produced. Verification steps:"
echo ""
echo "  1. Obtain a session cookie (or token) for tenant ${TENANT_ID}:"
echo "     POST http://localhost:10080/v1/auth/login"
echo ""
echo "  2. Open the SSE stream and watch for the ingest.status event:"
echo "     curl -N -H 'Cookie: rb_session=<token>' \\"
echo "          http://localhost:10080/v1/ingest/events"
echo ""
echo "     Expected SSE line:"
echo "       event: ingest.status"
echo "       data: {\"status\":\"processing\", ...}"
echo ""
echo "  3. Verify the full trace in Grafana Tempo (http://localhost:13000):"
echo "     Search by tag rb.tenant_id=${TENANT_ID}"
echo "     Expect one trace with all three spans:"
echo "       kafka.produce  (rb.tenant_id=${TENANT_ID})"
echo "       kafka.consume  (rb.tenant_id=${TENANT_ID})"
echo "       sse.publish    (rb.tenant_id=${TENANT_ID})"
echo ""
echo "  4. Confirm no DLQ increment:"
echo "     rb_kafka_dlq_total must not have increased for topic=${TOPIC}"
echo ""
echo "==> ingest-smoke: DONE"
