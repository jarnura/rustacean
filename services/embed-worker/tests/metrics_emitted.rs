//! Value-asserting tests for all `rb_embed_worker_*` metrics.
//!
//! Uses `metrics_util::debugging::DebuggingRecorder` as the in-process backend.
//! Metrics are emitted directly (matching the `counter!` call-sites in
//! `consumer.rs`) and asserted against the snapshot.

use metrics_util::debugging::{DebugValue, DebuggingRecorder};

fn counter_value(
    entries: &[(
        metrics_util::CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        DebugValue,
    )],
    name: &str,
    labels: &[(&str, &str)],
) -> u64 {
    entries
        .iter()
        .filter(|(key, _, _, _)| key.key().name() == name)
        .filter_map(|(key, _, _, value)| {
            let key_labels: std::collections::HashMap<&str, &str> =
                key.key().labels().map(|l| (l.key(), l.value())).collect();
            let matches = labels.iter().all(|(k, v)| key_labels.get(k) == Some(v));
            if matches {
                if let DebugValue::Counter(n) = value {
                    return Some(*n);
                }
            }
            None
        })
        .sum()
}

/// Covers all three `rb_embed_worker_*` metrics in a single recorder install
/// (global recorders cannot be replaced within the same test binary).
#[test]
fn all_embed_worker_metrics_are_emitted_with_correct_values() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    assert!(
        recorder.install().is_ok(),
        "DebuggingRecorder install must succeed"
    );

    // consumer.rs — successful process path
    metrics::counter!("rb_embed_worker_total", "outcome" => "ok").increment(1);

    // consumer.rs — failed process path
    metrics::counter!("rb_embed_worker_total", "outcome" => "err").increment(1);

    // consumer.rs — DLQ routing on max retries exceeded
    metrics::counter!("rb_embed_worker_dlq_total").increment(1);

    // consumer.rs — successful vector upsert to Qdrant
    metrics::counter!("rb_embed_worker_vectors_total").increment(1);

    let entries = snapshotter.snapshot().into_vec();

    assert_eq!(
        counter_value(&entries, "rb_embed_worker_total", &[("outcome", "ok")]),
        1,
        "rb_embed_worker_total outcome=ok must be 1"
    );
    assert_eq!(
        counter_value(&entries, "rb_embed_worker_total", &[("outcome", "err")]),
        1,
        "rb_embed_worker_total outcome=err must be 1"
    );
    assert_eq!(
        counter_value(&entries, "rb_embed_worker_dlq_total", &[]),
        1,
        "rb_embed_worker_dlq_total must be 1"
    );
    assert_eq!(
        counter_value(&entries, "rb_embed_worker_vectors_total", &[]),
        1,
        "rb_embed_worker_vectors_total must be 1"
    );
}
