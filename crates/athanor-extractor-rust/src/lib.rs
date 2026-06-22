use std::collections::HashMap;

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use proc_macro2::Span;
use quote::ToTokens;
use serde_json::json;
use syn::visit::Visit;
use syn::{ImplItem, Item, Type, Visibility, spanned::Spanned};

#[derive(Debug, Clone, Default)]
pub struct RustExtractor;

#[async_trait]
impl Extractor for RustExtractor {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn supports(&self, source: &SourceFile) -> bool {
        source.language_hint.as_deref() == Some("rust")
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let syntax = syn::parse_file(content).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to parse Rust source {}: {error}",
                input.source.path
            ))
        })?;
        let mut symbols = Vec::new();
        let mut module_imports = HashMap::new();
        let file_module_path = module_path(&input.source.path);

        // Push the file's own module entity first
        symbols.push(RustSymbol {
            name: file_module_path
                .split("::")
                .last()
                .unwrap_or("crate")
                .to_string(),
            qualified_name: file_module_path.clone(),
            entity_kind: EntityKind::Module,
            symbol_kind: "module",
            visibility: "public".to_string(),
            signature: None,
            spans: vec![syntax.span()],
            imports: Vec::new(),
            calls: Vec::new(),
            is_test: false,
        });

        collect_items(
            &syntax.items,
            &file_module_path,
            &mut symbols,
            &mut module_imports,
        );

        for symbol in &mut symbols {
            if symbol.entity_kind != EntityKind::Module {
                continue;
            }
            if let Some(imports) = module_imports.get(&symbol.qualified_name) {
                symbol.imports = imports.clone();
            }
        }

        let symbols = deduplicate_symbols(symbols);

        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let mut entities = Vec::with_capacity(symbols.len());
        let mut facts = Vec::with_capacity(symbols.len());

        for symbol in symbols {
            let prefix = match symbol.entity_kind {
                EntityKind::TestCase => "ent_rust_test_case_",
                EntityKind::Module => "ent_rust_module_",
                EntityKind::Function => "ent_rust_function_",
                _ => "ent_rust_symbol_",
            };
            let stable_key = StableKey(format!("symbol://rust:{}", symbol.qualified_name));
            let entity_id = EntityId(format!(
                "{}{:016x}",
                prefix,
                stable_hash(stable_key.0.as_bytes())
            ));
            let (line_start, line_end) = symbol_line_range(&symbol.spans);
            let ownership = ownership_for_file(&input.source.path);

            entities.push(Entity {
                id: entity_id.clone(),
                stable_key: stable_key.clone(),
                kind: symbol.entity_kind,
                name: symbol.name,
                title: None,
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(line_start),
                    line_end: Some(line_end),
                }),
                language: Some(LanguageCode("rust".to_string())),
                aliases: Vec::new(),
                ownership: ownership.clone(),
                payload: json!({
                    "symbol_kind": symbol.symbol_kind,
                    "qualified_name": symbol.qualified_name,
                    "visibility": symbol.visibility,
                    "signature": symbol.signature,
                    "definitions": symbol.spans.len(),
                    "imports": symbol.imports,
                    "calls": symbol.calls,
                    "is_test": symbol.is_test,
                }),
            });

            facts.push(Fact {
                id: FactId(format!(
                    "fact_symbol_defined_{:016x}",
                    stable_hash(stable_key.0.as_bytes())
                )),
                kind: FactKind::SymbolDefined,
                subject: entity_id,
                object: Some(file_id.clone()),
                value: json!({
                    "stable_key": stable_key.0,
                    "path": input.source.path,
                    "symbol_kind": symbol.symbol_kind,
                }),
                evidence: symbol
                    .spans
                    .iter()
                    .map(|span| {
                        let (line_start, line_end) = span_lines(*span);
                        evidence_for_file(
                            &input.source.path,
                            self.name(),
                            Some(line_start),
                            Some(line_end),
                        )
                    })
                    .collect(),
                ownership,
                snapshot: input.snapshot.clone(),
                extractor: self.name().to_string(),
                confidence: 1.0,
            });
        }

        Ok(ExtractOutput { entities, facts })
    }
}

#[derive(Debug)]
struct RustSymbol {
    name: String,
    qualified_name: String,
    entity_kind: EntityKind,
    symbol_kind: &'static str,
    visibility: String,
    signature: Option<String>,
    spans: Vec<Span>,
    imports: Vec<String>,
    calls: Vec<String>,
    is_test: bool,
}

struct CallVisitor {
    calls: Vec<String>,
}

impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            self.calls.push(path_to_string(&expr_path.path));
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.calls.push(node.method.to_string());
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        self.calls.push(path_to_string(&node.path));
        syn::visit::visit_expr_path(self, node);
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn is_test_function(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || attr.path().is_ident("tokio::test")
            || attr.path().segments.iter().any(|seg| seg.ident == "test")
    })
}

fn expand_use_tree(prefix: &str, tree: &syn::UseTree, out: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            let next_prefix = if prefix.is_empty() {
                path.ident.to_string()
            } else {
                format!("{prefix}::{}", path.ident)
            };
            expand_use_tree(&next_prefix, &path.tree, out);
        }
        syn::UseTree::Name(name) => {
            let path = if prefix.is_empty() {
                name.ident.to_string()
            } else {
                format!("{prefix}::{}", name.ident)
            };
            out.push(path);
        }
        syn::UseTree::Rename(rename) => {
            let path = if prefix.is_empty() {
                rename.ident.to_string()
            } else {
                format!("{prefix}::{}", rename.ident)
            };
            out.push(path);
        }
        syn::UseTree::Glob(_) => {
            let path = if prefix.is_empty() {
                "*".to_string()
            } else {
                format!("{prefix}::*")
            };
            out.push(path);
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                expand_use_tree(prefix, item, out);
            }
        }
    }
}

fn collect_items(
    items: &[Item],
    parent: &str,
    symbols: &mut Vec<RustSymbol>,
    module_imports: &mut HashMap<String, Vec<String>>,
) {
    for item in items {
        match item {
            Item::Fn(item) => {
                let mut visitor = CallVisitor { calls: Vec::new() };
                visitor.visit_block(&item.block);
                let is_test = is_test_function(&item.attrs);
                let (entity_kind, symbol_kind) = if is_test {
                    (EntityKind::TestCase, "test_case")
                } else {
                    (EntityKind::Function, "function")
                };
                symbols.push(RustSymbol {
                    name: item.sig.ident.to_string(),
                    qualified_name: qualify(parent, &item.sig.ident.to_string()),
                    entity_kind,
                    symbol_kind,
                    visibility: visibility(&item.vis),
                    signature: Some(item.sig.to_token_stream().to_string()),
                    spans: vec![item.sig.span()],
                    imports: Vec::new(),
                    calls: visitor.calls,
                    is_test,
                });
            }
            Item::Mod(item) => {
                let name = item.ident.to_string();
                let qualified_name = qualify(parent, &name);
                symbols.push(RustSymbol {
                    name,
                    qualified_name: qualified_name.clone(),
                    entity_kind: EntityKind::Module,
                    symbol_kind: "module",
                    visibility: visibility(&item.vis),
                    signature: None,
                    spans: vec![item.span()],
                    imports: Vec::new(),
                    calls: Vec::new(),
                    is_test: false,
                });
                if let Some((_, items)) = &item.content {
                    collect_items(items, &qualified_name, symbols, module_imports);
                }
            }
            Item::Struct(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "struct",
                &item.vis,
                item.span(),
            )),
            Item::Enum(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "enum",
                &item.vis,
                item.span(),
            )),
            Item::Trait(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "trait",
                &item.vis,
                item.span(),
            )),
            Item::Union(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "union",
                &item.vis,
                item.span(),
            )),
            Item::Type(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "type_alias",
                &item.vis,
                item.span(),
            )),
            Item::Const(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "constant",
                &item.vis,
                item.span(),
            )),
            Item::Static(item) => symbols.push(type_symbol(
                parent,
                &item.ident.to_string(),
                "static",
                &item.vis,
                item.span(),
            )),
            Item::Impl(item) => collect_impl_items(item, parent, symbols),
            Item::Use(item) => {
                let mut imports = Vec::new();
                expand_use_tree("", &item.tree, &mut imports);
                module_imports
                    .entry(parent.to_string())
                    .or_default()
                    .extend(imports);
            }
            _ => {}
        }
    }
}

fn collect_impl_items(item: &syn::ItemImpl, parent: &str, symbols: &mut Vec<RustSymbol>) {
    let Some(owner) = type_name(&item.self_ty) else {
        return;
    };
    let owner = qualify(parent, &owner);
    let method_parent = item
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .map_or_else(
            || owner.clone(),
            |segment| qualify(&owner, &segment.ident.to_string()),
        );

    for impl_item in &item.items {
        if let ImplItem::Fn(method) = impl_item {
            let mut visitor = CallVisitor { calls: Vec::new() };
            visitor.visit_block(&method.block);
            let is_test = is_test_function(&method.attrs);
            let (entity_kind, symbol_kind) = if is_test {
                (EntityKind::TestCase, "test_case")
            } else {
                (EntityKind::Function, "method")
            };
            symbols.push(RustSymbol {
                name: method.sig.ident.to_string(),
                qualified_name: qualify(&method_parent, &method.sig.ident.to_string()),
                entity_kind,
                symbol_kind,
                visibility: visibility(&method.vis),
                signature: Some(method.sig.to_token_stream().to_string()),
                spans: vec![method.sig.span()],
                imports: Vec::new(),
                calls: visitor.calls,
                is_test,
            });
        }
    }
}

fn type_symbol(
    parent: &str,
    name: &str,
    symbol_kind: &'static str,
    visibility_value: &Visibility,
    span: Span,
) -> RustSymbol {
    RustSymbol {
        name: name.to_string(),
        qualified_name: qualify(parent, name),
        entity_kind: EntityKind::Symbol,
        symbol_kind,
        visibility: visibility(visibility_value),
        signature: None,
        spans: vec![span],
        imports: Vec::new(),
        calls: Vec::new(),
        is_test: false,
    }
}

fn module_path(path: &str) -> String {
    let mut components = path
        .trim_end_matches(".rs")
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    let crate_name =
        if let Some(src_index) = components.iter().rposition(|component| *component == "src") {
            let crate_name = src_index
                .checked_sub(1)
                .and_then(|index| components.get(index))
                .map_or_else(|| "crate".to_string(), |name| rust_identifier(name));
            components.drain(..=src_index);
            crate_name
        } else {
            "crate".to_string()
        };
    if components.last() == Some(&"lib")
        || components.last() == Some(&"main")
        || components.last() == Some(&"mod")
    {
        components.pop();
    }

    if components.is_empty() {
        crate_name
    } else {
        format!("{crate_name}::{}", components.join("::"))
    }
}

fn rust_identifier(name: &str) -> String {
    name.replace('-', "_")
}

fn deduplicate_symbols(symbols: Vec<RustSymbol>) -> Vec<RustSymbol> {
    let mut indexes: HashMap<String, usize> = HashMap::new();
    let mut unique: Vec<RustSymbol> = Vec::new();

    for symbol in symbols {
        if let Some(index) = indexes.get(&symbol.qualified_name).copied() {
            unique[index].spans.extend(symbol.spans);
            unique[index].calls.extend(symbol.calls);
            unique[index].imports.extend(symbol.imports);
        } else {
            indexes.insert(symbol.qualified_name.clone(), unique.len());
            unique.push(symbol);
        }
    }

    unique
}

fn type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => type_name(&reference.elem),
        _ => None,
    }
}

fn qualify(parent: &str, name: &str) -> String {
    format!("{parent}::{name}")
}

fn visibility(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public(_) => "public".to_string(),
        Visibility::Inherited => "private".to_string(),
        Visibility::Restricted(restricted) => restricted.to_token_stream().to_string(),
    }
}

fn span_lines(span: Span) -> (u32, u32) {
    (span.start().line as u32, span.end().line as u32)
}

fn symbol_line_range(spans: &[Span]) -> (u32, u32) {
    spans
        .iter()
        .map(|span| span_lines(*span))
        .fold((u32::MAX, 0), |(minimum, maximum), (start, end)| {
            (minimum.min(start), maximum.max(end))
        })
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityKind, FactKind, RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn extracts_rust_symbols_methods_and_definition_facts() {
        let output = RustExtractor
            .extract(input(
                "src/auth.rs",
                r#"
pub struct Session;

impl Session {
    pub fn refresh(&self) {}
}

pub async fn login() {}
"#,
            ))
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 4); // Session, refresh, login, plus root module auth
        assert_eq!(output.facts.len(), 4);
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "symbol://rust:crate::auth::Session"
                && entity.kind == EntityKind::Symbol
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "symbol://rust:crate::auth::Session::refresh"
                && entity.kind == EntityKind::Function
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "symbol://rust:crate::auth::login"
                && entity.source.as_ref().unwrap().line_start == Some(8)
        }));
        assert!(output.facts.iter().all(|fact| {
            fact.kind == FactKind::SymbolDefined
                && !fact.evidence.is_empty()
                && !fact.ownership.is_empty()
                && fact
                    .object
                    .as_ref()
                    .is_some_and(|id| id.0.starts_with("ent_file_"))
        }));
    }

    #[tokio::test]
    async fn extracts_inline_modules_and_qualified_trait_methods() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub mod auth {
    pub trait Login { fn login(&self); }
    pub struct Service;
    impl Login for Service {
        fn login(&self) {}
    }
}
"#,
            ))
            .await
            .unwrap();

        let keys = output
            .entities
            .iter()
            .map(|entity| entity.stable_key.0.as_str())
            .collect::<Vec<_>>();
        assert!(keys.contains(&"symbol://rust:crate"));
        assert!(keys.contains(&"symbol://rust:crate::auth"));
        assert!(keys.contains(&"symbol://rust:crate::auth::Login"));
        assert!(keys.contains(&"symbol://rust:crate::auth::Service::Login::login"));
    }

    #[tokio::test]
    async fn rejects_invalid_rust_source() {
        let error = RustExtractor
            .extract(input("src/lib.rs", "pub fn broken("))
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to parse Rust source src/lib.rs")
        );
    }

    #[test]
    fn derives_crate_module_paths_inside_workspaces() {
        assert_eq!(module_path("src/lib.rs"), "crate");
        assert_eq!(module_path("src/auth/mod.rs"), "crate::auth");
        assert_eq!(
            module_path("crates/example-core/src/auth.rs"),
            "example_core::auth"
        );
        assert_eq!(module_path("apps/ath/src/main.rs"), "ath");
    }

    #[tokio::test]
    async fn merges_cfg_alternatives_into_one_symbol_with_multiple_evidence() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
#[cfg(unix)]
fn platform() {}

#[cfg(windows)]
fn platform() {}
"#,
            ))
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 2); // Platform plus root module crate
        let symbol = output
            .entities
            .iter()
            .find(|e| e.name == "platform")
            .unwrap();
        assert_eq!(symbol.payload["definitions"], 2);
    }

    fn input(path: &str, content: &str) -> ExtractInput {
        ExtractInput {
            repo: RepoId("repo_test".to_string()),
            snapshot: SnapshotId("snap_test".to_string()),
            source: SourceFile {
                path: path.to_string(),
                language_hint: Some("rust".to_string()),
                content_hash: Some("hash".to_string()),
                content: Some(content.to_string()),
            },
        }
    }
}
