use std::collections::HashMap;

use async_trait::async_trait;
use athanor_core::{
    CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile,
};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use proc_macro2::Span;
use quote::ToTokens;
use serde_json::{Value, json};
use syn::visit::Visit;
use syn::{ImplItem, Item, Type, Visibility, spanned::Spanned};

#[derive(Debug, Clone, Default)]
pub struct RustExtractor;

#[async_trait]
impl Extractor for RustExtractor {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
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
            typed_bindings: Vec::new(),
            is_test: false,
            supertraits: Vec::new(),
        });

        collect_items(
            &syntax.items,
            &file_module_path,
            &mut symbols,
            &mut module_imports,
        );

        // Keep module imports on traits too: canonical module entities can coalesce across files,
        // so supertrait resolution should not depend on which module payload wins that merge.
        for symbol in &mut symbols {
            let module_qn = if symbol.entity_kind == EntityKind::Module {
                Some(symbol.qualified_name.as_str())
            } else if symbol.symbol_kind == "trait" {
                symbol
                    .qualified_name
                    .rsplit_once("::")
                    .map(|(module, _)| module)
            } else {
                None
            };
            if let Some(imports) = module_qn.and_then(|module| module_imports.get(module)) {
                symbol.imports = imports.clone();
            }
        }

        let symbols = deduplicate_symbols(symbols);
        let mut symbol_ids = HashMap::new();
        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let mut entities = Vec::new();
        let mut facts = Vec::new();

        for symbol in &symbols {
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
            symbol_ids.insert(symbol.qualified_name.clone(), entity_id);
        }

        for symbol in symbols {
            let stable_key = StableKey(format!("symbol://rust:{}", symbol.qualified_name));
            let entity_id = symbol_ids.get(&symbol.qualified_name).cloned().unwrap();
            let (line_start, line_end) = symbol_line_range(&symbol.spans);
            let ownership = ownership_for_file(&input.source.path);

            let mut payload = json!({
                "symbol_kind": symbol.symbol_kind,
                "qualified_name": symbol.qualified_name,
                "visibility": symbol.visibility,
                "signature": symbol.signature,
                "definitions": symbol.spans.len(),
                "imports": symbol.imports,
                "calls": symbol.calls,
                "is_test": symbol.is_test,
            });
            if !symbol.typed_bindings.is_empty() {
                payload["typed_bindings"] = json!(symbol.typed_bindings);
            }
            if symbol.symbol_kind == "trait" {
                payload["supertraits"] = json!(symbol.supertraits);
            }

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
                payload,
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

        // Trait implementations are recorded as facts (not entities) so marker impls
        // without methods survive; the Rust linker turns them into `Implements` bridges.
        let mut trait_impls = Vec::new();
        collect_trait_impls(&syntax.items, &mut trait_impls);
        for trait_impl in trait_impls {
            let (line_start, line_end) = span_lines(trait_impl.span);
            facts.push(Fact {
                id: FactId(format!(
                    "fact_trait_impl_{:016x}",
                    stable_hash(
                        format!(
                            "{}::{}->{}",
                            input.source.path, trait_impl.self_type, trait_impl.trait_path
                        )
                        .as_bytes()
                    )
                )),
                kind: FactKind::SymbolDefined,
                subject: file_id.clone(),
                object: None,
                value: json!({
                    "symbol_kind": "trait_impl",
                    "self_type": trait_impl.self_type,
                    "trait_name": trait_impl.trait_name,
                    "trait_path": trait_impl.trait_path,
                }),
                evidence: vec![evidence_for_file(
                    &input.source.path,
                    self.name(),
                    Some(line_start),
                    Some(line_end),
                )],
                ownership: ownership_for_file(&input.source.path),
                snapshot: input.snapshot.clone(),
                extractor: self.name().to_string(),
                confidence: 1.0,
            });
        }

        // Environment variables extraction
        let mut env_visitor = EnvVarVisitor {
            current_parent: file_module_path.clone(),
            usages: Vec::new(),
        };
        env_visitor.visit_file(&syntax);

        // Deduplicate env vars by name
        let mut unique_env_vars = HashMap::new();
        for usage in &env_visitor.usages {
            unique_env_vars.entry(usage.name.clone()).or_insert(usage);
        }

        for (name, first_usage) in unique_env_vars {
            let stable_key = StableKey(format!("env://{name}"));
            let entity_id = EntityId(format!(
                "ent_env_var_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            ));
            let (line_start, line_end) = span_lines(first_usage.span);
            let ownership = ownership_for_file(&input.source.path);

            entities.push(Entity {
                id: entity_id,
                stable_key,
                kind: EntityKind::EnvVar,
                name: name.clone(),
                title: None,
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(line_start),
                    line_end: Some(line_end),
                }),
                language: Some(LanguageCode("rust".to_string())),
                aliases: Vec::new(),
                ownership,
                payload: json!({
                    "name": name,
                }),
            });
        }

        for (i, usage) in env_visitor.usages.iter().enumerate() {
            let env_var_stable_key = StableKey(format!("env://{}", usage.name));
            let env_var_id = EntityId(format!(
                "ent_env_var_{:016x}",
                stable_hash(env_var_stable_key.0.as_bytes())
            ));
            let parent_id = symbol_ids
                .get(&usage.parent_symbol)
                .cloned()
                .unwrap_or_else(|| file_id.clone());

            let (line_start, line_end) = span_lines(usage.span);
            let ownership = ownership_for_file(&input.source.path);

            facts.push(Fact {
                id: FactId(format!(
                    "fact_env_var_used_{:016x}_{i}",
                    stable_hash(format!("{}->{}", usage.parent_symbol, usage.name).as_bytes())
                )),
                kind: FactKind::EnvVarUsed,
                subject: parent_id,
                object: Some(env_var_id),
                value: json!({
                    "name": usage.name,
                    "mechanism": usage.mechanism,
                }),
                evidence: vec![evidence_for_file(
                    &input.source.path,
                    self.name(),
                    Some(line_start),
                    Some(line_end),
                )],
                ownership,
                snapshot: input.snapshot.clone(),
                extractor: self.name().to_string(),
                confidence: 1.0,
            });
        }

        Ok(ExtractOutput {
            entities,
            facts,
            diagnostics: Vec::new(),
        })
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
    calls: Vec<Value>,
    typed_bindings: Vec<Value>,
    is_test: bool,
    supertraits: Vec<String>,
}

struct CallVisitor {
    calls: Vec<Value>,
}

impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            let callee = path_to_string(&expr_path.path);
            let name = expr_path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_default();
            self.calls.push(call_site(
                "call",
                callee,
                name,
                None,
                node.span().start().line as u32,
            ));
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let receiver = node.receiver.to_token_stream().to_string();
        let name = node.method.to_string();
        self.calls.push(call_site(
            "method",
            format!("{receiver}.{name}"),
            name,
            Some(receiver),
            node.span().start().line as u32,
        ));
        syn::visit::visit_expr_method_call(self, node);
    }
}

/// One statically named call site.
///
/// A method call keeps the receiver expression it was written with, because the
/// receiver's declared type is the evidence for which type's method runs.
fn call_site(
    kind: &'static str,
    callee: String,
    name: String,
    receiver: Option<String>,
    line: u32,
) -> Value {
    json!({
        "callee": callee,
        "name": name,
        "receiver": receiver,
        "kind": kind,
        "line": line,
    })
}

/// The receiver types one function states.
///
/// A parameter or an annotated local is evidence only when its declared type
/// names one type: a bare path, a `dyn Trait`, or an `impl Trait`, with
/// references and the usual smart pointers unwrapped. A generic argument, a
/// tuple, a `Self` receiver, a trait object with extra bounds, and an
/// unannotated local name nothing the linker may dispatch on.
fn typed_bindings(sig: Option<&syn::Signature>, block: Option<&syn::Block>) -> Vec<Value> {
    let mut rows = Vec::new();
    if let Some(sig) = sig {
        for argument in &sig.inputs {
            if let syn::FnArg::Typed(argument) = argument
                && let syn::Pat::Ident(pattern) = &*argument.pat
                && let Some((type_name, kind)) = receiver_type(&argument.ty)
            {
                rows.push(typed_binding(
                    pattern.ident.to_string(),
                    type_name,
                    kind,
                    argument.ty.span().start().line as u32,
                ));
            }
        }
    }
    if let Some(block) = block {
        BindingVisitor { rows: &mut rows }.visit_block(block);
    }
    rows
}

/// Collects the `let` bindings a body states, including the ones nested in
/// blocks and `match` arms.
struct BindingVisitor<'a> {
    rows: &'a mut Vec<Value>,
}

impl<'ast> Visit<'ast> for BindingVisitor<'_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Type(annotation) = &local.pat
            && let syn::Pat::Ident(pattern) = &*annotation.pat
            && let Some((type_name, kind)) = receiver_type(&annotation.ty)
        {
            self.rows.push(typed_binding(
                pattern.ident.to_string(),
                type_name,
                kind,
                annotation.ty.span().start().line as u32,
            ));
        }
        syn::visit::visit_local(self, local);
    }
}

fn typed_binding(name: String, type_name: String, kind: &'static str, line: u32) -> Value {
    json!({
        "name": name,
        "type": type_name,
        "kind": kind,
        "line": line,
    })
}

/// Reduces one declared type to the single type a receiver can dispatch on.
fn receiver_type(ty: &Type) -> Option<(String, &'static str)> {
    match ty {
        Type::Reference(reference) => receiver_type(&reference.elem),
        Type::Paren(paren) => receiver_type(&paren.elem),
        Type::TraitObject(trait_object) => {
            single_trait_bound(trait_object.bounds.iter()).map(|name| (name, "trait_object"))
        }
        Type::ImplTrait(impl_trait) => {
            single_trait_bound(impl_trait.bounds.iter()).map(|name| (name, "impl_trait"))
        }
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            // `Box<dyn Trait>`, `Arc<dyn Trait>`, and `Rc<dyn Trait>` hold the
            // receiver behind one indirection, so the type inside decides.
            if matches!(segment.ident.to_string().as_str(), "Box" | "Rc" | "Arc")
                && let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments
                && arguments.args.len() == 1
                && let Some(syn::GenericArgument::Type(inner)) = arguments.args.first()
            {
                return receiver_type(inner);
            }
            if !matches!(segment.arguments, syn::PathArguments::None) {
                return None;
            }
            let name = segment.ident.to_string();
            if is_primitive(&name) {
                return None;
            }
            Some((name, "annotation"))
        }
        _ => None,
    }
}

/// Reports a type that no crate declaration can own: a primitive or `Self`.
fn is_primitive(name: &str) -> bool {
    matches!(
        name,
        "Self"
            | "bool"
            | "char"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
    )
}

/// Reads the one trait a `dyn`/`impl` bound names, or nothing when the bound
/// carries more than a trait: `dyn Trait + Send` dispatches like no single type.
fn single_trait_bound<'a>(
    mut bounds: impl Iterator<Item = &'a syn::TypeParamBound>,
) -> Option<String> {
    let Some(syn::TypeParamBound::Trait(bound)) = bounds.next() else {
        return None;
    };
    if bounds.next().is_some() {
        return None;
    }
    let segment = bound.path.segments.last()?;
    if !matches!(segment.arguments, syn::PathArguments::None) {
        return None;
    }
    Some(segment.ident.to_string())
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
                    typed_bindings: typed_bindings(Some(&item.sig), Some(&item.block)),
                    is_test,
                    supertraits: Vec::new(),
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
                    typed_bindings: Vec::new(),
                    is_test: false,
                    supertraits: Vec::new(),
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
            Item::Trait(item) => {
                let mut trait_symbol = type_symbol(
                    parent,
                    &item.ident.to_string(),
                    "trait",
                    &item.vis,
                    item.span(),
                );
                trait_symbol.supertraits = trait_supertraits(item);
                symbols.push(trait_symbol);
                collect_trait_items(item, parent, symbols);
            }
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
                typed_bindings: typed_bindings(Some(&method.sig), Some(&method.block)),
                is_test,
                supertraits: Vec::new(),
            });
        }
    }
}

/// Records the methods a trait declares.
///
/// A `dyn Trait` receiver has nothing to dispatch to unless the trait's own
/// method declarations exist as entities, so `fn login(&self);` inside a trait
/// becomes a declaration qualified `Trait::login`, with the calls of a default
/// body when it has one.
fn collect_trait_items(item: &syn::ItemTrait, parent: &str, symbols: &mut Vec<RustSymbol>) {
    let trait_qualified = qualify(parent, &item.ident.to_string());
    for member in &item.items {
        let syn::TraitItem::Fn(method) = member else {
            continue;
        };
        let mut visitor = CallVisitor { calls: Vec::new() };
        if let Some(block) = &method.default {
            visitor.visit_block(block);
        }
        symbols.push(RustSymbol {
            name: method.sig.ident.to_string(),
            qualified_name: qualify(&trait_qualified, &method.sig.ident.to_string()),
            entity_kind: EntityKind::Function,
            symbol_kind: "trait_method",
            visibility: visibility(&item.vis),
            signature: Some(method.sig.to_token_stream().to_string()),
            spans: vec![method.sig.span()],
            imports: Vec::new(),
            calls: visitor.calls,
            typed_bindings: typed_bindings(Some(&method.sig), method.default.as_ref()),
            is_test: false,
            supertraits: Vec::new(),
        });
    }
}

fn trait_supertraits(item: &syn::ItemTrait) -> Vec<String> {
    item.supertraits
        .iter()
        .filter_map(|bound| match bound {
            syn::TypeParamBound::Trait(bound) => Some(path_to_string(&bound.path)),
            _ => None,
        })
        .collect()
}

#[derive(Debug)]
struct TraitImplRecord {
    self_type: String,
    trait_name: String,
    trait_path: String,
    span: Span,
}

/// Collect `impl Trait for Type` blocks (including nested modules) so trait
/// implementations become `Implements` bridges even when the impl has no methods.
fn collect_trait_impls(items: &[Item], out: &mut Vec<TraitImplRecord>) {
    for item in items {
        match item {
            Item::Impl(item) => {
                let Some((_, path, _)) = &item.trait_ else {
                    continue;
                };
                let Some(self_type) = type_name(&item.self_ty) else {
                    continue;
                };
                let Some(trait_segment) = path.segments.last() else {
                    continue;
                };
                out.push(TraitImplRecord {
                    self_type,
                    trait_name: trait_segment.ident.to_string(),
                    trait_path: path_to_string(path),
                    span: item.span(),
                });
            }
            Item::Mod(item) => {
                if let Some((_, items)) = &item.content {
                    collect_trait_impls(items, out);
                }
            }
            _ => {}
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
        typed_bindings: Vec::new(),
        is_test: false,
        supertraits: Vec::new(),
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
            for supertrait in symbol.supertraits {
                if !unique[index].supertraits.contains(&supertrait) {
                    unique[index].supertraits.push(supertrait);
                }
            }
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

struct EnvVarUsage {
    name: String,
    mechanism: String,
    span: Span,
    parent_symbol: String,
}

struct EnvVarVisitor {
    current_parent: String,
    usages: Vec<EnvVarUsage>,
}

impl<'ast> Visit<'ast> for EnvVarVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.current_parent = old_parent;
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.ident.to_string());
        syn::visit::visit_item_mod(self, node);
        self.current_parent = old_parent;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_parent = old_parent;
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if let Some(owner) = type_name(&node.self_ty) {
            let old_parent = self.current_parent.clone();
            let owner_qualified = qualify(&old_parent, &owner);
            self.current_parent = node
                .trait_
                .as_ref()
                .and_then(|(_, path, _)| path.segments.last())
                .map_or_else(
                    || owner_qualified.clone(),
                    |segment| qualify(&owner_qualified, &segment.ident.to_string()),
                );
            syn::visit::visit_item_impl(self, node);
            self.current_parent = old_parent;
        } else {
            syn::visit::visit_item_impl(self, node);
        }
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.ident.to_string());
        syn::visit::visit_item_struct(self, node);
        self.current_parent = old_parent;
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.ident.to_string());
        syn::visit::visit_item_enum(self, node);
        self.current_parent = old_parent;
    }

    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.ident.to_string());
        syn::visit::visit_item_const(self, node);
        self.current_parent = old_parent;
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        let old_parent = self.current_parent.clone();
        self.current_parent = qualify(&old_parent, &node.ident.to_string());
        syn::visit::visit_item_static(self, node);
        self.current_parent = old_parent;
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        let macro_name = path_to_string(&node.mac.path);
        if macro_name == "env" || macro_name == "option_env" {
            let parsed = syn::parse2::<syn::LitStr>(node.mac.tokens.clone());
            if let Ok(lit) = parsed {
                self.usages.push(EnvVarUsage {
                    name: lit.value(),
                    mechanism: macro_name,
                    span: lit.span(),
                    parent_symbol: self.current_parent.clone(),
                });
            }
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            let func_path = path_to_string(&expr_path.path);
            if func_path == "std::env::var"
                || func_path == "env::var"
                || func_path == "std::env::var_os"
                || func_path == "env::var_os"
            {
                let first_arg = node.args.first();
                if let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                })) = first_arg
                {
                    self.usages.push(EnvVarUsage {
                        name: lit_str.value(),
                        mechanism: func_path,
                        span: lit_str.span(),
                        parent_symbol: self.current_parent.clone(),
                    });
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
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
    async fn extracts_inline_modules_and_qualified_trait_impl_methods() {
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
    async fn extracts_rust_trait_supertraits() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub mod base {
    pub trait Reader {}
}
use base::Reader;
pub trait BufferedReader: Reader {}
"#,
            ))
            .await
            .unwrap();

        let buffered = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://rust:crate::BufferedReader")
            .expect("trait declaration must be emitted");
        assert_eq!(
            buffered.payload["supertraits"],
            serde_json::json!(["Reader"])
        );
        assert_eq!(
            buffered.payload["imports"],
            serde_json::json!(["base::Reader"])
        );
    }

    #[tokio::test]
    async fn records_trait_impl_facts_for_types_and_nested_modules() {
        let output = RustExtractor
            .extract(input(
                "src/auth.rs",
                r#"
pub trait Login { fn login(&self); }
pub trait Marker {}
pub struct Session;
impl Login for Session {
    fn login(&self) {}
}
impl Marker for Session {}
pub mod nested {
    pub struct Token;
    impl super::Marker for Token {}
}
"#,
            ))
            .await
            .unwrap();

        let impls = output
            .facts
            .iter()
            .filter(|fact| {
                fact.value
                    .get("symbol_kind")
                    .and_then(|value| value.as_str())
                    == Some("trait_impl")
            })
            .collect::<Vec<_>>();
        assert_eq!(impls.len(), 3);
        assert!(impls.iter().any(|fact| {
            fact.value["self_type"] == "Session"
                && fact.value["trait_name"] == "Login"
                && fact.value["trait_path"] == "Login"
        }));
        // Marker impls have no methods, so the fact is the only anchor.
        assert!(impls.iter().any(|fact| {
            fact.value["self_type"] == "Session" && fact.value["trait_name"] == "Marker"
        }));
        assert!(impls.iter().any(|fact| {
            fact.value["self_type"] == "Token" && fact.value["trait_name"] == "Marker"
        }));
        assert!(impls.iter().all(|fact| {
            !fact.evidence.is_empty()
                && fact.evidence[0].source_file.as_deref() == Some("src/auth.rs")
                && fact.extractor == "rust"
        }));
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

    #[tokio::test]
    async fn extracts_environment_variables() {
        let output = RustExtractor
            .extract(input(
                "src/main.rs",
                r#"
                const DATABASE: &str = env!("DATABASE_URL");

                pub fn run() {
                    let port = std::env::var("PORT").unwrap();
                    let debug = env::var_os("DEBUG");
                }

                struct App;
                impl App {
                    fn get_host(&self) -> Option<&'static str> {
                        option_env!("HOST")
                    }
                }
                "#,
            ))
            .await
            .unwrap();

        let env_vars = output
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::EnvVar)
            .map(|e| e.stable_key.0.as_str())
            .collect::<Vec<_>>();

        assert_eq!(env_vars.len(), 4);
        assert!(env_vars.contains(&"env://DATABASE_URL"));
        assert!(env_vars.contains(&"env://PORT"));
        assert!(env_vars.contains(&"env://DEBUG"));
        assert!(env_vars.contains(&"env://HOST"));

        // Check that DATABASE_URL is used by the file itself/root module const
        let database_fact = output
            .facts
            .iter()
            .find(|f| f.kind == FactKind::EnvVarUsed && f.value["name"] == "DATABASE_URL")
            .unwrap();
        // PORT is used by symbol run
        let port_fact = output
            .facts
            .iter()
            .find(|f| f.kind == FactKind::EnvVarUsed && f.value["name"] == "PORT")
            .unwrap();
        // HOST is used by get_host method
        let host_fact = output
            .facts
            .iter()
            .find(|f| f.kind == FactKind::EnvVarUsed && f.value["name"] == "HOST")
            .unwrap();

        assert_eq!(database_fact.value["mechanism"], "env");
        assert_eq!(port_fact.value["mechanism"], "std::env::var");
        assert_eq!(host_fact.value["mechanism"], "option_env");
    }

    #[tokio::test]
    async fn records_method_call_sites_with_their_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn write(&self) {
        commit();
    }
}
fn commit() {}
fn run(store: Store) {
    store.write();
}
"#,
            ))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.name == "run")
            .expect("run must be extracted");
        let calls = run.payload["calls"].as_array().expect("calls array");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["kind"], "method");
        assert_eq!(calls[0]["callee"], "store.write");
        assert_eq!(calls[0]["name"], "write");
        assert_eq!(calls[0]["receiver"], "store");

        let write = output
            .entities
            .iter()
            .find(|entity| entity.name == "write")
            .expect("write must be extracted");
        let inner = write.payload["calls"].as_array().expect("calls array");
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0]["kind"], "call");
        assert_eq!(inner[0]["callee"], "commit");
        assert!(inner[0]["receiver"].is_null());
    }

    #[tokio::test]
    async fn records_typed_receiver_bindings() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub trait Reader {}
pub struct Store;
pub fn run(
    reader: &dyn Reader,
    boxed: Box<dyn Reader>,
    bound: &impl Reader,
    store: Store,
    generic: Vec<Store>,
    extra: &dyn Reader + Send,
    plain: u32,
) {
    let local: Store = Store;
    let inferred = Store;
    if plain > 0 {
        let nested: Store = Store;
    }
}
"#,
            ))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.name == "run")
            .expect("run must be extracted");
        let bindings = run.payload["typed_bindings"]
            .as_array()
            .expect("typed bindings array");
        let mut rows = Vec::new();
        for binding in bindings {
            rows.push(format!(
                "{}:{}:{}",
                binding["name"].as_str().unwrap_or_default(),
                binding["type"].as_str().unwrap_or_default(),
                binding["kind"].as_str().unwrap_or_default()
            ));
        }
        rows.sort();
        assert_eq!(
            rows,
            vec![
                "bound:Reader:impl_trait".to_string(),
                "boxed:Reader:trait_object".to_string(),
                "local:Store:annotation".to_string(),
                "nested:Store:annotation".to_string(),
                "reader:Reader:trait_object".to_string(),
                "store:Store:annotation".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn records_the_methods_a_trait_declares() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub trait Reader {
    fn read(&self);
    fn reset(&mut self) {}
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
        assert!(keys.contains(&"symbol://rust:crate::Reader::read"));
        assert!(keys.contains(&"symbol://rust:crate::Reader::reset"));
        let reset = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://rust:crate::Reader::reset")
            .expect("reset must be extracted");
        assert_eq!(reset.payload["symbol_kind"], "trait_method");
        assert_eq!(reset.kind, EntityKind::Function);
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
