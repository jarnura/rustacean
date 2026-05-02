//! Ollama HTTP client and composite embedding input builder.
//!
//! The composite prompt follows ADR-007 §3.5.7:
//!   fqn / `resolved_type_signature` / `trait_bounds` / source text
//!
//! All fields are best-effort: missing fields are omitted rather than erroring
//! (PARTIAL_* quality items per ADR-007 §13.4).

use anyhow::{Context as _, Result};
use serde_json::json;

/// Build the §3.5.7 composite embedding prompt for one item.
///
/// Fields that are empty or absent are omitted to avoid polluting the
/// embedding with uninformative whitespace lines.
pub(crate) fn build_composite(
    fqn: &str,
    type_signature: &str,
    trait_bounds: &[String],
    source_text: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(5);

    parts.push(format!("fqn: {fqn}"));

    if !type_signature.is_empty() {
        parts.push(format!("signature: {type_signature}"));
    }

    if !trait_bounds.is_empty() {
        parts.push(format!("bounds: {}", trait_bounds.join(", ")));
    }

    if let Some(src) = source_text {
        let trimmed = src.trim();
        if !trimmed.is_empty() {
            parts.push(format!("source:\n{trimmed}"));
        }
    }

    parts.join("\n")
}

/// POST to `{ollama_url}/api/embeddings` and return the embedding vector.
///
/// Ollama response: `{"embedding": [f64, ...]}`
#[allow(clippy::cast_possible_truncation)]
pub(crate) async fn call_ollama(
    http: &reqwest::Client,
    ollama_url: &str,
    model: &str,
    prompt: &str,
) -> Result<Vec<f32>> {
    let url = format!("{ollama_url}/api/embeddings");
    let body = json!({ "model": model, "prompt": prompt });

    let resp = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("Ollama request failed")?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama returned HTTP {status}: {text}");
    }

    let json: serde_json::Value = resp.json().await.context("Ollama response is not JSON")?;

    let embedding = json
        .get("embedding")
        .and_then(|v| v.as_array())
        .context("Ollama response missing 'embedding' array")?;

    let vector: Vec<f32> = embedding
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_f64()
                .map(|f| f as f32)
                .with_context(|| format!("embedding[{i}] is not a number"))
        })
        .collect::<Result<_>>()?;

    Ok(vector)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_includes_all_non_empty_fields() {
        let composite = build_composite(
            "src_lib::Foo",
            "impl Display for Foo",
            &["T: Clone".to_owned()],
            Some("pub struct Foo;"),
        );
        assert!(composite.contains("fqn: src_lib::Foo"));
        assert!(composite.contains("signature: impl Display for Foo"));
        assert!(composite.contains("bounds: T: Clone"));
        assert!(composite.contains("source:\npub struct Foo;"));
    }

    #[test]
    fn composite_omits_empty_fields() {
        let composite = build_composite("src_lib::Bar", "", &[], None);
        assert_eq!(composite, "fqn: src_lib::Bar");
        assert!(!composite.contains("signature"));
        assert!(!composite.contains("bounds"));
        assert!(!composite.contains("source"));
    }

    #[test]
    fn composite_multiple_bounds() {
        let composite = build_composite(
            "my_mod::process",
            "fn process<T>()",
            &["T: Clone".to_owned(), "T: Send".to_owned()],
            None,
        );
        assert!(composite.contains("bounds: T: Clone, T: Send"));
    }

    #[test]
    fn composite_trims_source_whitespace() {
        let composite = build_composite("x::y", "", &[], Some("  fn foo() {}  "));
        assert!(composite.contains("source:\nfn foo() {}"));
    }

    #[test]
    fn composite_skips_whitespace_only_source() {
        let composite = build_composite("x::y", "", &[], Some("   \n  "));
        assert!(!composite.contains("source"));
    }
}
