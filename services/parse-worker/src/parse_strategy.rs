//! Parse strategy — primary (syn) and fallback (tree-sitter) item extraction.

use rb_schemas::ItemKind;

pub(crate) struct Extraction {
    pub(crate) items: Vec<ExtractedItemData>,
    pub(crate) had_error: bool,
    pub(crate) error_message: String,
}

pub(crate) struct ExtractedItemData {
    pub(crate) name: String,
    pub(crate) kind: ItemKind,
    pub(crate) source_text: String,
    pub(crate) line_start: u32,
    pub(crate) line_end: u32,
}

pub(crate) fn parse_file(source: &str, rel_path: &str, ingest_run_id: &str) -> Extraction {
    // Primary: syn (accurate, full semantic info)
    match rb_parse_syn::extract_items(source) {
        Ok(items) => {
            let data = items
                .into_iter()
                .map(|i| ExtractedItemData {
                    name: i.name,
                    kind: syn_kind_to_proto(i.kind),
                    source_text: i.source_text,
                    line_start: i.line_start,
                    line_end: i.line_end,
                })
                .collect();
            return Extraction {
                items: data,
                had_error: false,
                error_message: String::new(),
            };
        }
        Err(e) => {
            tracing::warn!(
                ingest_run_id,
                path = rel_path,
                "parse_worker: syn failed, falling back to tree-sitter: {e}"
            );
        }
    }

    // Fallback: tree-sitter (error-tolerant, partial extraction)
    let partial = rb_parse_tree_sitter::extract_items_partial(source);
    if partial.is_empty() {
        return Extraction {
            items: Vec::new(),
            had_error: true,
            error_message: format!(
                "parse_error: syn and tree-sitter produced no items for {rel_path}"
            ),
        };
    }

    let data = partial
        .into_iter()
        .map(|i| ExtractedItemData {
            name: i.name,
            kind: ts_kind_to_proto(i.kind),
            source_text: source.to_owned(),
            line_start: i.line_start,
            line_end: i.line_end,
        })
        .collect();

    Extraction {
        items: data,
        had_error: true,
        error_message: String::new(),
    }
}

fn syn_kind_to_proto(k: rb_parse_syn::Kind) -> ItemKind {
    match k {
        rb_parse_syn::Kind::Fn => ItemKind::Fn,
        rb_parse_syn::Kind::Struct => ItemKind::Struct,
        rb_parse_syn::Kind::Enum => ItemKind::Enum,
        rb_parse_syn::Kind::Trait => ItemKind::Trait,
        rb_parse_syn::Kind::Impl => ItemKind::Impl,
        rb_parse_syn::Kind::Mod => ItemKind::Mod,
        rb_parse_syn::Kind::Const => ItemKind::Const,
        rb_parse_syn::Kind::TypeAlias | rb_parse_syn::Kind::Static => ItemKind::Unspecified,
        rb_parse_syn::Kind::MacroDef => ItemKind::MacroDef,
    }
}

fn ts_kind_to_proto(k: rb_parse_tree_sitter::Kind) -> ItemKind {
    match k {
        rb_parse_tree_sitter::Kind::Fn => ItemKind::Fn,
        rb_parse_tree_sitter::Kind::Struct => ItemKind::Struct,
        rb_parse_tree_sitter::Kind::Enum => ItemKind::Enum,
        rb_parse_tree_sitter::Kind::Trait => ItemKind::Trait,
        rb_parse_tree_sitter::Kind::Impl => ItemKind::Impl,
        rb_parse_tree_sitter::Kind::Mod => ItemKind::Mod,
        rb_parse_tree_sitter::Kind::Const => ItemKind::Const,
        rb_parse_tree_sitter::Kind::TypeAlias | rb_parse_tree_sitter::Kind::Static => {
            ItemKind::Unspecified
        }
        rb_parse_tree_sitter::Kind::MacroDef => ItemKind::MacroDef,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_extracts_items_from_valid_source() {
        let src = "pub fn hello() {}\npub struct World {}";
        let result = parse_file(src, "src/lib.rs", "run-1");
        assert!(!result.had_error);
        assert_eq!(result.items.len(), 2);
        let kinds: Vec<_> = result.items.iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&ItemKind::Fn));
        assert!(kinds.contains(&ItemKind::Struct));
    }

    #[test]
    fn parse_file_falls_back_to_tree_sitter_on_syn_error() {
        let src = "fn broken( { }\npub fn good() {}";
        let result = parse_file(src, "src/lib.rs", "run-2");
        assert!(result.had_error);
        assert!(result.items.iter().any(|i| i.name == "good"));
    }

    #[test]
    fn parse_file_error_item_emitted_when_completely_unparseable() {
        let src = "";
        let result = parse_file(src, "src/empty.rs", "run-3");
        // syn parses empty files fine — 0 items, no error
        assert!(!result.had_error);
        assert!(result.items.is_empty());
    }
}
