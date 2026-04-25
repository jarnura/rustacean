# rb-schemas

Protobuf-generated message types for the rust-brain v1 ingest pipeline, plus the `TenantId` newtype.

## Public API

| Type | Description |
|------|-------------|
| `struct TenantId` | UUID newtype identifying a tenant; serialises transparently as a UUID string |
| `struct IngestRequest` | Carries raw event data submitted to the ingest pipeline (`tenant_id`, `event_id`, `source`, `payload`, `created_at_ms`) |
| `enum IngestStatus` | Processing lifecycle states: `Unspecified`, `Pending`, `Processing`, `Done`, `Failed` |
| `struct IngestStatusEvent` | Emitted by the ingest pipeline whenever an `IngestRequest` changes status |

Types generated from `proto/rust_brain/v1/ingest.proto` via `prost`; source of truth is the `.proto` file.

## Usage

```rust
use rb_schemas::{IngestRequest, IngestStatus, TenantId};

let tenant = TenantId::new();

let req = IngestRequest {
    tenant_id: tenant.to_string(),
    event_id: "evt-abc123".to_string(),
    source: "github".to_string(),
    payload: serde_json::to_vec(&my_payload).unwrap(),
    created_at_ms: 1_700_000_000_000,
};

assert_eq!(req.source, "github");
assert_eq!(IngestStatus::Done as i32, 3);
```

## Proto source

The messages are compiled from:

```
proto/rust_brain/v1/ingest.proto   (package rust_brain.v1)
```

To regenerate after editing the `.proto` file, simply run `cargo build -p rb-schemas`; the `build.rs` script re-runs `protox` + `prost-build` automatically when the file changes.

## Dependencies

- Depends on: *(no other rb-\* crates)*
- Used by: ingest pipeline services (none wired up yet)
