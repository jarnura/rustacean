.PHONY: blob-smoke blob-smoke-s3

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
