/// ADR-006 §3.6 — synthetic test producer for the ingest-smoke exit gate.
///
/// Usage:
///   rb-test-producer --tenant-id <uuid> [--topic <topic>] [--count <n>]
///
/// Emits synthetic [`IngestStatusEvent`] messages to the specified Kafka topic
/// so the control-api ingest consumer can fan them out through SSE.
/// Used by `make ingest-smoke`.
use anyhow::{Context as _, Result};
use clap::Parser;
use rb_kafka::{EnvelopeMeta, EventEnvelope, Producer, ProducerCfg, SchemaVersion};
use rb_schemas::{IngestStatus, IngestStatusEvent, TenantId};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    name = "rb-test-producer",
    about = "Emit synthetic IngestStatusEvent messages to Kafka (dev / smoke testing)"
)]
struct Cli {
    /// Tenant UUID to scope events to.
    #[arg(long)]
    tenant_id: Uuid,

    /// Kafka topic to publish to.
    #[arg(long, default_value = "rb.projector.events")]
    topic: String,

    /// Number of synthetic events to emit.
    #[arg(long, default_value_t = 1)]
    count: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialise OTel so kafka.produce spans are exported to Tempo.
    // Falls back gracefully when OTEL_EXPORTER_OTLP_ENDPOINT is unset or unreachable.
    let _tracing_guard = rb_tracing::init("rb-smoke-producer").ok();

    let cfg = ProducerCfg::default();
    let producer =
        Producer::<IngestStatusEvent>::new(&cfg).context("failed to create Kafka producer")?;

    let tenant_id = TenantId::from(cli.tenant_id);

    for i in 1..=cli.count {
        let event_id = Uuid::new_v4();
        let payload = IngestStatusEvent {
            ingest_request_id: format!("smoke-{i}"),
            tenant_id: cli.tenant_id.to_string(),
            status: IngestStatus::Processing as i32, // synthetic — shows pipeline is active
            error_message: String::new(),
            occurred_at_ms: chrono::Utc::now().timestamp_millis(),
            ..Default::default()
        };

        let envelope = EventEnvelope {
            tenant_id,
            event_id,
            schema_version: SchemaVersion::V1,
            // No explicit trace_context: producer.publish() injects the active OTel span
            // (the kafka.produce span it creates internally) so kafka.consume and
            // sse.publish share the same trace id.
            trace_context: None,
            blob_ref: None,
            created_at: chrono::Utc::now(),
            payload,
            _meta: EnvelopeMeta::default(),
        };

        let key = cli.tenant_id.as_bytes().to_vec();
        producer
            .publish(&cli.topic, &key, envelope)
            .await
            .with_context(|| format!("failed to publish event {i}"))?;

        println!("published event {i}/{}", cli.count);
    }

    println!("done — {} event(s) sent to {}", cli.count, cli.topic);
    Ok(())
}
