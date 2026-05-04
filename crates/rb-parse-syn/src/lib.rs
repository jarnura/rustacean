//! Syn-based Rust item extractor for the parse pipeline stage.
//!
//! Parses syntactically valid `*.rs` source via `syn` and extracts top-level
//! items with their kind, name, and source location (ADR-007 §11.5).

use proc_macro2::LineColumn;
use syn::visit::Visit;

#[derive(Debug, thiserror::Error)]
pub enum ParseSynError {
    #[error("syn parse error: {0}")]
    Syn(#[from] syn::Error),
}

/// Rust item kind — mirrors `ItemKind` in `pipeline.proto` (ADR-007 §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Fn,
    Struct,
    Enum,
    Trait,
    Impl,
    Mod,
    Const,
    TypeAlias,
    Static,
    MacroDef,
}

/// A single extracted Rust item.
#[derive(Debug, Clone)]
pub struct ExtractedItem {
    /// Simple ident name (e.g. `"my_fn"`).
    pub name: String,
    pub kind: Kind,
    pub line_start: u32,
    pub line_end: u32,
}

/// Extract all top-level items from `source`.
///
/// Returns `Err` only when the source cannot be parsed at all. Individual
/// items that produce no name (rare) are silently skipped.
///
/// # Errors
///
/// Returns [`ParseSynError::Syn`] if `source` cannot be parsed as a Rust file.
pub fn extract_items(source: &str) -> Result<Vec<ExtractedItem>, ParseSynError> {
    let file: syn::File = syn::parse_str(source)?;
    let mut visitor = ItemVisitor { items: Vec::new() };
    visitor.visit_file(&file);
    Ok(visitor.items)
}

struct ItemVisitor {
    items: Vec<ExtractedItem>,
}

impl ItemVisitor {
    fn push(&mut self, name: String, kind: Kind, start_span: proc_macro2::Span, end_span: proc_macro2::Span) {
        let start: LineColumn = start_span.start();
        let end: LineColumn = end_span.end();
        self.items.push(ExtractedItem {
            name,
            kind,
            line_start: u32::try_from(start.line).expect("line number fits u32"),
            line_end: u32::try_from(end.line).expect("line number fits u32"),
        });
    }
}

impl<'ast> Visit<'ast> for ItemVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let end = node.block.brace_token.span.close();
        self.push(node.sig.ident.to_string(), Kind::Fn, node.sig.fn_token.span, end);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let end = match &node.fields {
            syn::Fields::Named(f) => f.brace_token.span.close(),
            syn::Fields::Unnamed(f) => f.paren_token.span.close(),
            syn::Fields::Unit => node.semi_token.map_or(node.ident.span(), |s| s.span),
        };
        self.push(node.ident.to_string(), Kind::Struct, node.struct_token.span, end);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        let end = node.brace_token.span.close();
        self.push(node.ident.to_string(), Kind::Enum, node.enum_token.span, end);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let end = node.brace_token.span.close();
        self.push(node.ident.to_string(), Kind::Trait, node.trait_token.span, end);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let name = impl_name(node);
        let end = node.brace_token.span.close();
        self.push(name, Kind::Impl, node.impl_token.span, end);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let end = node.content.as_ref().map_or(
            node.semi.map_or(node.ident.span(), |s| s.span),
            |(brace, _)| brace.span.close(),
        );
        self.push(node.ident.to_string(), Kind::Mod, node.mod_token.span, end);
    }

    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        self.push(node.ident.to_string(), Kind::Const, node.const_token.span, node.semi_token.span);
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.push(node.ident.to_string(), Kind::TypeAlias, node.type_token.span, node.semi_token.span);
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        self.push(node.ident.to_string(), Kind::Static, node.static_token.span, node.semi_token.span);
    }

    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        if let Some(ident) = &node.ident {
            let end = node.semi_token.map_or(ident.span(), |s| s.span);
            self.push(ident.to_string(), Kind::MacroDef, ident.span(), end);
        }
    }
}

fn impl_name(node: &syn::ItemImpl) -> String {
    let self_ty = type_name(&node.self_ty);
    match &node.trait_ {
        Some((_, path, _)) => {
            let trait_name = path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            format!("<{self_ty} as {trait_name}>")
        }
        None => format!("impl {self_ty}"),
    }
}

fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        _ => "?".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fn() {
        let src = "pub fn hello() {}";
        let items = extract_items(src).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "hello");
        assert_eq!(items[0].kind, Kind::Fn);
    }

    #[test]
    fn extracts_struct() {
        let src = "pub struct Foo { x: i32 }";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].kind, Kind::Struct);
        assert_eq!(items[0].name, "Foo");
    }

    #[test]
    fn extracts_enum() {
        let src = "pub enum Bar { A, B }";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].kind, Kind::Enum);
        assert_eq!(items[0].name, "Bar");
    }

    #[test]
    fn extracts_trait() {
        let src = "pub trait MyTrait { fn method(&self); }";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].kind, Kind::Trait);
        assert_eq!(items[0].name, "MyTrait");
    }

    #[test]
    fn extracts_impl_inherent() {
        let src = "impl Foo { fn new() -> Self { Foo } }";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].kind, Kind::Impl);
        assert_eq!(items[0].name, "impl Foo");
    }

    #[test]
    fn extracts_impl_trait() {
        let src = "impl MyTrait for Foo {}";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].kind, Kind::Impl);
        assert_eq!(items[0].name, "<Foo as MyTrait>");
    }

    #[test]
    fn extracts_mod() {
        let src = "pub mod inner { pub fn x() {} }";
        let items = extract_items(src).unwrap();
        // Only the mod itself — we don't recurse into inline mods.
        assert!(items.iter().any(|i| i.kind == Kind::Mod && i.name == "inner"));
    }

    #[test]
    fn extracts_const() {
        let src = "pub const MAX: usize = 42;";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].kind, Kind::Const);
        assert_eq!(items[0].name, "MAX");
    }

    #[test]
    fn returns_error_on_invalid_syntax() {
        let result = extract_items("fn foo( { }");
        assert!(result.is_err());
    }

    #[test]
    fn line_numbers_populated() {
        let src = "pub fn first() {}\npub fn second() {}";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].line_start, 1);
        assert_eq!(items[1].line_start, 2);
    }

    #[test]
    fn multiline_fn_has_distinct_line_end() {
        let src = "fn multi(\n    x: i32,\n) -> i32 {\n    x + 1\n}";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].line_start, 1);
        assert_eq!(items[0].line_end, 5);
    }

    #[test]
    fn multiline_struct_has_distinct_line_end() {
        let src = "pub struct Cfg {\n    host: String,\n    port: u16,\n}";
        let items = extract_items(src).unwrap();
        assert_eq!(items[0].line_start, 1);
        assert_eq!(items[0].line_end, 4);
    }

    #[test]
    fn extracts_multiple_items() {
        let src = r#"
pub struct Config { pub host: String }
pub fn run(cfg: Config) {}
pub const VERSION: &str = "1.0";
"#;
        let items = extract_items(src).unwrap();
        assert_eq!(items.len(), 3);
    }
}
