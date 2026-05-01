#!/usr/bin/env bash
# scripts/ingest-smoke.sh — tracer-bullet ingest smoke harness (ADR-006 §17)
#
# Produces one synthetic IngestStatusEvent to rb.projector.events via Kafka,
# which the control-api ingest consumer fans out to SSE as a sse.publish span.
# Three correlated spans must appear in Tempo under a single trace id:
#   kafka.produce  → kafka.consume  → sse.publish   (all rb.tenant_id tagged)
#
# The rb-test-producer binary runs inside the control-api container, so no
# host-side Rust toolchain or libcurl4-openssl-dev is required — only
# docker + docker compose.
#
# Prerequisites (compose/full.yml must be running with a built control-api image):
#   docker compose -f compose/full.yml build control-api
#   docker compose -f compose/full.yml up -d
#
# Environment overrides:
#   TENANT_ID    — UUID identifying the smoke tenant
#                  (default: 00000000-0000-0000-0000-000000000001)
#   KAFKA_TOPIC  — topic to publish to (default: rb.projector.events)
#
# Usage:
#   scripts/ingest-smoke.sh
#   TENANT_ID=<uuid> scripts/ingest-smoke.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

TENANT_ID="${TENANT_ID:-00000000-0000-0000-0000-000000000001}"
TOPIC="${KAFKA_TOPIC:-rb.projector.events}"

echo "==> ingest-smoke: producing 1 event"
echo "    tenant_id : ${TENANT_ID}"
echo "    topic     : ${TOPIC}"
echo "    bootstrap : kafka:9092 (compose-internal)"
echo ""

docker compose -f "${REPO_ROOT}/compose/full.yml" run --rm \
    --entrypoint /usr/local/bin/rb-test-producer \
    -e TENANT_ID="${TENANT_ID}" \
    -e KAFKA_BOOTSTRAP_SERVERS="kafka:9092" \
    -e KAFKA_TOPIC="${TOPIC}" \
    -e OTEL_EXPORTER_OTLP_ENDPOINT="http://otel-collector:4317" \
    control-api \
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
echo "  4. Confirm Kafka metrics are visible and DLQ counter did not increment:"
echo "     PROM_PORT=\${PROMETHEUS_HOST_PORT:-9090}"
echo "     curl -s \"http://localhost:\${PROM_PORT}/api/v1/query?query=rb_kafka_messages_total\" | jq ."
echo "     curl -s \"http://localhost:\${PROM_PORT}/api/v1/query?query=rb_kafka_dlq_total\" | jq ."
echo "     rb_kafka_dlq_total must return an empty result (no DLQ events);"
echo "     rb_kafka_messages_total{outcome=\"ok\"} must show at least 1 consume."
echo ""
echo "==> ingest-smoke: DONE"
