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
            omitted_typed_bindings: 0,
            call_bindings: Vec::new(),
            omitted_call_bindings: 0,
            is_test: false,
            supertraits: Vec::new(),
            aliased_type: None,
            return_type: None,
            return_kind: None,
            try_yield: false,
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
                payload["omitted_typed_bindings"] = json!(symbol.omitted_typed_bindings);
            }
            if !symbol.call_bindings.is_empty() {
                payload["call_bindings"] = json!(symbol.call_bindings);
                payload["omitted_call_bindings"] = json!(symbol.omitted_call_bindings);
            }
            if symbol.symbol_kind == "trait" {
                payload["supertraits"] = json!(symbol.supertraits);
            }
            if let Some(aliased_type) = &symbol.aliased_type {
                payload["aliased_type"] = json!(aliased_type);
            }
            if let Some(return_type) = &symbol.return_type {
                payload["return_type"] = json!(return_type);
            }
            if let Some(return_kind) = symbol.return_kind {
                payload["return_kind"] = json!(return_kind);
            }
            if symbol.try_yield {
                payload["try_yield"] = json!(true);
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
    omitted_typed_bindings: usize,
    call_bindings: Vec<Value>,
    omitted_call_bindings: usize,
    is_test: bool,
    supertraits: Vec<String>,
    /// The one named type a `type Client = Store` alias writes.
    aliased_type: Option<String>,
    /// The bare named type a function or method declares it returns: `fn create() -> Store`.
    return_type: Option<String>,
    /// How that return type dispatches: omitted for a concrete annotation,
    /// `impl_trait` / `trait_object` when the signature names a trait bound.
    return_kind: Option<&'static str>,
    /// The signature returns `Result<T, E>` or `Option<T>`, so `T` is only the
    /// yield of a trailing `?`.
    try_yield: bool,
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
        let receiver = normalize_receiver_expr(&node.receiver.to_token_stream().to_string());
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

/// Collapses pretty-printed token spacing so `self . reader` records as
/// `self.reader`, `svc . create ()` as `svc.create()`, `Store :: new ()` as
/// `Store::new()`, and peels wrapping parentheses so `(&mut self.reader)` is
/// `&mut self.reader`. Spaces that are not around `.`, `::`, or `()` stay, so
/// `&mut self` is not smashed into `&mutself`.
fn normalize_receiver_expr(text: &str) -> String {
    let mut text = compact_receiver_dots(text);
    while let Some(inner) = text
        .trim()
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
    {
        text = compact_receiver_dots(inner);
    }
    text
}

fn compact_receiver_dots(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" . ", ".")
        .replace(" .", ".")
        .replace(". ", ".")
        .replace(" :: ", "::")
        .replace(" ::", "::")
        .replace(":: ", "::")
        .replace(" ()", "()")
}

/// One statically named call site.
///
/// A method call keeps the receiver expression with spaces around `.` collapsed,
/// because the receiver's declared type is the evidence for which type's method
/// runs.
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

/// The most receiver rows one declaration records.
const MAX_TYPED_BINDINGS: usize = 64;

/// The receiver types one function states.
///
/// A parameter or an annotated local is evidence only when its declared type
/// names one type: a bare path, a `dyn Trait`, or an `impl Trait`, with
/// references and the usual smart pointers unwrapped. A type parameter that
/// names one trait bound is that trait: `T: Reader` types a `T` receiver as
/// `impl Reader`. A generic argument, a tuple, a `Self` receiver, a trait
/// object with extra bounds, and an unannotated local name nothing the linker
/// may dispatch on.
fn typed_bindings(
    sig: Option<&syn::Signature>,
    block: Option<&syn::Block>,
    outer: Option<&syn::Generics>,
) -> (Vec<Value>, usize, Vec<Value>, usize) {
    let mut scope = outer.map(type_param_scope).unwrap_or_default();
    if let Some(sig) = sig {
        for (name, bound) in type_param_scope(&sig.generics) {
            scope.insert(name, bound);
        }
    }
    let mut rows = Vec::new();
    let mut matched = 0usize;
    let mut call_bindings = Vec::new();
    let mut call_matched = 0usize;
    if let Some(sig) = sig {
        for argument in &sig.inputs {
            if let syn::FnArg::Typed(argument) = argument
                && let syn::Pat::Ident(pattern) = &*argument.pat
                && let Some((type_name, kind)) = receiver_type(&argument.ty)
            {
                matched += 1;
                if rows.len() < MAX_TYPED_BINDINGS {
                    let (type_name, kind) = apply_type_param_bound(type_name, kind, &scope);
                    rows.push(typed_binding(
                        pattern.ident.to_string(),
                        type_name,
                        kind,
                        argument.ty.span().start().line as u32,
                    ));
                }
            }
        }
    }
    if let Some(block) = block {
        BindingVisitor {
            rows: &mut rows,
            matched: &mut matched,
            call_bindings: &mut call_bindings,
            call_matched: &mut call_matched,
            scope: &scope,
        }
        .visit_block(block);
    }
    let omitted = matched.saturating_sub(rows.len());
    let omitted_call = call_matched.saturating_sub(call_bindings.len());
    (rows, omitted, call_bindings, omitted_call)
}

/// Collects the `let` bindings a body states, including the ones nested in
/// blocks and `match` arms.
struct BindingVisitor<'a> {
    rows: &'a mut Vec<Value>,
    matched: &'a mut usize,
    call_bindings: &'a mut Vec<Value>,
    call_matched: &'a mut usize,
    scope: &'a HashMap<String, Option<String>>,
}

impl<'ast> Visit<'ast> for BindingVisitor<'_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Type(annotation) = &local.pat
            && let syn::Pat::Ident(pattern) = &*annotation.pat
            && let Some((type_name, kind)) = receiver_type(&annotation.ty)
        {
            *self.matched += 1;
            if self.rows.len() < MAX_TYPED_BINDINGS {
                let (type_name, kind) = apply_type_param_bound(type_name, kind, self.scope);
                self.rows.push(typed_binding(
                    pattern.ident.to_string(),
                    type_name,
                    kind,
                    annotation.ty.span().start().line as u32,
                ));
            }
        } else if let syn::Pat::Ident(pattern) = &local.pat
            && let Some(init) = &local.init
            && let Some((callee, tried)) = factory_init_callee(&init.expr)
        {
            *self.call_matched += 1;
            if self.call_bindings.len() < MAX_TYPED_BINDINGS {
                self.call_bindings.push(call_binding(
                    pattern.ident.to_string(),
                    callee,
                    init.expr.span().start().line as u32,
                    tried,
                ));
            }
        }
        syn::visit::visit_local(self, local);
    }
}

/// The factory an unannotated `let store = create_repo()` names.
///
/// One identifier (`create_repo()`), one associated function (`Store::new()`),
/// one member factory (`svc.create()` / `self.create()`), a `crate::` /
/// `self::` / `super::` path (`crate::create_repo()`, `crate::Store::new()`),
/// or one extra hop (`create_repo().get()`, `Store::new().get()`,
/// `svc.create().get()`, `crate::create_repo().get()`). A trailing `?`
/// (`create_repo()?`) or trailing `.unwrap()` / `.expect(...)` is peeled and
/// recorded as `try`. A `?` or unwrap/expect on the inner hop
/// (`create_repo()?.get()`, `create_repo().unwrap().get()`) stays in the callee
/// as `create_repo()?.get`. A third hop and a generic instantiation name nothing.
fn factory_init_callee(expr: &syn::Expr) -> Option<(String, bool)> {
    let (current, tried) = peel_init_try(expr);
    let callee = match current {
        syn::Expr::Call(call) => path_factory_callee(&call.func)?,
        syn::Expr::MethodCall(call) if call.turbofish.is_none() => {
            nested_factory_init_callee(&call.receiver, &call.method)
                .or_else(|| member_factory_init_callee(&call.receiver, &call.method))?
        }
        _ => return None,
    };
    Some((callee, tried))
}

fn peel_expr_paren(expr: &syn::Expr) -> &syn::Expr {
    let mut current = expr;
    while let syn::Expr::Paren(paren) = current {
        current = &paren.expr;
    }
    current
}

/// Peels wrapping parentheses, a trailing `?`, and a trailing `.unwrap()` /
/// `.expect(...)` so `create_repo()?`, `create_repo().unwrap()`, and
/// `create_repo().expect("failed")` are the same factory. An inner unwrap is
/// peeled by `nested_factory_init_callee`.
fn peel_init_try(expr: &syn::Expr) -> (&syn::Expr, bool) {
    let mut current = expr;
    let mut tried = false;
    loop {
        let (next, t) = peel_trailing_try(current);
        tried |= t;
        let (next, unwrapped) = peel_once_unwrap_or_expect(next);
        if !unwrapped {
            return (next, tried);
        }
        tried = true;
        current = next;
    }
}

/// Peels wrapping parentheses and a trailing `?` so `create_repo()?` and
/// `(create_repo())?` are the same factory. A `?` on an inner hop is recorded
/// in the callee by `nested_factory_init_callee`.
fn peel_trailing_try(expr: &syn::Expr) -> (&syn::Expr, bool) {
    let mut current = peel_expr_paren(expr);
    let mut tried = false;
    while let syn::Expr::Try(try_expr) = current {
        tried = true;
        current = peel_expr_paren(&try_expr.expr);
    }
    (current, tried)
}

/// One trailing `.unwrap()` with no arguments, or `.expect(...)`.
fn peel_once_unwrap_or_expect(expr: &syn::Expr) -> (&syn::Expr, bool) {
    let current = peel_expr_paren(expr);
    let syn::Expr::MethodCall(call) = current else {
        return (current, false);
    };
    if call.turbofish.is_some() {
        return (current, false);
    }
    match call.method.to_string().as_str() {
        "unwrap" if call.args.is_empty() => (peel_expr_paren(&call.receiver), true),
        "expect" => (peel_expr_paren(&call.receiver), true),
        _ => (current, false),
    }
}

fn path_factory_callee(func: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(path) = func else {
        return None;
    };
    if path.path.leading_colon.is_some() {
        return None;
    }
    if path
        .path
        .segments
        .iter()
        .any(|segment| !matches!(segment.arguments, syn::PathArguments::None))
    {
        return None;
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    match segments.as_slice() {
        [name] => Some(name.clone()),
        [type_name, method]
            if !matches!(type_name.as_str(), "crate" | "super" | "self") =>
        {
            Some(format!("{type_name}::{method}"))
        }
        [first, rest @ ..]
            if matches!(first.as_str(), "crate" | "self" | "super") && !rest.is_empty() =>
        {
            Some(segments.join("::"))
        }
        _ => None,
    }
}

/// One extra hop: `create_repo().get`, `Store::new().get`, `svc.create().get`,
/// `crate::create_repo().get`. A `?` or unwrap/expect on the inner hop records
/// `create_repo()?.get`.
///
/// The inner factory is one hop only. `create_repo().get().extra()` names nothing.
fn nested_factory_init_callee(receiver: &syn::Expr, method: &syn::Ident) -> Option<String> {
    let (receiver, inner_tried) = peel_init_try(receiver);
    let inner = match receiver {
        syn::Expr::Call(call) => path_factory_callee(&call.func)?,
        syn::Expr::MethodCall(call) if call.turbofish.is_none() => {
            member_factory_init_callee(&call.receiver, &call.method)?
        }
        _ => return None,
    };
    if inner_tried {
        Some(format!("{inner}()?.{method}"))
    } else {
        Some(format!("{inner}().{method}"))
    }
}

fn member_factory_init_callee(receiver: &syn::Expr, method: &syn::Ident) -> Option<String> {
    let receiver = peel_expr_paren(receiver);
    let syn::Expr::Path(path) = receiver else {
        return None;
    };
    if path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    if !matches!(segment.arguments, syn::PathArguments::None) {
        return None;
    }
    Some(format!("{}.{}", segment.ident, method))
}

fn call_binding(name: String, callee: String, line: u32, tried: bool) -> Value {
    let mut value = json!({
        "name": name,
        "callee": callee,
        "line": line,
    });
    if tried {
        value["try"] = json!(true);
    }
    value
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

/// Reads the type a function or method declares it returns.
///
/// The reduction is the same one a parameter uses: `fn create() -> Store`
/// records `Store`, `-> Box<Store>` and `-> &Store` unwrap to `Store`,
/// `-> impl Reader` and `-> Box<dyn Reader>` record `Reader` as a trait bound,
/// and `fn create<T: Reader>() -> T` substitutes that bound. `-> Result<Store, E>`
/// and `-> Option<Store>` record `Store` as a try yield, so only a trailing `?`
/// may take it. On an inherent impl, `-> Self` is the impl type. An unconstrained
/// type parameter, a tuple, extra trait bounds, a missing `->`, and `Self` outside
/// an inherent impl name nothing.
fn function_return_type(
    sig: &syn::Signature,
    extra: Option<&syn::Generics>,
    self_type: Option<&str>,
) -> Option<(String, &'static str, bool)> {
    let syn::ReturnType::Type(_, ty) = &sig.output else {
        return None;
    };
    let (ty, try_yield) = match try_ok_type(ty) {
        Some(inner) => (inner, true),
        None => (ty.as_ref(), false),
    };
    let (name, kind) = match receiver_type(ty) {
        Some(pair) => pair,
        None => {
            let self_type = self_type.filter(|name| !is_primitive(name))?;
            if !is_self_path(ty) {
                return None;
            }
            (self_type.to_string(), "annotation")
        }
    };
    let mut scope = extra.map(type_param_scope).unwrap_or_default();
    for (param, bound) in type_param_scope(&sig.generics) {
        scope.insert(param, bound);
    }
    let (name, kind) = apply_type_param_bound(name, kind, &scope);
    if scope.contains_key(&name) {
        return None;
    }
    Some((name, kind, try_yield))
}

/// `Self`, `&Self`, and `Box<Self>` after the same peels `receiver_type` uses.
fn is_self_path(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => is_self_path(&reference.elem),
        Type::Paren(paren) => is_self_path(&paren.elem),
        Type::Path(path) => {
            if path.path.leading_colon.is_some() {
                return false;
            }
            let Some(segment) = path.path.segments.last() else {
                return false;
            };
            if matches!(segment.ident.to_string().as_str(), "Box" | "Rc" | "Arc")
                && let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments
                && arguments.args.len() == 1
                && let Some(syn::GenericArgument::Type(inner)) = arguments.args.first()
            {
                return is_self_path(inner);
            }
            path.path.segments.len() == 1
                && matches!(segment.arguments, syn::PathArguments::None)
                && segment.ident == "Self"
        }
        _ => false,
    }
}

/// The Ok/Some type a `Result<T, E>` or `Option<T>` return yields through `?`.
///
/// One outer constructor only. `Box<Result<T, E>>` and a custom `MyResult<T>`
/// name nothing.
fn try_ok_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if !matches!(segment.ident.to_string().as_str(), "Result" | "Option") {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

fn recorded_return(
    sig: &syn::Signature,
    extra: Option<&syn::Generics>,
    self_type: Option<&str>,
) -> (Option<String>, Option<&'static str>, bool) {
    match function_return_type(sig, extra, self_type) {
        Some((name, "annotation", try_yield)) => (Some(name), None, try_yield),
        Some((name, kind, try_yield)) => (Some(name), Some(kind), try_yield),
        None => (None, None, false),
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

/// The type parameters a declaration names, mapped to the one trait they bound.
///
/// An unconstrained parameter, two bounds, and a generic bound (`T: Into<U>`)
/// record no trait. A `where T: Reader` clause is the same evidence as
/// `T: Reader` on the parameter.
fn type_param_scope(generics: &syn::Generics) -> HashMap<String, Option<String>> {
    let mut scope = HashMap::new();
    for param in &generics.params {
        let syn::GenericParam::Type(param) = param else {
            continue;
        };
        let name = param.ident.to_string();
        scope.insert(name, single_trait_bound(param.bounds.iter()));
    }
    let Some(where_clause) = &generics.where_clause else {
        return scope;
    };
    for predicate in &where_clause.predicates {
        let syn::WherePredicate::Type(predicate) = predicate else {
            continue;
        };
        let Type::Path(path) = &predicate.bounded_ty else {
            continue;
        };
        if path.qself.is_some() || path.path.segments.len() != 1 {
            continue;
        }
        let Some(segment) = path.path.segments.first() else {
            continue;
        };
        if !matches!(segment.arguments, syn::PathArguments::None) {
            continue;
        }
        let name = segment.ident.to_string();
        if !scope.contains_key(&name) {
            continue;
        }
        let bound = single_trait_bound(predicate.bounds.iter());
        match (scope.get(&name).cloned().flatten(), bound) {
            (Some(_), Some(_)) | (Some(_), None) => {
                scope.insert(name, None);
            }
            (None, bound) => {
                scope.insert(name, bound);
            }
        }
    }
    scope
}

/// Substitutes a type-parameter name for the one trait it bounds.
fn apply_type_param_bound(
    type_name: String,
    kind: &'static str,
    scope: &HashMap<String, Option<String>>,
) -> (String, &'static str) {
    match scope.get(&type_name) {
        Some(Some(bound)) => (bound.clone(), "impl_trait"),
        _ => (type_name, kind),
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
                let (typed_bindings, omitted_typed_bindings, call_bindings, omitted_call_bindings) =
                    typed_bindings(Some(&item.sig), Some(&item.block), None);
                let (return_type, return_kind, try_yield) =
                    recorded_return(&item.sig, None, None);
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
                    typed_bindings,
                    omitted_typed_bindings,
                    call_bindings,
                    omitted_call_bindings,
                    is_test,
                    supertraits: Vec::new(),
                    aliased_type: None,
                    return_type,
                    return_kind,
                    try_yield,
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
                    omitted_typed_bindings: 0,
                    call_bindings: Vec::new(),
                    omitted_call_bindings: 0,
                    is_test: false,
                    supertraits: Vec::new(),
                    aliased_type: None,
                    return_type: None,
                    return_kind: None,
                    try_yield: false,
                });
                if let Some((_, items)) = &item.content {
                    collect_items(items, &qualified_name, symbols, module_imports);
                }
            }
            Item::Struct(item) => {
                let mut symbol = type_symbol(
                    parent,
                    &item.ident.to_string(),
                    "struct",
                    &item.vis,
                    item.span(),
                );
                let (typed_bindings, omitted_typed_bindings) = struct_field_bindings(item);
                symbol.typed_bindings = typed_bindings;
                symbol.omitted_typed_bindings = omitted_typed_bindings;
                symbols.push(symbol);
            }
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
            Item::Type(item) => {
                let mut symbol = type_symbol(
                    parent,
                    &item.ident.to_string(),
                    "type_alias",
                    &item.vis,
                    item.span(),
                );
                symbol.aliased_type = aliased_type(&item.ty);
                symbols.push(symbol);
            }
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
    let Some(owner_name) = type_name(&item.self_ty) else {
        return;
    };
    let owner = qualify(parent, &owner_name);
    let self_type = item.trait_.is_none().then_some(owner_name.as_str());
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
            let (typed_bindings, omitted_typed_bindings, call_bindings, omitted_call_bindings) =
                typed_bindings(Some(&method.sig), Some(&method.block), Some(&item.generics));
            let (return_type, return_kind, try_yield) =
                recorded_return(&method.sig, Some(&item.generics), self_type);
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
                typed_bindings,
                omitted_typed_bindings,
                call_bindings,
                omitted_call_bindings,
                is_test,
                supertraits: Vec::new(),
                aliased_type: None,
                return_type,
                return_kind,
                try_yield,
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
        let (typed_bindings, omitted_typed_bindings, call_bindings, omitted_call_bindings) =
            typed_bindings(
                Some(&method.sig),
                method.default.as_ref(),
                Some(&item.generics),
            );
        let (return_type, return_kind, try_yield) =
            recorded_return(&method.sig, Some(&item.generics), None);
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
            typed_bindings,
            omitted_typed_bindings,
            call_bindings,
            omitted_call_bindings,
            is_test: false,
            supertraits: Vec::new(),
            aliased_type: None,
            return_type,
            return_kind,
            try_yield,
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

/// Named struct fields the linker can walk from `self.field`.
///
/// A tuple struct, a unit struct, and a field whose type names nothing stay
/// unrecorded. A type parameter that names one trait bound is that trait, the
/// same substitution a function parameter uses.
fn struct_field_bindings(item: &syn::ItemStruct) -> (Vec<Value>, usize) {
    let syn::Fields::Named(fields) = &item.fields else {
        return (Vec::new(), 0);
    };
    let scope = type_param_scope(&item.generics);
    let mut rows = Vec::new();
    let mut matched = 0usize;
    for field in &fields.named {
        let Some(ident) = &field.ident else {
            continue;
        };
        let Some((type_name, kind)) = receiver_type(&field.ty) else {
            continue;
        };
        matched += 1;
        if rows.len() < MAX_TYPED_BINDINGS {
            let (type_name, kind) = apply_type_param_bound(type_name, kind, &scope);
            rows.push(typed_binding(
                ident.to_string(),
                type_name,
                kind,
                field.ty.span().start().line as u32,
            ));
        }
    }
    let omitted = matched.saturating_sub(rows.len());
    (rows, omitted)
}

/// The one named type a `type Client = Store` alias writes.
///
/// Only a bare path without type arguments is evidence: `Vec<Store>`, a
/// reference, a tuple, a `dyn`/`impl` bound, and a primitive name nothing the
/// linker can unfold.
fn aliased_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Paren(paren) => aliased_type(&paren.elem),
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            if !matches!(segment.arguments, syn::PathArguments::None) {
                return None;
            }
            let name = segment.ident.to_string();
            if is_primitive(&name) {
                return None;
            }
            Some(name)
        }
        _ => None,
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
        omitted_typed_bindings: 0,
        call_bindings: Vec::new(),
        omitted_call_bindings: 0,
        is_test: false,
        supertraits: Vec::new(),
        aliased_type: None,
        return_type: None,
        return_kind: None,
        try_yield: false,
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
            unique[index].typed_bindings.extend(symbol.typed_bindings);
            unique[index].omitted_typed_bindings = unique[index]
                .omitted_typed_bindings
                .saturating_add(symbol.omitted_typed_bindings);
            unique[index].call_bindings.extend(symbol.call_bindings);
            unique[index].omitted_call_bindings = unique[index]
                .omitted_call_bindings
                .saturating_add(symbol.omitted_call_bindings);
            if unique[index].return_type != symbol.return_type
                || unique[index].return_kind != symbol.return_kind
                || unique[index].try_yield != symbol.try_yield
            {
                unique[index].return_type = None;
                unique[index].return_kind = None;
                unique[index].try_yield = false;
            }
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
    extra: &(dyn Reader + Send),
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
    async fn records_constrained_type_parameters_as_the_bound() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub trait Reader {}
pub fn run<T: Reader>(item: T) {
    let local: T = item;
}
pub fn where_bound<T>(item: T)
where
    T: Reader,
{
}
pub struct App;
impl<T: Reader> App {
    pub fn start(&self, item: T) {}
}
pub trait BoxLike<T: Reader> {
    fn take(&self, item: T);
}
"#,
            ))
            .await
            .unwrap();

        let rows_of = |name: &str| {
            let entity = output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .unwrap_or_else(|| panic!("{name} must be extracted"));
            let bindings = entity.payload["typed_bindings"]
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
            rows
        };

        assert_eq!(
            rows_of("run"),
            vec![
                "item:Reader:impl_trait".to_string(),
                "local:Reader:impl_trait".to_string(),
            ]
        );
        assert_eq!(
            rows_of("where_bound"),
            vec!["item:Reader:impl_trait".to_string()]
        );
        assert_eq!(rows_of("start"), vec!["item:Reader:impl_trait".to_string()]);
        assert_eq!(rows_of("take"), vec!["item:Reader:impl_trait".to_string()]);
    }

    #[tokio::test]
    async fn unconstrained_or_extra_type_parameter_bounds_do_not_take_a_bound() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub trait Reader {}
pub fn free<T>(item: T) {}
pub fn extra<T: Reader + Send>(item: T) {}
pub fn generic<T: Into<String>>(item: T) {}
"#,
            ))
            .await
            .unwrap();

        for name in ["free", "extra", "generic"] {
            let entity = output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .unwrap_or_else(|| panic!("{name} must be extracted"));
            let rows = entity.payload["typed_bindings"]
                .as_array()
                .expect("typed bindings array");
            assert!(
                !rows.iter().any(|row| row["type"] == "Reader"),
                "{name} must not record the bound, got {rows:?}"
            );
        }
    }

    #[tokio::test]
    async fn records_a_bare_named_type_alias() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
pub type Client = Store;
pub type Qualified = crate::app::Store;
pub type Generic = Vec<Store>;
pub type Pair = (Store, Store);
pub type Primitive = u32;
"#,
            ))
            .await
            .unwrap();

        let alias = |name: &str| {
            output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .unwrap_or_else(|| panic!("{name} type alias must exist"))
        };
        assert_eq!(alias("Client").payload["aliased_type"], "Store");
        assert_eq!(alias("Qualified").payload["aliased_type"], "Store");
        assert!(
            alias("Generic").payload.get("aliased_type").is_none(),
            "a generic alias names nothing, got {}",
            alias("Generic").payload
        );
        assert!(
            alias("Pair").payload.get("aliased_type").is_none(),
            "a tuple alias names nothing, got {}",
            alias("Pair").payload
        );
        assert!(
            alias("Primitive").payload.get("aliased_type").is_none(),
            "a primitive alias names nothing, got {}",
            alias("Primitive").payload
        );
    }

    #[tokio::test]
    async fn records_declared_return_types() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub trait Reader {}
pub struct Store;
impl Store {
    pub fn write(&self) {}
    pub fn build(&self) -> Store { Store }
}
pub fn create_repo() -> Store { Store }
pub fn boxed() -> Box<Store> { Box::new(Store) }
pub fn borrowed() -> &'static Store { &Store }
pub fn free<T>() -> T { loop {} }
pub fn wrap<T: Reader>() -> T { loop {} }
pub fn where_bound<T>() -> T where T: Reader { loop {} }
pub fn implied() -> impl Reader { loop {} }
pub fn object() -> Box<dyn Reader> { loop {} }
pub fn extra<T: Reader + Send>() -> T { loop {} }
pub fn unit() {}
pub fn fallible() -> Result<Store, String> { Ok(Store) }
pub fn optional() -> Option<Store> { Some(Store) }
pub fn io_result() -> std::io::Result<Store> { loop {} }
"#,
            ))
            .await
            .unwrap();

        let return_of = |name: &str| -> Option<String> {
            output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .and_then(|entity| {
                    entity
                        .payload
                        .get("return_type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        };
        let kind_of = |name: &str| -> Option<String> {
            output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .and_then(|entity| {
                    entity
                        .payload
                        .get("return_kind")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        };

        assert_eq!(return_of("create_repo").as_deref(), Some("Store"));
        assert_eq!(kind_of("create_repo"), None);
        assert_eq!(return_of("boxed").as_deref(), Some("Store"));
        assert_eq!(return_of("borrowed").as_deref(), Some("Store"));
        assert_eq!(return_of("build").as_deref(), Some("Store"));
        assert_eq!(return_of("free"), None);
        assert_eq!(return_of("wrap").as_deref(), Some("Reader"));
        assert_eq!(kind_of("wrap").as_deref(), Some("impl_trait"));
        assert_eq!(return_of("where_bound").as_deref(), Some("Reader"));
        assert_eq!(kind_of("where_bound").as_deref(), Some("impl_trait"));
        assert_eq!(return_of("implied").as_deref(), Some("Reader"));
        assert_eq!(kind_of("implied").as_deref(), Some("impl_trait"));
        assert_eq!(return_of("object").as_deref(), Some("Reader"));
        assert_eq!(kind_of("object").as_deref(), Some("trait_object"));
        assert_eq!(return_of("extra"), None);
        assert_eq!(return_of("unit"), None);
        assert_eq!(return_of("write"), None);
        assert_eq!(return_of("fallible").as_deref(), Some("Store"));
        assert_eq!(return_of("optional").as_deref(), Some("Store"));
        assert_eq!(return_of("io_result").as_deref(), Some("Store"));
        let try_of = |name: &str| -> bool {
            output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .and_then(|entity| entity.payload.get("try_yield").and_then(Value::as_bool))
                .unwrap_or(false)
        };
        assert!(try_of("fallible"));
        assert!(try_of("optional"));
        assert!(try_of("io_result"));
        assert!(!try_of("create_repo"));
        assert!(!try_of("boxed"));
    }

    #[tokio::test]
    async fn records_self_return_types_on_inherent_impls() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn new() -> Self { Store }
    pub fn wrap(&self) -> Self { Store }
    pub fn boxed() -> Box<Self> { loop {} }
    pub fn borrowed() -> &Self { loop {} }
    pub fn fallible() -> Result<Self, String> { loop {} }
}
pub trait Reader {
    fn fresh() -> Self;
}
impl Reader for Store {
    fn fresh() -> Self { Store }
}
pub fn free() -> Self { loop {} }
"#,
            ))
            .await
            .unwrap();

        let return_of = |name: &str| -> Option<String> {
            output
                .entities
                .iter()
                .find(|entity| {
                    entity.name == name
                        && entity.payload.get("symbol_kind").and_then(Value::as_str)
                            == Some("method")
                })
                .and_then(|entity| {
                    entity
                        .payload
                        .get("return_type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        };
        assert_eq!(return_of("new").as_deref(), Some("Store"));
        assert_eq!(return_of("wrap").as_deref(), Some("Store"));
        assert_eq!(return_of("boxed").as_deref(), Some("Store"));
        assert_eq!(return_of("borrowed").as_deref(), Some("Store"));
        assert_eq!(return_of("fallible").as_deref(), Some("Store"));
        assert_eq!(return_of("fresh"), None);
        let fallible = output
            .entities
            .iter()
            .find(|entity| entity.name == "fallible")
            .expect("fallible");
        assert_eq!(fallible.payload["try_yield"], true);
        assert!(output.entities.iter().any(|entity| {
            entity.name == "fresh"
                && entity.payload.get("return_type").is_none()
        }));
        let free = output
            .entities
            .iter()
            .find(|entity| entity.name == "free")
            .expect("free");
        assert!(free.payload.get("return_type").is_none());
    }

    #[tokio::test]
    async fn records_named_struct_fields_as_typed_bindings() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub trait Reader {}
pub struct Store;
pub struct App<T: Reader> {
    reader: Store,
    boxed: Box<dyn Reader>,
    bound: T,
    generic: Vec<Store>,
    extra: &(dyn Reader + Send),
    plain: u32,
}
pub struct Pair(Store, Store);
pub struct Marker;
"#,
            ))
            .await
            .unwrap();

        let app = output
            .entities
            .iter()
            .find(|entity| entity.name == "App")
            .expect("App must be extracted");
        let bindings = app.payload["typed_bindings"]
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
                "reader:Store:annotation".to_string(),
            ]
        );

        for name in ["Pair", "Marker"] {
            let entity = output
                .entities
                .iter()
                .find(|entity| entity.name == name)
                .unwrap_or_else(|| panic!("{name} must be extracted"));
            assert!(
                entity.payload.get("typed_bindings").is_none(),
                "a tuple or unit struct names no field, got {}",
                entity.payload
            );
        }
    }

    #[tokio::test]
    async fn records_a_nested_self_field_receiver_without_spaces() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn write(&self) {}
}
pub struct App {
    reader: Store,
}
impl App {
    pub fn start(&mut self) {
        self.reader.write();
        (&self.reader).write();
        (&mut self.reader).write();
    }
}
"#,
            ))
            .await
            .unwrap();

        let start = output
            .entities
            .iter()
            .find(|entity| entity.name == "start")
            .expect("start must be extracted");
        let calls = start.payload["calls"].as_array().expect("calls array");
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0]["kind"], "method");
        assert_eq!(calls[0]["callee"], "self.reader.write");
        assert_eq!(calls[0]["name"], "write");
        assert_eq!(calls[0]["receiver"], "self.reader");
        for call in calls {
            let receiver = call["receiver"].as_str().unwrap_or_default();
            assert!(
                receiver.ends_with("self.reader") && !receiver.contains('('),
                "receiver must be a compacted self.reader path, got {receiver}"
            );
        }
    }

    #[tokio::test]
    async fn records_a_member_factory_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn write(&self) {}
}
pub struct App;
impl App {
    pub fn create(&self) -> Store { Store }
    pub fn start(&self, svc: App) {
        svc.create().write();
        self.create().write();
    }
}
"#,
            ))
            .await
            .unwrap();

        let start = output
            .entities
            .iter()
            .find(|entity| entity.name == "start")
            .expect("start must be extracted");
        let create = output
            .entities
            .iter()
            .find(|entity| entity.name == "create")
            .expect("create must be extracted");
        assert_eq!(create.payload["return_type"], "Store");
        let calls = start.payload["calls"].as_array().expect("calls array");
        let receivers = calls
            .iter()
            .filter(|call| call["name"] == "write")
            .map(|call| call["receiver"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            receivers.iter().any(|receiver| receiver == "svc.create()"),
            "a member factory result must keep the call, got {receivers:?}"
        );
        assert!(
            receivers.iter().any(|receiver| receiver == "self.create()"),
            "a self factory result must keep the call, got {receivers:?}"
        );
    }

    #[tokio::test]
    async fn records_a_nested_factory_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Inner;
impl Inner {
    pub fn write(&self) {}
}
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn get(&self) -> Inner { Inner }
}
pub fn create_repo() -> Store { Store }
pub struct App;
impl App {
    pub fn create(&self) -> Store { Store }
    pub fn start(&self, svc: App) {
        create_repo().get().write();
        svc.create().get().write();
        self.create().get().write();
        Store::new().get().write();
        crate::create_repo().get().write();
        crate::Store::new().get().write();
    }
}
"#,
            ))
            .await
            .unwrap();

        let start = output
            .entities
            .iter()
            .find(|entity| entity.name == "start")
            .expect("start must be extracted");
        let get = output
            .entities
            .iter()
            .find(|entity| entity.name == "get")
            .expect("get must be extracted");
        assert_eq!(get.payload["return_type"], "Inner");
        let calls = start.payload["calls"].as_array().expect("calls array");
        let receivers = calls
            .iter()
            .filter(|call| call["name"] == "write")
            .map(|call| call["receiver"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "create_repo().get()"),
            "a nested identifier factory must keep both calls, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "svc.create().get()"),
            "a nested member factory must keep both calls, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "self.create().get()"),
            "a nested self factory must keep both calls, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "Store::new().get()"),
            "a nested associated factory must keep both calls, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::create_repo().get()"),
            "a nested crate-path identifier factory must keep both calls, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::Store::new().get()"),
            "a nested crate-path associated factory must keep both calls, got {receivers:?}"
        );
    }

    #[tokio::test]
    async fn records_an_associated_factory_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn write(&self) {}
    pub fn start() {
        Store::new().write();
    }
}
"#,
            ))
            .await
            .unwrap();

        let start = output
            .entities
            .iter()
            .find(|entity| entity.name == "start")
            .expect("start must be extracted");
        let new = output
            .entities
            .iter()
            .find(|entity| entity.name == "new")
            .expect("new must be extracted");
        assert_eq!(new.payload["return_type"], "Store");
        let calls = start.payload["calls"].as_array().expect("calls array");
        let receivers = calls
            .iter()
            .filter(|call| call["name"] == "write")
            .map(|call| call["receiver"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            receivers.iter().any(|receiver| receiver == "Store::new()"),
            "an associated factory result must keep the path, got {receivers:?}"
        );
    }

    #[tokio::test]
    async fn records_a_crate_path_factory_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn write(&self) {}
}
pub fn create_repo() -> Store { Store }
pub fn run() {
    crate::create_repo().write();
    crate::Store::new().write();
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
        let receivers = calls
            .iter()
            .filter(|call| call["name"] == "write")
            .map(|call| call["receiver"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::create_repo()"),
            "a crate-path identifier factory must keep the path, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::Store::new()"),
            "a crate-path associated factory must keep the path, got {receivers:?}"
        );
    }

    #[tokio::test]
    async fn records_bindings_initialized_by_a_factory() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn write(&self) {}
}
pub struct App;
impl App {
    pub fn create(&self) -> Store { Store }
    pub fn run(&self, svc: App) {
        let from_fn = create_repo();
        let from_new = Store::new();
        let from_member = svc.create();
        let from_self = self.create();
        let annotated: Store = create_repo();
        let nested = create_repo().get();
        let nested_new = Store::new().get();
        let nested_member = svc.create().get();
        let nested_self = self.create().get();
        let nested_path = crate::create_repo().get();
        let third = create_repo().get().extra();
        let path = crate::create_repo();
        let tried = create_repo()?;
        let tried_nested = create_repo().get()?;
        let middle = create_repo()?.get();
        let middle_new = Store::new()?.get();
        let middle_member = svc.create()?.get();
        let middle_self = self.create()?.get();
        let middle_path = crate::create_repo()?.get();
        let turbo = crate::create_repo::<Store>();
        let abs = ::create_repo();
        from_fn.write();
    }
}
pub fn create_repo() -> Store { Store }
"#,
            ))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.name == "run")
            .expect("run must be extracted");
        let bindings = run.payload["call_bindings"]
            .as_array()
            .expect("call bindings array");
        let rows = bindings
            .iter()
            .map(|binding| {
                format!(
                    "{}:{}",
                    binding["name"].as_str().unwrap_or_default(),
                    binding["callee"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        assert!(
            rows.iter().any(|row| row == "from_fn:create_repo"),
            "an identifier factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "from_new:Store::new"),
            "an associated factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "from_member:svc.create"),
            "a member factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "from_self:self.create"),
            "a self factory init must be recorded, got {rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.starts_with("annotated:")),
            "an annotated let is typed_bindings, not a call binding, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "path:crate::create_repo"),
            "a crate-path factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "nested:create_repo().get"),
            "a nested identifier factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "nested_new:Store::new().get"),
            "a nested associated factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows
                .iter()
                .any(|row| row == "nested_member:svc.create().get"),
            "a nested member factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "nested_self:self.create().get"),
            "a nested self factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows
                .iter()
                .any(|row| row == "nested_path:crate::create_repo().get"),
            "a nested crate-path factory init must be recorded, got {rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.starts_with("third:")),
            "a third-hop init must not be recorded, got {rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.starts_with("turbo:") || row.starts_with("abs:")),
            "a turbofish or leading-colon init must not be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "tried:create_repo"),
            "a try factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "tried_nested:create_repo().get"),
            "a try nested factory init must be recorded, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "middle:create_repo()?.get"),
            "an inner-try nested factory init must keep the ?, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "middle_new:Store::new()?.get"),
            "an inner-try nested associated factory init must keep the ?, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "middle_member:svc.create()?.get"),
            "an inner-try nested member factory init must keep the ?, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "middle_self:self.create()?.get"),
            "an inner-try nested self factory init must keep the ?, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "middle_path:crate::create_repo()?.get"),
            "an inner-try nested crate-path factory init must keep the ?, got {rows:?}"
        );
        let middle = bindings
            .iter()
            .find(|binding| binding["name"] == "middle")
            .expect("middle binding");
        assert!(middle.get("try").is_none());
        let tried = bindings
            .iter()
            .find(|binding| binding["name"] == "tried")
            .expect("tried binding");
        assert_eq!(tried["try"], true);
        let from_fn = bindings
            .iter()
            .find(|binding| binding["name"] == "from_fn")
            .expect("from_fn binding");
        assert!(from_fn.get("try").is_none());
        assert_eq!(run.payload["omitted_call_bindings"], 0);
    }

    #[tokio::test]
    async fn records_a_try_factory_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Inner;
impl Inner {
    pub fn write(&self) {}
}
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn get(&self) -> Inner { Inner }
    pub fn write(&self) {}
}
pub struct App;
impl App {
    pub fn create(&self) -> Store { Store }
}
pub fn create_repo() -> Store { Store }
pub fn run(svc: App) {
    create_repo()?.write();
    create_repo().get()?.write();
    create_repo()?.get().write();
    svc.create()?.get().write();
    Store::new()?.get().write();
    crate::create_repo()?.get().write();
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
        let receivers = calls
            .iter()
            .filter(|call| call["name"] == "write")
            .map(|call| call["receiver"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            receivers.iter().any(|receiver| receiver == "create_repo()?"),
            "a try factory result must keep the ?, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "create_repo().get()?"),
            "a try nested factory result must keep the ?, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "create_repo()?.get()"),
            "an inner-try nested factory must keep the inner ?, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "svc.create()?.get()"),
            "an inner-try nested member factory must keep the inner ?, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "Store::new()?.get()"),
            "an inner-try nested associated factory must keep the inner ?, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::create_repo()?.get()"),
            "an inner-try nested crate-path factory must keep the inner ?, got {receivers:?}"
        );
    }

    #[tokio::test]
    async fn records_an_unwrap_factory_receiver() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Inner;
impl Inner {
    pub fn write(&self) {}
}
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn get(&self) -> Inner { Inner }
    pub fn write(&self) {}
}
pub struct App;
impl App {
    pub fn create(&self) -> Store { Store }
}
pub fn create_repo() -> Store { Store }
pub fn run(svc: App) {
    create_repo().unwrap().write();
    create_repo().expect("failed").write();
    create_repo().get().unwrap().write();
    svc.create().unwrap().write();
    Store::new().unwrap().write();
    crate::create_repo().unwrap().write();
    create_repo().unwrap().get().write();
    svc.create().unwrap().get().write();
    Store::new().unwrap().get().write();
    crate::create_repo().unwrap().get().write();
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
        let receivers = calls
            .iter()
            .filter(|call| call["name"] == "write")
            .map(|call| call["receiver"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "create_repo().unwrap()"),
            "an unwrap factory result must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers.iter().any(|receiver| {
                receiver.starts_with("create_repo().expect") && receiver.ends_with(')')
            }),
            "an expect factory result must keep expect, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "create_repo().get().unwrap()"),
            "an unwrap nested factory result must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "svc.create().unwrap()"),
            "an unwrap member factory result must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "Store::new().unwrap()"),
            "an unwrap associated factory result must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::create_repo().unwrap()"),
            "an unwrap crate-path factory result must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "create_repo().unwrap().get()"),
            "an inner-unwrap nested factory must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "svc.create().unwrap().get()"),
            "an inner-unwrap nested member factory must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "Store::new().unwrap().get()"),
            "an inner-unwrap nested associated factory must keep unwrap, got {receivers:?}"
        );
        assert!(
            receivers
                .iter()
                .any(|receiver| receiver == "crate::create_repo().unwrap().get()"),
            "an inner-unwrap nested crate-path factory must keep unwrap, got {receivers:?}"
        );
    }

    #[tokio::test]
    async fn records_bindings_initialized_by_an_unwrap_factory() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
impl Store {
    pub fn new() -> Store { Store }
    pub fn get(&self) -> Store { Store }
    pub fn write(&self) {}
}
pub struct App;
impl App {
    pub fn create(&self) -> Store { Store }
    pub fn run(&self, svc: App) {
        let unwrapped = create_repo().unwrap();
        let expected = create_repo().expect("failed");
        let nested = create_repo().get().unwrap();
        let member = svc.create().unwrap();
        let inner = create_repo().unwrap().get();
        let inner_new = Store::new().unwrap().get();
        let inner_member = svc.create().unwrap().get();
        let inner_path = crate::create_repo().unwrap().get();
        let inner_expect = create_repo().expect("failed").get();
        let or_default = create_repo().unwrap_or_default();
        unwrapped.write();
    }
}
pub fn create_repo() -> Store { Store }
"#,
            ))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.name == "run")
            .expect("run must be extracted");
        let bindings = run.payload["call_bindings"]
            .as_array()
            .expect("call bindings array");
        let rows = bindings
            .iter()
            .map(|binding| {
                format!(
                    "{}:{}",
                    binding["name"].as_str().unwrap_or_default(),
                    binding["callee"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        assert!(
            rows.iter().any(|row| row == "unwrapped:create_repo"),
            "an unwrap factory init must peel unwrap, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "expected:create_repo"),
            "an expect factory init must peel expect, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "nested:create_repo().get"),
            "an unwrap nested factory init must peel unwrap, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "member:svc.create"),
            "an unwrap member factory init must peel unwrap, got {rows:?}"
        );
        assert!(
            rows.iter().any(|row| row == "inner:create_repo()?.get"),
            "an inner unwrap hop must record as try, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "inner_new:Store::new()?.get"),
            "an inner unwrap associated hop must record as try, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "inner_member:svc.create()?.get"),
            "an inner unwrap member hop must record as try, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "inner_path:crate::create_repo()?.get"),
            "an inner unwrap crate-path hop must record as try, got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|row| row == "inner_expect:create_repo()?.get"),
            "an inner expect hop must record as try, got {rows:?}"
        );
        let inner = bindings
            .iter()
            .find(|binding| binding["name"] == "inner")
            .expect("inner binding");
        assert!(inner.get("try").is_none());
        assert!(
            !rows.iter().any(|row| row.starts_with("or_default:")),
            "unwrap_or_default must not be recorded as try, got {rows:?}"
        );
        for name in ["unwrapped", "expected", "nested", "member"] {
            let binding = bindings
                .iter()
                .find(|binding| binding["name"] == name)
                .unwrap_or_else(|| panic!("{name} binding"));
            assert_eq!(binding["try"], true, "{name} must be a try init");
        }
        assert_eq!(run.payload["omitted_call_bindings"], 0);
    }

    #[tokio::test]
    async fn coalesces_cfg_struct_fields() {
        let output = RustExtractor
            .extract(input(
                "src/lib.rs",
                r#"
pub struct Store;
pub struct Other;
#[cfg(unix)]
pub struct App {
    reader: Store,
}
#[cfg(windows)]
pub struct App {
    reader: Other,
}
"#,
            ))
            .await
            .unwrap();

        let app = output
            .entities
            .iter()
            .find(|entity| entity.name == "App")
            .expect("App must be extracted");
        let bindings = app.payload["typed_bindings"]
            .as_array()
            .expect("typed bindings array");
        let mut types = bindings
            .iter()
            .map(|binding| binding["type"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        types.sort();
        assert_eq!(types, vec!["Other".to_string(), "Store".to_string()]);
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

    #[tokio::test]
    async fn bounds_the_receiver_rows_a_declaration_records() {
        let mut source = String::from("pub struct Store;\npub fn run() {\n");
        for index in 0..70 {
            source.push_str(&format!("    let binding{index}: Store = Store;\n"));
        }
        source.push_str("}\n");

        let output = RustExtractor
            .extract(input("src/lib.rs", &source))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.name == "run")
            .expect("run must be extracted");
        assert_eq!(run.payload["typed_bindings"].as_array().unwrap().len(), 64);
        assert_eq!(run.payload["omitted_typed_bindings"], 6);
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
