# rb-tracing

Structured JSON logging and OpenTelemetry tracing initialization for rust-brain services.

## Public API

| Type | Description |
|------|-------------|
| `fn init` | Initializes OTLP span export and a `tracing-subscriber` registry; returns a `TracingGuard` |
| `struct TracingGuard` | Flushes pending spans on drop — hold for the process lifetime inside `main()` |
| `struct StructuredJsonLayer` | `tracing-subscriber` layer that emits one compact JSON line per event |
| `enum TracingError` | Error variants for OTLP exporter init (`OtlpInit`) and subscriber registration (`Subscriber`) |

## Usage

```rust
use rb_tracing::init;

#[tokio::main]
async fn main() {
    let _guard = init("my-service").expect("tracing init failed");
    // _guard is held for the process lifetime; dropping it flushes pending spans
    tracing::info!(version = "1.0", "service started");
}
```

### JSON log line shape

Each line written by `StructuredJsonLayer` contains:

```json
{
  "timestamp": "2024-06-01T12:34:56.123456789Z",
  "level": "INFO",
  "target": "my_service::handler",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "span_id": "00f067aa0ba902b7",
  "span": "handle_request",
  "spans": ["serve", "handle_request"],
  "fields": { "message": "service started", "version": "1.0" }
}
```

`trace_id` and `span_id` are non-empty only when the event is emitted inside an active OpenTelemetry span. `span` / `spans` are omitted when no `tracing` span is active.

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | gRPC endpoint for the OTLP span exporter |
| `RUST_LOG` | `info` | Log filter passed to `EnvFilter` |
| `RB_LOG_FORMAT` | `json` | Set to `pretty` for human-readable output in development |

## Dependencies

- Depends on: *(no other rb-\* crates)*
- Used by: all rust-brain services (none wired up yet)
