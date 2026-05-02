use std::path::Path;

use rb_parse_syn::extract_items as syn_extract;
use rb_parse_tree_sitter::extract_items_partial as ts_extract;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/parse_inputs");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(FIXTURE_DIR).join(name))
        .unwrap_or_else(|_| panic!("fixture {name} not found"))
}

// ── simple.rs corpus ────────────────────────────────────────────────────────

#[test]
fn simple_fixture_syn_extracts_all_item_kinds() {
    let src = fixture("simple.rs");
    let items = syn_extract(&src).expect("simple.rs must parse cleanly");

    let kinds: Vec<_> = items.iter().map(|i| i.kind).collect();
    assert!(
        kinds.contains(&rb_parse_syn::Kind::Struct),
        "expected Struct in simple.rs"
    );
    assert!(
        kinds.contains(&rb_parse_syn::Kind::Fn),
        "expected Fn in simple.rs"
    );
    assert!(
        kinds.contains(&rb_parse_syn::Kind::Const),
        "expected Const in simple.rs"
    );
    assert!(
        kinds.contains(&rb_parse_syn::Kind::Enum),
        "expected Enum in simple.rs"
    );
}

#[test]
fn simple_fixture_syn_item_names_correct() {
    let src = fixture("simple.rs");
    let items = syn_extract(&src).expect("simple.rs must parse cleanly");
    let names: Vec<_> = items.iter().map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"Config"), "expected Config");
    assert!(names.contains(&"connect"), "expected connect");
    assert!(names.contains(&"DEFAULT_PORT"), "expected DEFAULT_PORT");
    assert!(names.contains(&"Transport"), "expected Transport");
}

#[test]
fn simple_fixture_syn_line_numbers_populated() {
    let src = fixture("simple.rs");
    let items = syn_extract(&src).unwrap();
    for item in &items {
        assert!(item.line_start > 0, "line_start must be ≥1 for {}", item.name);
        assert!(
            item.line_end >= item.line_start,
            "line_end must be ≥ line_start for {}",
            item.name
        );
    }
}

// ── complex.rs corpus ────────────────────────────────────────────────────────

#[test]
fn complex_fixture_syn_extracts_traits_impl_mod() {
    let src = fixture("complex.rs");
    let items = syn_extract(&src).expect("complex.rs must parse cleanly");
    let kinds: Vec<_> = items.iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&rb_parse_syn::Kind::Trait));
    assert!(kinds.contains(&rb_parse_syn::Kind::Impl));
    assert!(kinds.contains(&rb_parse_syn::Kind::Mod));
    assert!(kinds.contains(&rb_parse_syn::Kind::Enum));
    assert!(kinds.contains(&rb_parse_syn::Kind::Fn));
}

#[test]
fn complex_fixture_syn_impl_name_includes_type() {
    let src = fixture("complex.rs");
    let items = syn_extract(&src).unwrap();
    let impl_names: Vec<_> = items
        .iter()
        .filter(|i| i.kind == rb_parse_syn::Kind::Impl)
        .map(|i| i.name.as_str())
        .collect();
    assert!(
        impl_names.iter().any(|n| n.contains("Registry")),
        "expected impl for Registry, got: {impl_names:?}"
    );
}

// ── bad_syntax.rs corpus ─────────────────────────────────────────────────────

#[test]
fn bad_syntax_fixture_syn_fails_gracefully() {
    let src = fixture("bad_syntax.rs");
    let result = syn_extract(&src);
    assert!(result.is_err(), "bad_syntax.rs must fail syn parsing");
}

#[test]
fn bad_syntax_fixture_tree_sitter_recovers_items() {
    let src = fixture("bad_syntax.rs");
    let items = ts_extract(&src);
    // tree-sitter should find at least one of the valid items
    assert!(
        items.iter().any(|i| i.name == "valid_before_error" || i.name == "ValidAfterError"),
        "tree-sitter should recover at least one item from bad_syntax.rs, got: {items:?}"
    );
}

// ── Round-trip: all fixtures parseable ───────────────────────────────────────

#[test]
fn all_fixtures_produce_at_least_one_item_via_combined_strategy() {
    for fixture_name in &["simple.rs", "complex.rs", "bad_syntax.rs"] {
        let src = fixture(fixture_name);
        let count = match syn_extract(&src) {
            Ok(items) => items.len(),
            Err(_) => ts_extract(&src).len(),
        };
        assert!(
            count > 0,
            "fixture {fixture_name} produced zero items from both parsers"
        );
    }
}
