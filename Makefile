.PHONY: review-ready blob-smoke blob-smoke-s3 ingest-smoke ingest-smoke-offline

# Run all local pre-PR checks: fmt, clippy, test, deny, openapi, frontend (if changed).
# All steps run even on partial failure so you see the full picture before pushing.
review-ready:
	bash scripts/review-ready.sh

# Run the full tracer-bullet ingest smoke (ADR-006 §17 exit gate).
#
# Requires compose/full.yml to be running (Kafka + OTel collector + Tempo):
#   docker compose -f compose/full.yml up -d
#   cargo build --workspace
#
# Produces one synthetic IngestStatusEvent to rb.projector.events, then prints
# the curl and Tempo verification commands.
#
# Env overrides (see scripts/ingest-smoke.sh for full list):
#   TENANT_ID                — target tenant UUID
#   KAFKA_BOOTSTRAP_SERVERS  — Kafka bootstrap (default: localhost:9094)
#
# Usage:
#   make ingest-smoke
#   TENANT_ID=<uuid> make ingest-smoke
ingest-smoke: ingest-smoke-offline
	@echo "==> ingest smoke: invoking Kafka smoke harness"
	bash scripts/ingest-smoke.sh

# Offline gate: run rb-sse unit tests and verify the producer binary compiles.
# No Kafka or compose stack required.
ingest-smoke-offline:
	@echo "==> ingest smoke: rb-sse unit tests"
	cargo test -p rb-sse --features test-util -- --nocapture
	@echo "==> ingest smoke: test-producer binary compiles"
	cargo build -p control-api --bin rb-test-producer --quiet
	@echo "==> ingest smoke offline: PASS"

# Run filesystem blob store smoke tests (no external deps required).
blob-smoke:
	@echo "==> blob smoke: filesystem roundtrip"
	cargo test -p rb-blob fs_roundtrip -- --nocapture
	@echo "==> blob smoke: tenant isolation"
	cargo test -p rb-blob tenant_isolation -- --nocapture
	@echo "==> blob smoke: large blob (100 MiB)"
	cargo test -p rb-blob large_blob -- --nocapture
	@echo "==> blob smoke: PASS"

# Run S3 smoke tests against a running localstack instance.
# Requires: docker compose -f compose/test.yml up localstack
# and a pre-created bucket: aws --endpoint-url=http://localhost:4566 s3 mb s3://rb-blobs
blob-smoke-s3:
	@echo "==> blob smoke: s3 roundtrip (localstack)"
	RB_BLOB_S3_ENDPOINT=http://localhost:4566 \
	RB_BLOB_S3_BUCKET=rb-blobs \
	AWS_ACCESS_KEY_ID=test \
	AWS_SECRET_ACCESS_KEY=test \
	AWS_REGION=us-east-1 \
	cargo test -p rb-blob s3_roundtrip --features s3 -- --nocapture
	@echo "==> blob smoke s3: PASS"
