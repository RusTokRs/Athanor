use std::borrow::Cow;
#[cfg(feature = "precision-parser")]
use std::collections::BTreeMap;

use async_trait::async_trait;
use athanor_core::{
    CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind, Fact,
    FactId, FactKind, LanguageCode, Severity, SnapshotId, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::{Value, json};
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Default)]
pub struct JsTsExtractor;

#[async_trait]
impl Extractor for JsTsExtractor {
    fn name(&self) -> &'static str {
        "js-ts"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
    }

    fn supports(&self, source: &SourceFile) -> bool {
        js_ts_language(source).is_some() || is_package_json(source) || is_tsconfig_json(source)
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };

        if is_package_json(&input.source) {
            return Ok(extract_package_json(self.name(), &input, content));
        }

        if is_tsconfig_json(&input.source) {
            return Ok(extract_typescript_config(self.name(), &input, content));
        }

        let Some(language) = js_ts_language(&input.source) else {
            return Ok(ExtractOutput::default());
        };

        let mut parser = Parser::new();
        match language.parser {
            ParserLanguage::Javascript => {
                parser
                    .set_language(&tree_sitter_javascript::LANGUAGE.into())
                    .map_err(|error| athanor_core::CoreError::Adapter(error.to_string()))?;
            }
            ParserLanguage::Typescript => {
                parser
                    .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                    .map_err(|error| athanor_core::CoreError::Adapter(error.to_string()))?;
            }
            ParserLanguage::Tsx => {
                parser
                    .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                    .map_err(|error| athanor_core::CoreError::Adapter(error.to_string()))?;
            }
        }

        let parse_content = normalized_parse_content(content);
        let parse_bytes = parse_content.as_bytes();
        let Some(tree) = parser.parse(parse_bytes, None) else {
            return Ok(ExtractOutput::default());
        };

        let root = tree.root_node();
        let mut declarations = Vec::new();
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut diagnostics = Vec::new();

        collect_source_items(
            root,
            parse_bytes,
            &mut declarations,
            &mut imports,
            &mut exports,
        );

        if root.has_error() {
            collect_parse_errors(
                self.name(),
                &input.source.path,
                &input.snapshot,
                root,
                &mut diagnostics,
            );
        }

        collect_unsupported_top_level(
            self.name(),
            &input.source.path,
            &input.snapshot,
            root,
            parse_bytes,
            &mut diagnostics,
        );

        #[cfg(feature = "precision-parser")]
        let precision = verify_with_oxc(
            self.name(),
            &input.source.path,
            &input.snapshot,
            PrecisionVerificationInput {
                content: parse_content.as_ref(),
                declarations: &declarations,
                imports: &imports,
                exports: &exports,
                primary_recoveries: diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.kind == DiagnosticKind::Other("js_ts_parse_error".to_string())
                    })
                    .count(),
            },
            &mut diagnostics,
        );

        let mut entities = Vec::new();
        let mut facts = Vec::new();
        let context = ExtractionEvidenceContext {
            extractor: self.name(),
            path: &input.source.path,
            snapshot: &input.snapshot,
        };
        let ownership = ownership_for_file(&input.source.path);
        let module_stable_key = StableKey(format!("module://js-ts:{}", input.source.path));
        let module_id = EntityId(format!(
            "ent_js_ts_module_{:016x}",
            stable_hash(module_stable_key.0.as_bytes())
        ));
        #[allow(unused_mut)]
        let mut module_payload = json!({
            "language_hint": input.source.language_hint,
            "parser": language.parser_name,
            "imports": imports,
            "exports": exports,
            "declaration_count": declarations.len(),
        });
        #[cfg(feature = "precision-parser")]
        {
            module_payload["precision_verification"] = precision;
        }

        entities.push(Entity {
            id: module_id.clone(),
            stable_key: module_stable_key.clone(),
            kind: EntityKind::Module,
            name: module_name(&input.source.path),
            title: None,
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(1),
                line_end: Some(root.end_position().row as u32 + 1),
            }),
            language: Some(LanguageCode(language.hint.to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: module_payload,
        });

        facts.push(symbol_fact(
            context,
            SymbolFactSpec {
                subject: &module_id,
                object: None,
                stable_key: &module_stable_key,
                symbol_kind: "module",
                range: line_range(root),
            },
        ));

        for declaration in declarations {
            let stable_key = StableKey(format!(
                "symbol://js-ts:{}#{}",
                input.source.path, declaration.qualified_name
            ));
            let entity_id = EntityId(format!(
                "{}{:016x}",
                declaration.id_prefix(),
                stable_hash(stable_key.0.as_bytes())
            ));
            let (line_start, line_end) = declaration.line_range;

            let mut payload = json!({
                "symbol_kind": declaration.symbol_kind,
                "qualified_name": declaration.qualified_name,
                "exported": declaration.exported,
                "async": declaration.is_async,
                "source_node": declaration.source_node,
                "calls": declaration.calls,
            });
            if declaration.qualified_name == ANONYMOUS_DEFAULT_NAME {
                payload["anonymous_default"] = json!(true);
            }
            if let Some(heritage) = &declaration.heritage {
                payload["extends"] = json!(heritage.extends);
                payload["implements"] = json!(heritage.implements);
            }

            entities.push(Entity {
                id: entity_id.clone(),
                stable_key: stable_key.clone(),
                kind: declaration.entity_kind,
                name: declaration.name.clone(),
                title: None,
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(line_start),
                    line_end: Some(line_end),
                }),
                language: Some(LanguageCode(language.hint.to_string())),
                aliases: Vec::new(),
                ownership: ownership.clone(),
                payload,
            });

            facts.push(symbol_fact(
                context,
                SymbolFactSpec {
                    subject: &entity_id,
                    object: Some(&module_id),
                    stable_key: &stable_key,
                    symbol_kind: declaration.symbol_kind,
                    range: declaration.line_range,
                },
            ));
        }

        entities.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        facts.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));

        Ok(ExtractOutput {
            entities,
            facts,
            diagnostics,
        })
    }
}

fn normalized_parse_content(content: &str) -> Cow<'_, str> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    if let Some(rest) = content.strip_prefix("#!") {
        Cow::Owned(format!("//{rest}"))
    } else {
        Cow::Borrowed(content)
    }
}

#[derive(Debug, Clone, Copy)]
enum ParserLanguage {
    Javascript,
    Typescript,
    Tsx,
}

#[derive(Debug, Clone, Copy)]
struct JsTsLanguage {
    hint: &'static str,
    parser: ParserLanguage,
    parser_name: &'static str,
}

fn js_ts_language(source: &SourceFile) -> Option<JsTsLanguage> {
    match source.language_hint.as_deref() {
        Some("javascript") => Some(JsTsLanguage {
            hint: "javascript",
            parser: ParserLanguage::Javascript,
            parser_name: "tree-sitter-javascript",
        }),
        Some("javascriptreact") => Some(JsTsLanguage {
            hint: "javascriptreact",
            parser: ParserLanguage::Javascript,
            parser_name: "tree-sitter-javascript",
        }),
        Some("typescript") => Some(JsTsLanguage {
            hint: "typescript",
            parser: ParserLanguage::Typescript,
            parser_name: "tree-sitter-typescript",
        }),
        Some("typescriptreact") => Some(JsTsLanguage {
            hint: "typescriptreact",
            parser: ParserLanguage::Tsx,
            parser_name: "tree-sitter-tsx",
        }),
        _ => None,
    }
}

fn is_package_json(source: &SourceFile) -> bool {
    source.path.ends_with("package.json")
}

/// Recognizes the first-party TypeScript and JavaScript project configuration
/// files that carry module-resolution options: `tsconfig.json`, `jsconfig.json`,
/// and their named `.json` variants such as `tsconfig.base.json`.
fn is_tsconfig_json(source: &SourceFile) -> bool {
    let normalized = source.path.replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    is_tsconfig_file_name(file_name)
}

fn is_tsconfig_file_name(file_name: &str) -> bool {
    let Some(stem) = file_name.strip_suffix(".json") else {
        return false;
    };
    stem == "tsconfig"
        || stem == "jsconfig"
        || stem.starts_with("tsconfig.")
        || stem.starts_with("jsconfig.")
}

/// Upper bound on recorded call sites per declaration.
///
/// Payloads stay bounded and deterministic: call sites are recorded in source
/// order and anything beyond this limit is omitted.
const MAX_CALL_SITES_PER_DECLARATION: usize = 256;

/// Callee names known to invoke a function-valued argument.
///
/// The allowlist keeps recorded callback evidence bounded to combinators
/// whose argument position is a real indirect call: array iteration, promise
/// handlers, timers, and event/observable registration. Only these names
/// carry `callback_arguments`, so a function passed to an ordinary call
/// never becomes a guessed call edge.
const CALLBACK_CALLEES: &[&str] = &[
    "map",
    "filter",
    "forEach",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "some",
    "every",
    "reduce",
    "reduceRight",
    "flatMap",
    "sort",
    "then",
    "catch",
    "finally",
    "setTimeout",
    "setInterval",
    "addEventListener",
    "addListener",
    "on",
    "once",
    "subscribe",
];

/// Upper bound on recorded callback arguments per call site.
///
/// Combinators take at most two callbacks in practice; the bound keeps the
/// payload deterministic even for unusually long argument lists.
const MAX_CALLBACK_ARGUMENTS: usize = 4;

/// Upper bound on recorded TypeScript path mappings per configuration file.
///
/// Path aliases are configuration evidence, not source structure: the bound
/// keeps one configuration file from dominating the snapshot while the
/// omitted count stays recorded.
const MAX_PATH_MAPPINGS: usize = 64;

/// Upper bound on recorded substitution targets per path mapping.
const MAX_PATH_TARGETS: usize = 8;

/// Upper bound on recorded workspace glob patterns per `package.json`.
const MAX_WORKSPACE_PATTERNS: usize = 64;

/// Recorded name of an anonymous `export default` declaration.
///
/// `export default function () {}` declares a callable value that other
/// modules import, but the source gives it no name. It is recorded as
/// `default`, the one name a JavaScript or TypeScript binding cannot have,
/// so the synthesized name can never collide with a declared binding and
/// every consumer can see that the name is not evidence.
const ANONYMOUS_DEFAULT_NAME: &str = "default";

/// Node kinds that an `export default` clause may export anonymously.
const ANONYMOUS_DEFAULT_KINDS: [&str; 7] = [
    "function_declaration",
    "generator_function_declaration",
    "arrow_function",
    "function",
    "function_expression",
    "class_declaration",
    "class",
];

#[derive(Debug, Clone)]
struct SourceDeclaration {
    name: String,
    qualified_name: String,
    entity_kind: EntityKind,
    symbol_kind: &'static str,
    exported: bool,
    is_async: bool,
    source_node: String,
    line_range: (u32, u32),
    calls: Vec<Value>,
    heritage: Option<TypeHeritage>,
}

/// `extends`/`implements` heritage of a class or interface, reduced to bare type names.
#[derive(Debug, Clone, Default)]
struct TypeHeritage {
    extends: Vec<String>,
    implements: Vec<String>,
}

impl SourceDeclaration {
    fn id_prefix(&self) -> &'static str {
        match self.entity_kind {
            EntityKind::Function => "ent_js_ts_function_",
            EntityKind::Class => "ent_js_ts_class_",
            _ => "ent_js_ts_symbol_",
        }
    }
}

fn collect_source_items(
    root: Node,
    bytes: &[u8],
    declarations: &mut Vec<SourceDeclaration>,
    imports: &mut Vec<Value>,
    exports: &mut Vec<Value>,
) {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        collect_statement(child, bytes, false, declarations, imports, exports);
    }
}

fn collect_statement(
    node: Node,
    bytes: &[u8],
    exported: bool,
    declarations: &mut Vec<SourceDeclaration>,
    imports: &mut Vec<Value>,
    exports: &mut Vec<Value>,
) {
    match node.kind() {
        "import_statement" => imports.push(import_payload(node, bytes)),
        "export_statement" => {
            exports.push(export_payload(node, bytes));
            collect_anonymous_default(node, bytes, declarations);
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_statement(child, bytes, true, declarations, imports, exports);
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name) = node_name(node, bytes) {
                declarations.push(SourceDeclaration {
                    name: name.clone(),
                    qualified_name: name,
                    entity_kind: EntityKind::Function,
                    symbol_kind: "function",
                    exported,
                    is_async: has_child_kind(node, "async"),
                    source_node: node.kind().to_string(),
                    line_range: line_range(node),
                    calls: collect_call_sites(node, bytes),
                    heritage: None,
                });
            }
        }
        "method_definition" | "method_signature" | "public_field_definition" => {
            if let Some(name) = node_name(node, bytes) {
                declarations.push(SourceDeclaration {
                    name: name.clone(),
                    qualified_name: name,
                    entity_kind: EntityKind::Function,
                    symbol_kind: "method",
                    exported,
                    is_async: has_child_kind(node, "async"),
                    source_node: node.kind().to_string(),
                    line_range: line_range(node),
                    calls: collect_call_sites(node, bytes),
                    heritage: None,
                });
            }
        }
        "class_declaration" => {
            if let Some(name) = node_name(node, bytes) {
                declarations.push(SourceDeclaration {
                    name: name.clone(),
                    qualified_name: name.clone(),
                    entity_kind: EntityKind::Class,
                    symbol_kind: "class",
                    exported,
                    is_async: false,
                    source_node: node.kind().to_string(),
                    line_range: line_range(node),
                    calls: Vec::new(),
                    heritage: class_heritage(node, bytes),
                });
                collect_class_members(&name, node, bytes, exported, declarations);
            }
        }
        "interface_declaration" => {
            if let Some(name) = node_name(node, bytes) {
                declarations.push(SourceDeclaration {
                    name: name.clone(),
                    qualified_name: name,
                    entity_kind: EntityKind::Symbol,
                    symbol_kind: "interface",
                    exported,
                    is_async: false,
                    source_node: node.kind().to_string(),
                    line_range: line_range(node),
                    calls: Vec::new(),
                    heritage: interface_heritage(node, bytes),
                });
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = node_name(node, bytes) {
                declarations.push(SourceDeclaration {
                    name: name.clone(),
                    qualified_name: name,
                    entity_kind: EntityKind::Symbol,
                    symbol_kind: "type_alias",
                    exported,
                    is_async: false,
                    source_node: node.kind().to_string(),
                    line_range: line_range(node),
                    calls: Vec::new(),
                    heritage: None,
                });
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            collect_variable_functions(node, bytes, exported, declarations);
        }
        _ => {}
    }
}

/// Records an anonymous `export default` declaration under a canonical name.
///
/// `export default function () {}`, `export default class {}`, and
/// `export default () => {}` export a real callable or constructible value that
/// other modules import, but the source gives it no name. Such a declaration is
/// recorded as `default`, so a default import resolves to it instead of staying
/// unresolved. Default expressions that declare nothing callable or
/// constructible — an object literal, a call result, or a primitive — are not
/// declarations and stay unrecorded. At most one anonymous default is recorded
/// per file, because a module has at most one default export.
fn collect_anonymous_default(node: Node, bytes: &[u8], declarations: &mut Vec<SourceDeclaration>) {
    if !is_default_export(node) {
        return;
    }
    if declarations
        .iter()
        .any(|declaration| declaration.qualified_name == ANONYMOUS_DEFAULT_NAME)
    {
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !ANONYMOUS_DEFAULT_KINDS.contains(&child.kind()) || node_name(child, bytes).is_some() {
            continue;
        }
        let entity_kind = if matches!(child.kind(), "class_declaration" | "class") {
            EntityKind::Class
        } else {
            EntityKind::Function
        };
        declarations.push(SourceDeclaration {
            name: ANONYMOUS_DEFAULT_NAME.to_string(),
            qualified_name: ANONYMOUS_DEFAULT_NAME.to_string(),
            entity_kind,
            symbol_kind: if entity_kind == EntityKind::Class {
                "class"
            } else {
                "function"
            },
            exported: true,
            is_async: has_child_kind(child, "async"),
            source_node: child.kind().to_string(),
            line_range: line_range(child),
            calls: collect_call_sites(child, bytes),
            heritage: if entity_kind == EntityKind::Class {
                class_heritage(child, bytes)
            } else {
                None
            },
        });
        if entity_kind == EntityKind::Class {
            collect_class_members(ANONYMOUS_DEFAULT_NAME, child, bytes, true, declarations);
        }
        return;
    }
}

fn collect_class_members(
    class_name: &str,
    class_node: Node,
    bytes: &[u8],
    exported: bool,
    declarations: &mut Vec<SourceDeclaration>,
) {
    let mut cursor = class_node.walk();
    for child in class_node.named_children(&mut cursor) {
        if child.kind() != "class_body" {
            continue;
        }
        let mut body_cursor = child.walk();
        for member in child.named_children(&mut body_cursor) {
            if matches!(member.kind(), "method_definition" | "method_signature")
                && let Some(name) = node_name(member, bytes)
            {
                declarations.push(SourceDeclaration {
                    name: name.clone(),
                    qualified_name: format!("{class_name}.{name}"),
                    entity_kind: EntityKind::Function,
                    symbol_kind: "method",
                    exported,
                    is_async: has_child_kind(member, "async"),
                    source_node: member.kind().to_string(),
                    line_range: line_range(member),
                    calls: collect_call_sites(member, bytes),
                    heritage: None,
                });
            }
        }
    }
}

fn collect_variable_functions(
    node: Node,
    bytes: &[u8],
    exported: bool,
    declarations: &mut Vec<SourceDeclaration>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }
        let Some(name_node) = child.child_by_field_name("name") else {
            continue;
        };
        let Some(value_node) = child.child_by_field_name("value") else {
            continue;
        };
        if matches!(
            value_node.kind(),
            "arrow_function" | "function" | "function_expression"
        ) && let Ok(name) = name_node.utf8_text(bytes)
        {
            declarations.push(SourceDeclaration {
                name: name.to_string(),
                qualified_name: name.to_string(),
                entity_kind: EntityKind::Function,
                symbol_kind: "function",
                exported,
                is_async: has_child_kind(value_node, "async"),
                source_node: value_node.kind().to_string(),
                line_range: line_range(child),
                calls: collect_call_sites(value_node, bytes),
                heritage: None,
            });
        }
    }
}

/// Read the `extends`/`implements` clauses of a class declaration.
///
/// Both clauses are matched by node kind (`class_heritage` holds them in the JS and TS grammars)
/// and reduced to bare type names, so generic arguments and qualified paths do not leak into the
/// payload: `class Service extends api.Base implements Store<K> {}` records `Base` and `Store`.
fn class_heritage(node: Node, bytes: &[u8]) -> Option<TypeHeritage> {
    let mut heritage = TypeHeritage::default();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "class_heritage" {
            continue;
        }
        let mut clause_cursor = child.walk();
        for clause in child.named_children(&mut clause_cursor) {
            match clause.kind() {
                "extends_clause" => {
                    let value = clause
                        .child_by_field_name("value")
                        .or_else(|| clause.named_child(0));
                    if let Some(name) = value.and_then(|value| heritage_name(value, bytes)) {
                        push_heritage_name(&mut heritage.extends, name);
                    }
                }
                "implements_clause" => {
                    let mut list_cursor = clause.walk();
                    for item in clause.named_children(&mut list_cursor) {
                        let Some(name) = heritage_name(item, bytes) else {
                            continue;
                        };
                        push_heritage_name(&mut heritage.implements, name);
                    }
                }
                _ => {}
            }
        }
    }
    if heritage.extends.is_empty() && heritage.implements.is_empty() {
        return None;
    }
    Some(heritage)
}

/// Read the multiple `extends` entries allowed by a TypeScript interface declaration.
fn interface_heritage(node: Node, bytes: &[u8]) -> Option<TypeHeritage> {
    let mut heritage = TypeHeritage::default();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "extends_type_clause" {
            continue;
        }
        collect_interface_heritage_names(child, bytes, &mut heritage.extends);
    }
    if heritage.extends.is_empty() {
        None
    } else {
        Some(heritage)
    }
}

fn collect_interface_heritage_names(node: Node, bytes: &[u8], names: &mut Vec<String>) {
    if is_type_reference_node(node.kind())
        && let Some(name) = heritage_name(node, bytes)
    {
        push_heritage_name(names, name);
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_interface_heritage_names(child, bytes, names);
    }
}

fn is_type_reference_node(kind: &str) -> bool {
    matches!(
        kind,
        "type_identifier" | "nested_type_identifier" | "generic_type" | "identifier"
    )
}

fn push_heritage_name(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

/// Bare type name of one heritage entry: generic arguments dropped, last path segment kept.
fn heritage_name(node: Node, bytes: &[u8]) -> Option<String> {
    let text = node_text(node, bytes)?;
    let base = text.split('<').next()?.trim();
    let name = base.rsplit(['.', ':']).next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn node_name(node: Node, bytes: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|child| child.utf8_text(bytes).ok())
        .map(ToString::to_string)
}

fn has_child_kind(node: Node, kind: &str) -> bool {
    (0..node.child_count()).any(|index| {
        node.child(index as u32)
            .is_some_and(|child| child.kind() == kind)
    })
}

fn import_payload(node: Node, bytes: &[u8]) -> Value {
    let (default_name, namespace_name, named, named_bindings) = import_bindings(node, bytes);
    json!({
        "source": child_text(node, "source", bytes),
        "line_start": node.start_position().row as u32 + 1,
        "line_end": node.end_position().row as u32 + 1,
        "default_name": default_name,
        "namespace_name": namespace_name,
        "named": named,
        "named_bindings": named_bindings,
    })
}

/// Collects the local and imported names bound by one `import_statement`.
///
/// `named` retains the sorted local-name list for compatibility. `named_bindings`
/// additionally preserves the source name for aliases such as
/// `import { fetchUser as fetch }`, allowing linkers to resolve `fetch` back to
/// the exported name `fetchUser` without guessing.
fn import_bindings(
    node: Node,
    bytes: &[u8],
) -> (Option<String>, Option<String>, Vec<String>, Vec<Value>) {
    let mut default_name = None;
    let mut namespace_name = None;
    let mut named = Vec::new();
    let mut named_bindings = Vec::new();

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "import_clause" {
            continue;
        }
        let mut clause_cursor = child.walk();
        for item in child.named_children(&mut clause_cursor) {
            match item.kind() {
                "identifier" => default_name = node_text(item, bytes),
                "namespace_import" => {
                    let mut namespace_cursor = item.walk();
                    for candidate in item.named_children(&mut namespace_cursor) {
                        if candidate.kind() == "identifier" {
                            namespace_name = node_text(candidate, bytes);
                        }
                    }
                }
                "named_imports" => {
                    let mut named_cursor = item.walk();
                    for specifier in item.named_children(&mut named_cursor) {
                        if specifier.kind() != "import_specifier" {
                            continue;
                        }
                        let imported = specifier
                            .child_by_field_name("name")
                            .and_then(|child| node_text(child, bytes));
                        let local = specifier
                            .child_by_field_name("alias")
                            .or_else(|| specifier.child_by_field_name("name"))
                            .and_then(|child| node_text(child, bytes));
                        if let Some(local) = local {
                            named.push(local.clone());
                            if let Some(imported) = imported {
                                named_bindings.push((imported, local));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    named.sort();
    named.dedup();
    named_bindings.sort();
    named_bindings.dedup();
    let named_bindings: Vec<Value> = named_bindings
        .into_iter()
        .map(|(imported, local)| json!({ "imported": imported, "local": local }))
        .collect();
    (default_name, namespace_name, named, named_bindings)
}

fn node_text(node: Node, bytes: &[u8]) -> Option<String> {
    node.utf8_text(bytes).ok().map(ToString::to_string)
}

/// Records the static call sites of one declaration subtree.
///
/// Only statically named callees are recorded: identifiers (`login()`),
/// member chains rooted in an identifier or `this` (`api.send()`,
/// `this.refresh()`), `super` calls, and constructor invocations
/// (`new Session()`). Computed calls (`handlers[key]()`), calls through
/// parenthesized or call results (`factory()()`), and tags/JSX are omitted
/// rather than guessed. Call sites are attributed to the declaration that
/// contains them; nested class bodies are skipped because their members are
/// separate declarations.
///
/// A call whose callee name is a known callback combinator additionally
/// records its statically nameable arguments in `callback_arguments`, so
/// the call linker can bridge the indirect invocation to a declaration.
fn collect_call_sites(node: Node, bytes: &[u8]) -> Vec<Value> {
    let mut calls = Vec::new();
    let mut seen = std::collections::HashSet::new();
    collect_call_sites_inner(node, bytes, &mut calls, &mut seen);
    calls
}

fn collect_call_sites_inner(
    node: Node,
    bytes: &[u8],
    calls: &mut Vec<Value>,
    seen: &mut std::collections::HashSet<(String, u32, u32)>,
) {
    if calls.len() >= MAX_CALL_SITES_PER_DECLARATION {
        return;
    }
    if matches!(node.kind(), "class_declaration" | "class") {
        return;
    }
    if let Some(payload) = call_site_payload(node, bytes)
        && let Some(callee) = payload.get("callee").and_then(Value::as_str)
    {
        let key = (
            callee.to_string(),
            payload.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
            payload.get("column").and_then(Value::as_u64).unwrap_or(0) as u32,
        );
        if seen.insert(key) {
            calls.push(payload);
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_sites_inner(child, bytes, calls, seen);
    }
}

fn call_site_payload(node: Node, bytes: &[u8]) -> Option<Value> {
    let (call_kind, callee_node) = match node.kind() {
        "call_expression" => ("call", node.child_by_field_name("function")?),
        "new_expression" => ("new", node.child_by_field_name("constructor")?),
        _ => return None,
    };
    let callee = callee_path(callee_node, bytes)?;
    let (name, receiver) = match callee.rsplit_once('.') {
        Some((receiver, name)) => (name.to_string(), Some(receiver.to_string())),
        None => (callee.clone(), None),
    };
    let start = callee_node.start_position();
    let arguments = node.child_by_field_name("arguments");
    let arg_count = arguments
        .map_or(0, |arguments| arguments.named_child_count());

    let mut payload = json!({
        "callee": callee,
        "name": name,
        "receiver": receiver,
        "kind": call_kind,
        "line": start.row as u32 + 1,
        "column": start.column as u32 + 1,
        "arg_count": arg_count,
    });
    if CALLBACK_CALLEES.contains(&name.as_str())
        && let Some(callbacks) = callback_arguments(arguments, bytes)
    {
        payload["callback_arguments"] = json!(callbacks);
    }
    Some(payload)
}

/// Records the statically nameable arguments of a callback-taking call.
///
/// Only plain identifiers and identifier-rooted member chains
/// (`this.handler`, `api.helpers.log`) are recorded, because those are
/// the argument shapes the call linker can resolve to a declaration.
/// Inline arrows, call results, literals, and computed expressions are
/// skipped rather than guessed at.
fn callback_arguments(arguments: Option<Node>, bytes: &[u8]) -> Option<Vec<String>> {
    let arguments = arguments?;
    let mut cursor = arguments.walk();
    let names = arguments
        .named_children(&mut cursor)
        .filter_map(|argument| callee_path(argument, bytes))
        .take(MAX_CALLBACK_ARGUMENTS)
        .collect::<Vec<_>>();
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

/// Normalizes a statically named callee into a dotted path.
///
/// Returns `None` for call shapes whose target cannot be named statically.
fn callee_path(node: Node, bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier"
        | "property_identifier"
        | "private_property_identifier"
        | "type_identifier"
        | "this"
        | "super" => node_text(node, bytes),
        "member_expression" => {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;
            let object_path = callee_path(object, bytes)?;
            let property_path = callee_path(property, bytes)?;
            Some(format!("{object_path}.{property_path}"))
        }
        _ => None,
    }
}

fn export_payload(node: Node, bytes: &[u8]) -> Value {
    json!({
        "source": child_text(node, "source", bytes),
        "bindings": export_bindings(node, bytes),
        "star": is_star_reexport(node),
        "namespace_name": namespace_export_name(node, bytes),
        "default_export": is_default_export(node),
        "default_name": default_export_name(node, bytes),
        "line_start": node.start_position().row as u32 + 1,
        "line_end": node.end_position().row as u32 + 1,
    })
}

/// Returns the exported namespace alias for `export * as name from "…"`.
fn namespace_export_name(node: Node, bytes: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let namespace_export = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "namespace_export")?;

    for index in 0..namespace_export.child_count() {
        let Some(child) = namespace_export.child(index as u32) else {
            continue;
        };
        match child.kind() {
            "identifier" => return node_text(child, bytes),
            "string" => {
                return node_text(child, bytes)
                    .map(|name| name.trim_matches(['"', '\'']).to_string());
            }
            "default" => return Some("default".to_string()),
            _ => {}
        }
    }
    None
}

/// Identifies `export default` without confusing `default` names inside an
/// explicit export clause.
fn is_default_export(node: Node) -> bool {
    has_child_kind(node, "default")
}

/// Returns the local binding for a default function, class, or identifier
/// export. An anonymous default function or class is bound to the canonical
/// `default` name; other default expressions stay explicitly unbound and
/// cannot be guessed.
fn default_export_name(node: Node, bytes: &[u8]) -> Option<String> {
    if !is_default_export(node) {
        return None;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "function_declaration"
            | "generator_function_declaration"
            | "arrow_function"
            | "function"
            | "function_expression"
            | "class_declaration"
            | "class" => {
                return Some(
                    node_name(child, bytes).unwrap_or_else(|| ANONYMOUS_DEFAULT_NAME.to_string()),
                );
            }
            "identifier" => return node_text(child, bytes),
            _ => {}
        }
    }
    None
}

/// Identifies only `export * from "…"`; `export * as name from "…"` is a
/// namespace export and must not be treated as forwarding named exports.
fn is_star_reexport(node: Node) -> bool {
    node.child_by_field_name("source").is_some()
        && (0..node.child_count()).any(|index| {
            node.child(index as u32)
                .is_some_and(|child| child.kind() == "*")
        })
}

/// Records explicit `{ imported as exported }` entries in an export clause.
///
/// The same representation covers local export aliases and source-backed
/// named re-exports; callers distinguish them by whether the enclosing
/// statement has a `source` specifier. Plain stars have no explicit `bindings`;
/// namespace exports record their alias separately from both `bindings` and `star`.
fn export_bindings(node: Node, bytes: &[u8]) -> Vec<Value> {
    let mut bindings = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "export_clause" {
            continue;
        }
        let mut clause_cursor = child.walk();
        for specifier in child.named_children(&mut clause_cursor) {
            if specifier.kind() != "export_specifier" {
                continue;
            }
            let imported = child_text(specifier, "name", bytes).or_else(|| {
                specifier
                    .named_child(0)
                    .and_then(|name| node_text(name, bytes))
                    .map(|name| name.trim_matches(['"', '\'']).to_string())
            });
            let Some(imported) = imported else {
                continue;
            };
            let exported = child_text(specifier, "alias", bytes)
                .or_else(|| {
                    specifier
                        .named_child(1)
                        .and_then(|name| node_text(name, bytes))
                        .map(|name| name.trim_matches(['"', '\'']).to_string())
                })
                .unwrap_or_else(|| imported.clone());
            bindings.push(json!({
                "imported": imported,
                "exported": exported,
            }));
        }
    }
    bindings
}

fn child_text(node: Node, field: &str, bytes: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(bytes).ok())
        .map(|text| text.trim_matches(['"', '\'']).to_string())
}

fn collect_parse_errors(
    extractor: &str,
    path: &str,
    snapshot: &SnapshotId,
    node: Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if node.is_error() || node.is_missing() {
        diagnostics.push(diagnostic(DiagnosticSpec {
            extractor,
            path,
            snapshot,
            kind: "js_ts_parse_error",
            title: "JavaScript/TypeScript parse error",
            message: format!("tree-sitter reported a parse error in {path}"),
            range: line_range(node),
            payload: json!({
                "node_kind": node.kind(),
            }),
        }));
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() || child.is_error() || child.is_missing() {
            collect_parse_errors(extractor, path, snapshot, child, diagnostics);
        }
    }
}

fn collect_unsupported_top_level(
    extractor: &str,
    path: &str,
    snapshot: &SnapshotId,
    root: Node,
    bytes: &[u8],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "ambient_declaration"
                | "import_statement"
                | "export_statement"
                | "function_declaration"
                | "generator_function_declaration"
                | "class_declaration"
                | "interface_declaration"
                | "module_declaration"
                | "type_alias_declaration"
                | "lexical_declaration"
                | "variable_declaration"
                | "comment"
        ) {
            continue;
        }
        if child.kind().ends_with("declaration") {
            diagnostics.push(diagnostic(DiagnosticSpec {
                extractor,
                path,
                snapshot,
                kind: "js_ts_unsupported_syntax",
                title: "Unsupported JavaScript/TypeScript declaration shape",
                message: format!("{} is not extracted as a canonical declaration yet", child.kind()),
                range: line_range(child),
                payload: json!({
                    "node_kind": child.kind(),
                    "snippet": child.utf8_text(bytes).unwrap_or("").chars().take(120).collect::<String>(),
                }),
            }));
        }
    }
}

#[cfg(feature = "precision-parser")]
const MAX_PRECISION_DIAGNOSTICS: usize = 64;

#[cfg(feature = "precision-parser")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct VerificationRow {
    kind: String,
    key: String,
    range: (u32, u32),
}

#[cfg(feature = "precision-parser")]
#[derive(Debug)]
struct VerificationDifference {
    kind: &'static str,
    key: String,
    primary_range: Option<(u32, u32)>,
    secondary_range: Option<(u32, u32)>,
}

#[cfg(feature = "precision-parser")]
struct PrecisionVerificationInput<'a> {
    content: &'a str,
    declarations: &'a [SourceDeclaration],
    imports: &'a [Value],
    exports: &'a [Value],
    primary_recoveries: usize,
}

#[cfg(feature = "precision-parser")]
fn verify_with_oxc(
    extractor: &str,
    path: &str,
    snapshot: &SnapshotId,
    input: PrecisionVerificationInput<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Value {
    let primary_rows = primary_verification_rows(input.declarations, input.imports, input.exports);
    let secondary = oxc_verification_rows(path, input.content);
    let mut differences = compare_verification_rows(&primary_rows, &secondary.rows);

    append_recovery_difference(
        input.primary_recoveries,
        secondary.recoveries,
        secondary.panicked,
        &mut differences,
    );

    differences.sort_by(|left, right| {
        (left.kind, left.key.as_str()).cmp(&(right.kind, right.key.as_str()))
    });
    let total_differences = differences.len();
    for difference in differences.iter().take(MAX_PRECISION_DIAGNOSTICS) {
        let range = difference
            .primary_range
            .or(difference.secondary_range)
            .unwrap_or((1, 1));
        let diagnostic_kind = match difference.kind {
            "source_range_mismatch" => "js_ts_parser_source_range_mismatch",
            "recovery_difference" => "js_ts_parser_recovery_difference",
            _ => "js_ts_parser_backend_only_finding",
        };
        diagnostics.push(precision_diagnostic(
            DiagnosticSpec {
                extractor,
                path,
                snapshot,
                kind: diagnostic_kind,
                title: "JavaScript/TypeScript parser disagreement",
                message: format!(
                    "tree-sitter and Oxc disagree about {} ({}) in {path}",
                    difference.key, difference.kind
                ),
                range,
                payload: json!({
                    "disagreement_kind": difference.kind,
                    "finding": difference.key,
                    "primary_backend": "tree-sitter",
                    "secondary_backend": "oxc",
                    "primary_range": difference.primary_range,
                    "secondary_range": difference.secondary_range,
                }),
            },
            &difference.key,
        ));
    }

    json!({
        "schema": "athanor.js_ts_precision.v1",
        "enabled": true,
        "primary_backend": "tree-sitter",
        "secondary_backend": "oxc",
        "primary_rows": primary_rows.len(),
        "secondary_rows": secondary.rows.len(),
        "primary_recoveries": input.primary_recoveries,
        "secondary_recoveries": secondary.recoveries,
        "secondary_panicked": secondary.panicked,
        "differences": total_differences,
        "reported_differences": total_differences.min(MAX_PRECISION_DIAGNOSTICS),
        "omitted_differences": total_differences.saturating_sub(MAX_PRECISION_DIAGNOSTICS),
        "diagnostic_limit": MAX_PRECISION_DIAGNOSTICS,
    })
}

#[cfg(feature = "precision-parser")]
fn primary_verification_rows(
    declarations: &[SourceDeclaration],
    imports: &[Value],
    exports: &[Value],
) -> Vec<VerificationRow> {
    let mut rows = declarations
        .iter()
        .filter(|declaration| {
            // A synthesized anonymous-default name has no counterpart in the
            // second backend, so it is verified by neither row set.
            declaration.symbol_kind != "method"
                && declaration.qualified_name != ANONYMOUS_DEFAULT_NAME
                && !matches!(
                    declaration.source_node.as_str(),
                    "arrow_function" | "function" | "function_expression"
                )
        })
        .map(|declaration| VerificationRow {
            kind: "declaration".to_string(),
            key: format!("{}:{}", declaration.symbol_kind, declaration.qualified_name),
            range: declaration.line_range,
        })
        .collect::<Vec<_>>();
    rows.extend(imports.iter().filter_map(|import| {
        Some(VerificationRow {
            kind: "import".to_string(),
            key: import.get("source")?.as_str()?.to_string(),
            range: value_line_range(import),
        })
    }));
    rows.extend(exports.iter().filter_map(|export| {
        Some(VerificationRow {
            kind: "export".to_string(),
            key: export.get("source")?.as_str()?.to_string(),
            range: value_line_range(export),
        })
    }));
    rows.sort_by(|left, right| {
        (&left.kind, &left.key, left.range).cmp(&(&right.kind, &right.key, right.range))
    });
    rows
}

#[cfg(feature = "precision-parser")]
fn value_line_range(value: &Value) -> (u32, u32) {
    (
        value.get("line_start").and_then(Value::as_u64).unwrap_or(1) as u32,
        value.get("line_end").and_then(Value::as_u64).unwrap_or(1) as u32,
    )
}

#[cfg(feature = "precision-parser")]
struct OxcVerification {
    rows: Vec<VerificationRow>,
    recoveries: usize,
    panicked: bool,
}

#[cfg(feature = "precision-parser")]
fn oxc_verification_rows(path: &str, content: &str) -> OxcVerification {
    use std::path::Path;

    use oxc_allocator::Allocator;
    use oxc_ast_visit::Visit;
    use oxc_parser::Parser as OxcParser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(path)).unwrap_or_default();
    let parsed = OxcParser::new(&allocator, content, source_type).parse();
    let recoveries = parsed.diagnostics.len();
    let panicked = parsed.panicked;
    let mut visitor = OxcVerificationVisitor::new(content);
    visitor.visit_program(&parsed.program);
    visitor.rows.sort_by(|left, right| {
        (&left.kind, &left.key, left.range).cmp(&(&right.kind, &right.key, right.range))
    });
    OxcVerification {
        rows: visitor.rows,
        recoveries,
        panicked,
    }
}

#[cfg(feature = "precision-parser")]
struct OxcVerificationVisitor<'s> {
    content: &'s str,
    rows: Vec<VerificationRow>,
}

#[cfg(feature = "precision-parser")]
impl<'s> OxcVerificationVisitor<'s> {
    fn new(content: &'s str) -> Self {
        Self {
            content,
            rows: Vec::new(),
        }
    }

    fn push(&mut self, kind: &str, key: String, span: oxc_span::Span) {
        self.rows.push(VerificationRow {
            kind: kind.to_string(),
            key,
            range: byte_span_line_range(self.content, span.start, span.end),
        });
    }
}

#[cfg(feature = "precision-parser")]
impl<'a> oxc_ast_visit::Visit<'a> for OxcVerificationVisitor<'_> {
    fn visit_function(
        &mut self,
        function: &oxc_ast::ast::Function<'a>,
        flags: oxc_syntax::scope::ScopeFlags,
    ) {
        if let Some(identifier) = &function.id {
            self.push(
                "declaration",
                format!("function:{}", identifier.name.as_str()),
                function.span,
            );
        }
        oxc_ast_visit::walk::walk_function(self, function, flags);
    }

    fn visit_class(&mut self, class: &oxc_ast::ast::Class<'a>) {
        if let Some(identifier) = &class.id {
            self.push(
                "declaration",
                format!("class:{}", identifier.name.as_str()),
                class.span,
            );
        }
        oxc_ast_visit::walk::walk_class(self, class);
    }

    fn visit_ts_interface_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSInterfaceDeclaration<'a>,
    ) {
        self.push(
            "declaration",
            format!("interface:{}", declaration.id.name.as_str()),
            declaration.span,
        );
        oxc_ast_visit::walk::walk_ts_interface_declaration(self, declaration);
    }

    fn visit_ts_type_alias_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSTypeAliasDeclaration<'a>,
    ) {
        self.push(
            "declaration",
            format!("type_alias:{}", declaration.id.name.as_str()),
            declaration.span,
        );
        oxc_ast_visit::walk::walk_ts_type_alias_declaration(self, declaration);
    }

    fn visit_import_declaration(&mut self, declaration: &oxc_ast::ast::ImportDeclaration<'a>) {
        self.push(
            "import",
            declaration.source.value.as_str().to_string(),
            declaration.span,
        );
        oxc_ast_visit::walk::walk_import_declaration(self, declaration);
    }

    fn visit_export_named_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportNamedDeclaration<'a>,
    ) {
        if let Some(source) = &declaration.source {
            self.push(
                "export",
                source.value.as_str().to_string(),
                declaration.span,
            );
        }
        oxc_ast_visit::walk::walk_export_named_declaration(self, declaration);
    }
}

#[cfg(feature = "precision-parser")]
fn byte_span_line_range(content: &str, start: u32, end: u32) -> (u32, u32) {
    let line = |offset: u32| {
        content
            .as_bytes()
            .iter()
            .take(offset as usize)
            .filter(|byte| **byte == b'\n')
            .count() as u32
            + 1
    };
    (line(start), line(end))
}

#[cfg(feature = "precision-parser")]
fn compare_verification_rows(
    primary: &[VerificationRow],
    secondary: &[VerificationRow],
) -> Vec<VerificationDifference> {
    let primary = primary
        .iter()
        .map(|row| ((row.kind.as_str(), row.key.as_str()), row.range))
        .collect::<BTreeMap<_, _>>();
    let secondary = secondary
        .iter()
        .map(|row| ((row.kind.as_str(), row.key.as_str()), row.range))
        .collect::<BTreeMap<_, _>>();
    let mut differences = Vec::new();

    for ((kind, key), primary_range) in &primary {
        match secondary.get(&(*kind, *key)) {
            None => differences.push(VerificationDifference {
                kind: "primary_backend_only",
                key: format!("{kind}:{key}"),
                primary_range: Some(*primary_range),
                secondary_range: None,
            }),
            Some(secondary_range) if secondary_range != primary_range => {
                differences.push(VerificationDifference {
                    kind: "source_range_mismatch",
                    key: format!("{kind}:{key}"),
                    primary_range: Some(*primary_range),
                    secondary_range: Some(*secondary_range),
                });
            }
            Some(_) => {}
        }
    }
    for ((kind, key), secondary_range) in &secondary {
        if !primary.contains_key(&(*kind, *key)) {
            differences.push(VerificationDifference {
                kind: "secondary_backend_only",
                key: format!("{kind}:{key}"),
                primary_range: None,
                secondary_range: Some(*secondary_range),
            });
        }
    }
    differences
}

#[cfg(feature = "precision-parser")]
fn append_recovery_difference(
    primary_recoveries: usize,
    secondary_recoveries: usize,
    secondary_panicked: bool,
    differences: &mut Vec<VerificationDifference>,
) {
    if primary_recoveries != secondary_recoveries || secondary_panicked {
        differences.push(VerificationDifference {
            kind: "recovery_difference",
            key: "parser-recovery".to_string(),
            primary_range: Some((1, 1)),
            secondary_range: Some((1, 1)),
        });
    }
}

#[cfg(feature = "precision-parser")]
fn precision_diagnostic(spec: DiagnosticSpec<'_>, finding: &str) -> Diagnostic {
    let mut output = diagnostic(spec);
    output.id = DiagnosticId(format!(
        "diag_js_ts_parser_verification_{:016x}",
        stable_hash(format!("{}:{finding}", output.id.0).as_bytes())
    ));
    output
}

struct DiagnosticSpec<'a> {
    extractor: &'a str,
    path: &'a str,
    snapshot: &'a SnapshotId,
    kind: &'a str,
    title: &'a str,
    message: String,
    range: (u32, u32),
    payload: Value,
}

fn diagnostic(spec: DiagnosticSpec) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_{}_{:016x}",
            spec.kind,
            stable_hash(
                format!(
                    "{}:{}:{}:{}",
                    spec.path, spec.kind, spec.range.0, spec.range.1
                )
                .as_bytes()
            )
        )),
        kind: DiagnosticKind::Other(spec.kind.to_string()),
        severity: Severity::Low,
        status: DiagnosticStatus::Open,
        title: spec.title.to_string(),
        message: spec.message,
        entities: Vec::new(),
        evidence: vec![evidence_for_file(
            spec.path,
            spec.extractor,
            Some(spec.range.0),
            Some(spec.range.1),
        )],
        ownership: ownership_for_file(spec.path),
        snapshot: spec.snapshot.clone(),
        suggested_fix: None,
        payload: spec.payload,
    }
}

#[derive(Clone, Copy)]
struct ExtractionEvidenceContext<'a> {
    extractor: &'a str,
    path: &'a str,
    snapshot: &'a SnapshotId,
}

struct SymbolFactSpec<'a> {
    subject: &'a EntityId,
    object: Option<&'a EntityId>,
    stable_key: &'a StableKey,
    symbol_kind: &'a str,
    range: (u32, u32),
}

fn symbol_fact(context: ExtractionEvidenceContext, spec: SymbolFactSpec) -> Fact {
    Fact {
        id: FactId(format!(
            "fact_js_ts_symbol_defined_{:016x}",
            stable_hash(spec.stable_key.0.as_bytes())
        )),
        kind: FactKind::SymbolDefined,
        subject: spec.subject.clone(),
        object: spec.object.cloned(),
        value: json!({
            "stable_key": spec.stable_key.0,
            "path": context.path,
            "symbol_kind": spec.symbol_kind,
        }),
        evidence: vec![evidence_for_file(
            context.path,
            context.extractor,
            Some(spec.range.0),
            Some(spec.range.1),
        )],
        ownership: ownership_for_file(context.path),
        snapshot: context.snapshot.clone(),
        extractor: context.extractor.to_string(),
        confidence: 1.0,
    }
}

fn extract_package_json(extractor: &str, input: &ExtractInput, content: &str) -> ExtractOutput {
    let Ok(root) = serde_json::from_str::<Value>(content) else {
        let diagnostic = diagnostic(DiagnosticSpec {
            extractor,
            path: &input.source.path,
            snapshot: &input.snapshot,
            kind: "js_ts_package_json_parse_error",
            title: "package.json parse error",
            message: format!("failed to parse {}", input.source.path),
            range: (1, 1),
            payload: json!({}),
        });
        return ExtractOutput {
            entities: Vec::new(),
            facts: Vec::new(),
            diagnostics: vec![diagnostic],
        };
    };

    let package_name = root
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("(anonymous)");
    // Workspace patterns and the package entry point are the evidence a linker
    // needs to resolve a bare workspace-package specifier inside the repository.
    let workspaces = workspace_patterns(root.get("workspaces"));
    let main = root
        .get("main")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let package_stable_key = StableKey(format!("package://npm:{package_name}"));
    let package_id = EntityId(format!(
        "ent_js_ts_package_{:016x}",
        stable_hash(package_stable_key.0.as_bytes())
    ));
    let mut entities = vec![Entity {
        id: package_id.clone(),
        stable_key: package_stable_key.clone(),
        kind: EntityKind::Package,
        name: package_name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(1),
            line_end: None,
        }),
        language: Some(LanguageCode("json".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(&input.source.path),
            payload: json!({
                "package_manager": "npm",
                "version": root.get("version").and_then(Value::as_str),
                "private": root.get("private").and_then(Value::as_bool),
                "main": main,
                "workspaces": workspaces.patterns,
                "omitted_workspaces": workspaces.omitted,
            }),
        }];
    let context = ExtractionEvidenceContext {
        extractor,
        path: &input.source.path,
        snapshot: &input.snapshot,
    };
    let mut facts = vec![symbol_fact(
        context,
        SymbolFactSpec {
            subject: &package_id,
            object: None,
            stable_key: &package_stable_key,
            symbol_kind: "package",
            range: (1, 1),
        },
    )];

    for dependency_kind in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        let Some(dependencies) = root.get(dependency_kind).and_then(Value::as_object) else {
            continue;
        };
        for (name, version) in dependencies {
            let stable_key = StableKey(format!("dependency://npm:{name}"));
            let entity_id = EntityId(format!(
                "ent_js_ts_dependency_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            ));
            entities.push(Entity {
                id: entity_id.clone(),
                stable_key: stable_key.clone(),
                kind: EntityKind::Dependency,
                name: name.clone(),
                title: None,
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(1),
                    line_end: None,
                }),
                language: Some(LanguageCode("json".to_string())),
                aliases: Vec::new(),
                ownership: ownership_for_file(&input.source.path),
                payload: json!({
                    "ecosystem": "npm",
                    "dependency_kind": dependency_kind,
                    "requirement": version.as_str(),
                    "package": package_name,
                }),
            });
            facts.push(symbol_fact(
                context,
                SymbolFactSpec {
                    subject: &entity_id,
                    object: Some(&package_id),
                    stable_key: &stable_key,
                    symbol_kind: dependency_kind,
                    range: (1, 1),
                },
            ));
        }
    }

    entities.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    facts.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    ExtractOutput {
        entities,
        facts,
        diagnostics: Vec::new(),
    }
}

/// Projects the bounded module-resolution options of one first-party
/// TypeScript or JavaScript configuration file.
///
/// `tsconfig.json` and `jsconfig.json` map non-relative import specifiers onto
/// repository paths, so they are the evidence that lets a linker resolve
/// `@/lib/api` or a workspace package name without guessing. The projection
/// records only that evidence: the `baseUrl`-resolved base directory, each
/// `paths` pattern with its substitution targets and line, and an inherited
/// `extends` specifier. Every other compiler option stays outside the model.
fn extract_typescript_config(
    extractor: &str,
    input: &ExtractInput,
    content: &str,
) -> ExtractOutput {
    let normalized = strip_trailing_commas(&strip_json_comments(content));
    let Ok(root) = serde_json::from_str::<Value>(&normalized) else {
        let diagnostic = diagnostic(DiagnosticSpec {
            extractor,
            path: &input.source.path,
            snapshot: &input.snapshot,
            kind: "js_ts_typescript_config_parse_error",
            title: "TypeScript configuration parse error",
            message: format!("failed to parse {}", input.source.path),
            range: (1, 1),
            payload: json!({}),
        });
        return ExtractOutput {
            entities: Vec::new(),
            facts: Vec::new(),
            diagnostics: vec![diagnostic],
        };
    };

    let Some(config) = parse_typescript_config(&input.source.path, &normalized, &root) else {
        return ExtractOutput::default();
    };

    let file_name = config_file_name(&input.source.path);
    let label = if file_name.starts_with("jsconfig") {
        "JavaScript"
    } else {
        "TypeScript"
    };
    let config_path = input.source.path.as_str();
    let stable_key = StableKey(format!("config://{config_path}#js-ts-module-resolution"));
    let entity_id = EntityId(format!(
        "ent_js_ts_config_{:016x}",
        stable_hash(stable_key.0.as_bytes())
    ));
    let line = json_member_line(&normalized, "compilerOptions").unwrap_or(1);
    let mut mappings = Vec::new();
    for mapping in &config.paths {
        mappings.push(json!({
            "pattern": mapping.pattern,
            "targets": mapping.targets,
            "line": mapping.line,
        }));
    }

    let entity = Entity {
        id: entity_id.clone(),
        stable_key: stable_key.clone(),
        kind: EntityKind::Feature,
        name: file_name,
        title: Some(format!("{label} module resolution configuration")),
        source: Some(SourceLocation {
            path: input.source.path.clone(),
            line_start: Some(line),
            line_end: None,
        }),
        language: Some(LanguageCode("json".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(&input.source.path),
        payload: json!({
            "config_kind": "js_ts_module_resolution",
            "extends": config.extends,
            "base_dir": config.base_dir,
            "paths": mappings,
            "omitted_paths": config.omitted_paths,
            "omitted_targets": config.omitted_targets,
        }),
    };
    let fact = symbol_fact(
        ExtractionEvidenceContext {
            extractor,
            path: &input.source.path,
            snapshot: &input.snapshot,
        },
        SymbolFactSpec {
            subject: &entity_id,
            object: None,
            stable_key: &stable_key,
            symbol_kind: "module_resolution_config",
            range: (line, line),
        },
    );

    ExtractOutput {
        entities: vec![entity],
        facts: vec![fact],
        diagnostics: Vec::new(),
    }
}

struct TypeScriptConfig {
    extends: Option<String>,
    base_dir: Option<String>,
    paths: Vec<PathMapping>,
    omitted_paths: usize,
    omitted_targets: usize,
}

struct PathMapping {
    pattern: String,
    targets: Vec<String>,
    line: Option<u32>,
}

/// Reads the module-resolution options of one configuration file.
///
/// A file without `compilerOptions` and without `extends` carries no
/// resolution evidence, so it is not projected at all.
fn parse_typescript_config(path: &str, content: &str, root: &Value) -> Option<TypeScriptConfig> {
    let compiler_options = root.get("compilerOptions").and_then(Value::as_object);
    let extends = root
        .get("extends")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if compiler_options.is_none() && extends.is_none() {
        return None;
    }

    let base_dir = match compiler_options
        .and_then(|options| options.get("baseUrl"))
        .and_then(Value::as_str)
    {
        Some(base_url) => config_base_dir(path, base_url),
        None => Some(config_directory(path)),
    };
    let (paths, omitted_paths, omitted_targets) =
        match compiler_options.and_then(|options| options.get("paths")) {
            Some(paths) => parse_path_mappings(content, paths),
            None => (Vec::new(), 0, 0),
        };

    Some(TypeScriptConfig {
        extends,
        base_dir,
        paths,
        omitted_paths,
        omitted_targets,
    })
}

fn parse_path_mappings(content: &str, paths: &Value) -> (Vec<PathMapping>, usize, usize) {
    let mut mappings = Vec::new();
    let mut omitted_paths = 0usize;
    let mut omitted_targets = 0usize;
    let Some(entries) = paths.as_object() else {
        return (mappings, omitted_paths, omitted_targets);
    };

    for (pattern, targets) in entries {
        let recorded_targets = match targets.as_array() {
            Some(targets) => bounded_path_targets(targets, &mut omitted_targets),
            None => Vec::new(),
        };
        let supported_pattern = !pattern.trim().is_empty() && pattern.matches('*').count() <= 1;
        if !supported_pattern
            || recorded_targets.is_empty()
            || mappings.len() == MAX_PATH_MAPPINGS
        {
            omitted_paths += 1;
            continue;
        }
        mappings.push(PathMapping {
            pattern: pattern.trim().to_string(),
            targets: recorded_targets,
            line: json_member_line(content, pattern),
        });
    }

    (mappings, omitted_paths, omitted_targets)
}

fn bounded_path_targets(targets: &[Value], omitted_targets: &mut usize) -> Vec<String> {
    let mut recorded = Vec::new();
    for target in targets {
        let Some(target) = target
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            *omitted_targets += 1;
            continue;
        };
        if recorded.len() == MAX_PATH_TARGETS {
            *omitted_targets += 1;
            continue;
        }
        recorded.push(target.to_string());
    }
    recorded
}

/// Resolves `compilerOptions.baseUrl` against the directory of the declaring
/// configuration file.
///
/// An absolute, drive-qualified, empty, or repository-escaping `baseUrl` has no
/// canonical repository directory, so it is reported as unsupported instead of
/// being normalized into a path the model cannot prove.
fn config_base_dir(path: &str, base_url: &str) -> Option<String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains(':') {
        return None;
    }
    join_config_dir(&config_directory(path), trimmed)
}

fn config_directory(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .rsplit_once('/')
        .map_or(String::new(), |(parent, _)| parent.to_string())
}

fn config_file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_string()
}

/// Joins a repository-relative directory with a configuration-relative path.
///
/// A path that walks above the repository root has no canonical target, so the
/// join fails closed instead of producing `../` segments.
fn join_config_dir(base: &str, relative: &str) -> Option<String> {
    let mut parts = base
        .split('/')
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for part in relative.replace('\\', "/").split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other.to_string()),
        }
    }
    Some(parts.join("/"))
}

/// Line of one object member, found by its quoted key.
///
/// TypeScript configuration files are JSONC, so formatting varies; the search
/// keeps recorded evidence on the line that declares the member and reports
/// `None` rather than a guessed line when the key is not at the start of a line.
fn json_member_line(content: &str, key: &str) -> Option<u32> {
    let needle = format!("\"{key}\":");
    for (index, line) in content.lines().enumerate() {
        if line.trim_start().starts_with(&needle) {
            return Some((index + 1) as u32);
        }
    }
    None
}

/// Removes JSONC line and block comments outside string literals.
///
/// Newlines inside block comments are preserved, so recorded line numbers keep
/// pointing at the same text after normalization.
fn strip_json_comments(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut characters = content.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if in_string {
            output.push(character);
            match (escaped, character) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        if character != '/' {
            if character == '"' {
                in_string = true;
            }
            output.push(character);
            continue;
        }
        match characters.peek().copied() {
            Some('/') => {
                characters.next();
                for skipped in characters.by_ref() {
                    if skipped == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            Some('*') => {
                characters.next();
                let mut previous = '*';
                for skipped in characters.by_ref() {
                    if previous == '*' && skipped == '/' {
                        break;
                    }
                    if skipped == '\n' {
                        output.push('\n');
                    }
                    previous = skipped;
                }
            }
            _ => output.push(character),
        }
    }
    output
}

/// Removes trailing commas outside string literals.
///
/// TypeScript configuration files may end an object or array member list with a
/// comma; strict JSON rejects it. The scan is string-aware so a comma inside a
/// value is never removed.
fn strip_trailing_commas(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut characters = content.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if in_string {
            output.push(character);
            match (escaped, character) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        if character == ',' {
            let mut next = None;
            for candidate in characters.clone() {
                if candidate.is_whitespace() {
                    continue;
                }
                next = Some(candidate);
                break;
            }
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
            output.push(character);
            continue;
        }
        if character == '"' {
            in_string = true;
        }
        output.push(character);
    }
    output
}

struct WorkspacePatterns {
    patterns: Vec<String>,
    omitted: usize,
}

/// Reads the bounded workspace glob patterns of one `package.json`.
///
/// Both the array form and the `{ "packages": [...] }` object form are
/// recognized; a non-string entry or an entry beyond the bound is counted as
/// omitted instead of being silently dropped.
fn workspace_patterns(value: Option<&Value>) -> WorkspacePatterns {
    let mut patterns = Vec::new();
    let mut omitted = 0usize;
    let entries = match value {
        Some(Value::Array(entries)) => entries,
        Some(object) => match object.get("packages").and_then(Value::as_array) {
            Some(entries) => entries,
            None => return WorkspacePatterns { patterns, omitted },
        },
        None => return WorkspacePatterns { patterns, omitted },
    };
    for entry in entries {
        let pattern = entry.as_str().map(str::trim).filter(|value| !value.is_empty());
        let Some(pattern) = pattern else {
            omitted += 1;
            continue;
        };
        if patterns.len() == MAX_WORKSPACE_PATTERNS {
            omitted += 1;
            continue;
        }
        if !patterns.iter().any(|existing| existing == pattern) {
            patterns.push(pattern.to_string());
        }
    }
    WorkspacePatterns { patterns, omitted }
}

fn line_range(node: Node) -> (u32, u32) {
    (
        node.start_position().row as u32 + 1,
        node.end_position().row as u32 + 1,
    )
}

fn module_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn extracts_javascript_imports_exports_functions_and_classes() {
        let output = JsTsExtractor
            .extract(input(
                "src/auth.js",
                "import jwt from 'jsonwebtoken';\nexport function login() {}\nclass Session { refresh() {} }\nconst logout = () => null;\n",
                "javascript",
            ))
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Module
                && entity.payload["imports"][0]["source"] == "jsonwebtoken"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Function
                && entity.stable_key.0 == "symbol://js-ts:src/auth.js#login"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Class
                && entity.stable_key.0 == "symbol://js-ts:src/auth.js#Session"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Function
                && entity.stable_key.0 == "symbol://js-ts:src/auth.js#logout"
        }));
        assert!(output.facts.iter().all(|fact| {
            !fact.evidence.is_empty()
                && !fact.ownership.is_empty()
                && fact.kind == FactKind::SymbolDefined
        }));
    }

    #[tokio::test]
    async fn records_import_bindings_for_default_named_and_namespace_imports() {
        let output = JsTsExtractor
            .extract(input(
                "src/auth.ts",
                "import api, { fetchUser as fetch, login } from './client';\nimport * as utils from './utils';\nimport './side-effect';\n",
                "typescript",
            ))
            .await
            .unwrap();

        let module = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Module)
            .expect("module entity must exist");

        assert_eq!(module.payload["imports"][0]["source"], "./client");
        assert_eq!(module.payload["imports"][0]["default_name"], "api");
        assert_eq!(module.payload["imports"][0]["named"][0], "fetch");
        assert_eq!(module.payload["imports"][0]["named"][1], "login");
        assert_eq!(
            module.payload["imports"][0]["named_bindings"][0],
            json!({"imported": "fetchUser", "local": "fetch"})
        );
        assert_eq!(
            module.payload["imports"][0]["named_bindings"][1],
            json!({"imported": "login", "local": "login"})
        );
        assert_eq!(module.payload["imports"][0]["namespace_name"], Value::Null);
        assert_eq!(module.payload["imports"][1]["namespace_name"], "utils");
        assert!(module.payload["imports"][1]["default_name"].is_null());
        assert_eq!(
            module.payload["imports"][1]["named"]
                .as_array()
                .expect("named imports array")
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn records_named_export_aliases_and_source_backed_reexports() {
        for (path, language) in [
            ("src/barrel.js", "javascript"),
            ("src/barrel.ts", "typescript"),
        ] {
            let output = JsTsExtractor
                .extract(input(
                    path,
                    "export { login as signIn, logout } from './auth';\nexport { local as publicLocal };\nexport * from './wild';\nexport * as ns from './namespace';\nexport { default as signIn } from './default-auth';\n",
                    language,
                ))
                .await
                .unwrap();
            let module = output
                .entities
                .iter()
                .find(|entity| entity.kind == EntityKind::Module)
                .expect("module entity must exist");

            assert_eq!(module.payload["exports"][0]["source"], "./auth");
            assert_eq!(module.payload["exports"][0]["star"], false);
            assert_eq!(module.payload["exports"][0]["default_export"], false);
            assert!(module.payload["exports"][0]["default_name"].is_null());
            assert_eq!(
                module.payload["exports"][0]["bindings"][0],
                json!({"imported": "login", "exported": "signIn"})
            );
            assert_eq!(
                module.payload["exports"][0]["bindings"][1],
                json!({"imported": "logout", "exported": "logout"})
            );
            assert!(module.payload["exports"][1]["source"].is_null());
            assert_eq!(module.payload["exports"][1]["star"], false);
            assert_eq!(
                module.payload["exports"][1]["bindings"][0],
                json!({"imported": "local", "exported": "publicLocal"})
            );
            assert_eq!(module.payload["exports"][2]["source"], "./wild");
            assert_eq!(module.payload["exports"][2]["star"], true);
            assert!(module.payload["exports"][2]["namespace_name"].is_null());
            assert!(
                module.payload["exports"][2]["bindings"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(module.payload["exports"][3]["source"], "./namespace");
            assert_eq!(module.payload["exports"][3]["star"], false);
            assert_eq!(module.payload["exports"][3]["namespace_name"], "ns");
            assert!(
                module.payload["exports"][3]["bindings"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(module.payload["exports"][4]["default_export"], false);
            assert!(module.payload["exports"][4]["default_name"].is_null());
            assert_eq!(
                module.payload["exports"][4]["bindings"][0],
                json!({"imported": "default", "exported": "signIn"})
            );
        }
    }

    #[tokio::test]
    async fn records_named_default_export_bindings() {
        for (path, language) in [
            ("src/default.js", "javascript"),
            ("src/default.ts", "typescript"),
        ] {
            for (source, expected_name) in [
                (
                    "export default function authenticate() {}\n",
                    Some("authenticate"),
                ),
                (
                    "function authenticate() {}\nexport default authenticate;\n",
                    Some("authenticate"),
                ),
                ("export default () => {};\n", Some("default")),
                (
                    "export default class ApiController {}\n",
                    Some("ApiController"),
                ),
                (
                    "class Controller {}\nexport default Controller;\n",
                    Some("Controller"),
                ),
                ("export default class {}\n", Some("default")),
            ] {
                let output = JsTsExtractor
                    .extract(input(path, source, language))
                    .await
                    .unwrap();
                let module = output
                    .entities
                    .iter()
                    .find(|entity| entity.kind == EntityKind::Module)
                    .expect("module entity must exist");

                assert_eq!(module.payload["exports"][0]["default_export"], true);
                if let Some(expected_name) = expected_name {
                    assert_eq!(module.payload["exports"][0]["default_name"], expected_name);
                } else {
                    assert!(module.payload["exports"][0]["default_name"].is_null());
                }
            }
        }
    }

    #[tokio::test]
    async fn records_anonymous_default_declarations_under_a_canonical_name() {
        for (path, language, source, expected_kind, symbol_kind) in [
            (
                "src/anon.js",
                "javascript",
                "export default function (request) {\n  return send(request);\n}\n",
                EntityKind::Function,
                "function",
            ),
            (
                "src/async_anon.ts",
                "typescript",
                "export default async function () {}\n",
                EntityKind::Function,
                "function",
            ),
            (
                "src/arrow.js",
                "javascript",
                "export default () => {\n  return 1;\n};\n",
                EntityKind::Function,
                "function",
            ),
            (
                "src/anon_class.ts",
                "typescript",
                "export default class {\n  run() {}\n}\n",
                EntityKind::Class,
                "class",
            ),
        ] {
            let output = JsTsExtractor
                .extract(input(path, source, language))
                .await
                .unwrap();
            let default = output
                .entities
                .iter()
                .find(|entity| entity.stable_key.0 == format!("symbol://js-ts:{path}#default"))
                .expect("anonymous default declaration");

            assert_eq!(default.kind, expected_kind);
            assert_eq!(default.name, "default");
            assert_eq!(default.payload["qualified_name"], "default");
            assert_eq!(default.payload["symbol_kind"], symbol_kind);
            assert_eq!(default.payload["exported"], true);
            assert_eq!(default.payload["anonymous_default"], true);

            let module = output
                .entities
                .iter()
                .find(|entity| entity.kind == EntityKind::Module)
                .expect("module entity must exist");
            assert_eq!(module.payload["exports"][0]["default_export"], true);
            assert_eq!(module.payload["exports"][0]["default_name"], "default");
        }
    }

    #[tokio::test]
    async fn records_class_members_of_an_anonymous_default_class() {
        let output = JsTsExtractor
            .extract(input(
                "src/anon_class.ts",
                "export default class {\n  run() {}\n}\n",
                "typescript",
            ))
            .await
            .unwrap();

        let mut members = 0;
        for entity in &output.entities {
            if entity.stable_key.0 == "symbol://js-ts:src/anon_class.ts#default.run" {
                members += 1;
            }
        }
        assert_eq!(members, 1);
    }

    #[tokio::test]
    async fn records_call_sites_inside_an_anonymous_default_function() {
        let output = JsTsExtractor
            .extract(input(
                "src/anon.ts",
                "export default function (request) {\n  return send(request);\n}\n",
                "typescript",
            ))
            .await
            .unwrap();

        let default = output
            .entities
            .iter()
            .find(|entity| entity.name == "default")
            .expect("anonymous default declaration");
        let calls = default.payload["calls"].as_array().expect("call sites");
        assert!(calls.iter().any(|call| call["callee"] == "send"));
    }

    #[tokio::test]
    async fn anonymous_default_expressions_without_a_declaration_stay_unbound() {
        for source in [
            "export default 42;\n",
            "export default { run() {} };\n",
            "export default helper();\n",
            "export default new Service();\n",
        ] {
            let output = JsTsExtractor
                .extract(input("src/value.ts", source, "typescript"))
                .await
                .unwrap();

            for entity in &output.entities {
                assert_ne!(entity.name, "default");
            }
            let module = output
                .entities
                .iter()
                .find(|entity| entity.kind == EntityKind::Module)
                .expect("module entity must exist");
            assert_eq!(module.payload["exports"][0]["default_export"], true);
            assert!(module.payload["exports"][0]["default_name"].is_null());
        }
    }

    #[tokio::test]
    async fn records_at_most_one_anonymous_default_per_file() {
        let output = JsTsExtractor
            .extract(input(
                "src/duplicate.ts",
                "export default function () {}\nexport default function () {}\n",
                "typescript",
            ))
            .await
            .unwrap();

        let mut defaults = 0;
        for entity in &output.entities {
            if entity.name == "default" {
                defaults += 1;
            }
        }
        assert_eq!(defaults, 1);
    }

    #[tokio::test]
    async fn records_class_and_interface_heritage_separately() {
        let output = JsTsExtractor
            .extract(input(
                "src/service.ts",
                "export class Service extends api.Base implements Store<K>, Closeable {}\nclass Plain {}\ninterface Store<T> extends Parent<T>, api.Hierarchy {}\ninterface Parent<T> {}\ninterface Hierarchy {}\n",
                "typescript",
            ))
            .await
            .unwrap();

        let service = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/service.ts#Service")
            .expect("class entity must exist");
        assert_eq!(service.payload["extends"], json!(["Base"]));
        assert_eq!(
            service.payload["implements"],
            json!(["Store", "Closeable"]),
            "generic arguments are stripped and declaration order is preserved"
        );

        let store = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/service.ts#Store")
            .expect("interface entity must exist");
        assert_eq!(
            store.payload["extends"],
            json!(["Parent", "Hierarchy"]),
            "interface extends clauses can name multiple generic or qualified parents"
        );
        assert_eq!(store.payload["implements"], json!([]));

        let plain = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/service.ts#Plain")
            .expect("plain class entity must exist");
        assert!(plain.payload.get("extends").is_none());
        assert!(plain.payload.get("implements").is_none());
    }

    #[tokio::test]
    async fn records_per_declaration_call_sites_with_lines_and_receivers() {
        let output = JsTsExtractor
            .extract(input(
                "src/auth.ts",
                "import api, { login } from './client';\nimport * as utils from './utils';\nexport function run() {\n  login();\n  api.send({ id: 1 });\n  utils.normalize(id);\n  return new Session().refresh();\n}\nclass Session { refresh() { login(); } }\n",
                "typescript",
            ))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/auth.ts#run")
            .expect("run declaration must exist");
        let calls = run.payload["calls"].as_array().expect("calls array");

        assert!(calls.iter().any(|call| {
            call["callee"] == "login" && call["kind"] == "call" && call["line"].as_u64() == Some(4)
        }));
        assert!(calls.iter().any(|call| {
            call["callee"] == "api.send"
                && call["name"] == "send"
                && call["receiver"] == "api"
                && call["arg_count"].as_u64() == Some(1)
        }));
        assert!(calls.iter().any(|call| call["callee"] == "utils.normalize"));
        assert!(calls.iter().any(|call| {
            call["callee"] == "Session" && call["kind"] == "new" && call["line"].as_u64() == Some(7)
        }));

        let refresh = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/auth.ts#Session.refresh")
            .expect("method declaration must exist");
        assert!(
            refresh.payload["calls"]
                .as_array()
                .expect("calls array")
                .iter()
                .any(|call| call["callee"] == "login")
        );
    }

    #[tokio::test]
    async fn records_callback_arguments_for_known_combinators() {
        let output = JsTsExtractor
            .extract(input(
                "src/pipeline.ts",
                "import { filterActive } from './filters';\nexport function run(items, promise, emitter) {\n  items.map(filterActive);\n  promise.then(handleOk);\n  emitter.addEventListener('change', this.onChange);\n  setTimeout(tick, 100);\n  items.reduce(combine, seed, one, two, three, four);\n  items.filter((item) => item.ok);\n  items.sort();\n  queue.push(skipMe);\n}\n",
                "typescript",
            ))
            .await
            .unwrap();

        let run = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/pipeline.ts#run")
            .expect("run declaration must exist");
        let calls = run.payload["calls"].as_array().expect("calls array");

        let map_call = calls
            .iter()
            .find(|call| call["callee"] == "items.map")
            .expect("map call site must exist");
        assert_eq!(
            map_call["callback_arguments"],
            json!(["filterActive"]),
            "identifier arguments of a known combinator are recorded"
        );

        let then_call = calls
            .iter()
            .find(|call| call["callee"] == "promise.then")
            .expect("then call site must exist");
        assert_eq!(
            then_call["callback_arguments"],
            json!(["handleOk"]),
            "promise handler arguments are recorded"
        );

        let listener_call = calls
            .iter()
            .find(|call| call["callee"] == "emitter.addEventListener")
            .expect("addEventListener call site must exist");
        assert_eq!(
            listener_call["callback_arguments"],
            json!(["this.onChange"]),
            "member-path callbacks keep their receiver and literals are skipped"
        );

        let timer_call = calls
            .iter()
            .find(|call| call["callee"] == "setTimeout")
            .expect("setTimeout call site must exist");
        assert_eq!(timer_call["callback_arguments"], json!(["tick"]));

        let reduce_call = calls
            .iter()
            .find(|call| call["callee"] == "items.reduce")
            .expect("reduce call site must exist");
        assert_eq!(
            reduce_call["callback_arguments"],
            json!(["combine", "seed", "one", "two"]),
            "callback arguments stay bounded per call site"
        );

        for callee in ["items.filter", "items.sort", "queue.push"] {
            let call = calls
                .iter()
                .find(|call| call["callee"] == callee)
                .unwrap_or_else(|| panic!("{callee} call site must exist"));
            assert!(
                call.get("callback_arguments").is_none(),
                "{callee} must not record callback arguments"
            );
        }
    }

    #[tokio::test]
    async fn skips_computed_and_dynamic_call_targets() {
        let output = JsTsExtractor
            .extract(input(
                "src/dispatch.ts",
                "export function dispatch() {\n  handlers[key]();\n  registry[cacheKey].run();\n  run();\n}\n",
                "typescript",
            ))
            .await
            .unwrap();

        let dispatch = output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == "symbol://js-ts:src/dispatch.ts#dispatch")
            .expect("dispatch declaration must exist");
        let calls = dispatch.payload["calls"].as_array().expect("calls array");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["callee"], "run");
    }

    #[tokio::test]
    async fn extracts_typescript_interfaces_type_aliases_and_tsx_functions() {
        let ts = JsTsExtractor
            .extract(input(
                "src/types.ts",
                "export interface User { id: string }\ntype UserId = string;\n",
                "typescript",
            ))
            .await
            .unwrap();
        assert!(ts.entities.iter().any(|entity| {
            entity.kind == EntityKind::Symbol
                && entity.stable_key.0 == "symbol://js-ts:src/types.ts#User"
                && entity.payload["symbol_kind"] == "interface"
        }));
        assert!(ts.entities.iter().any(|entity| {
            entity.kind == EntityKind::Symbol
                && entity.stable_key.0 == "symbol://js-ts:src/types.ts#UserId"
                && entity.payload["symbol_kind"] == "type_alias"
        }));

        let tsx = JsTsExtractor
            .extract(input(
                "src/App.tsx",
                "export const App = () => <main>Hello</main>;\n",
                "typescriptreact",
            ))
            .await
            .unwrap();
        assert!(tsx.entities.iter().any(|entity| {
            entity.kind == EntityKind::Function
                && entity.stable_key.0 == "symbol://js-ts:src/App.tsx#App"
        }));
    }

    #[tokio::test]
    async fn reports_parser_errors_as_diagnostics_without_failing_extraction() {
        let output = JsTsExtractor
            .extract(input(
                "src/broken.ts",
                "export function broken(",
                "typescript",
            ))
            .await
            .unwrap();

        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.kind == EntityKind::Module)
        );
        assert!(
            output
                .facts
                .iter()
                .any(|fact| fact.kind == FactKind::SymbolDefined)
        );
        let parse_errors = output
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.kind == DiagnosticKind::Other("js_ts_parse_error".to_string())
            })
            .count();
        assert_eq!(parse_errors, 1);
    }

    #[tokio::test]
    async fn ignores_top_level_runtime_statements_as_unsupported_declarations() {
        let output = JsTsExtractor
            .extract(input(
                "scripts/check.mjs",
                "console.log('checking');\nif (process.env.CI) { console.log('ci'); }\n",
                "javascript",
            ))
            .await
            .unwrap();

        assert!(output.diagnostics.iter().all(|diagnostic| {
            diagnostic.kind != DiagnosticKind::Other("js_ts_unsupported_syntax".to_string())
        }));
    }

    #[tokio::test]
    async fn parses_node_shebang_without_parser_diagnostics() {
        let output = JsTsExtractor
            .extract(input(
                "scripts/check.mjs",
                "#!/usr/bin/env node\nconsole.log('checking');\n",
                "javascript",
            ))
            .await
            .unwrap();

        assert!(output.diagnostics.iter().all(|diagnostic| {
            diagnostic.kind != DiagnosticKind::Other("js_ts_parse_error".to_string())
        }));
    }

    #[tokio::test]
    async fn parses_utf8_bom_prefixed_tsx_without_parser_diagnostics() {
        let output = JsTsExtractor
            .extract(input(
                "src/index.tsx",
                "\u{feff}export const App = () => <main />;\n",
                "typescriptreact",
            ))
            .await
            .unwrap();

        assert!(output.diagnostics.iter().all(|diagnostic| {
            diagnostic.kind != DiagnosticKind::Other("js_ts_parse_error".to_string())
        }));
    }

    #[tokio::test]
    async fn accepts_typescript_ambient_module_declarations() {
        let output = JsTsExtractor
            .extract(input(
                "src/next-auth.d.ts",
                "import { DefaultSession } from 'next-auth';\n\ndeclare module 'next-auth' {\n  interface Session { user: DefaultSession['user'] }\n}\n",
                "typescript",
            ))
            .await
            .unwrap();

        assert!(output.diagnostics.iter().all(|diagnostic| {
            diagnostic.kind != DiagnosticKind::Other("js_ts_unsupported_syntax".to_string())
        }));
    }

    #[tokio::test]
    async fn extracts_package_dependencies() {
        let output = JsTsExtractor
            .extract(input(
                "package.json",
                r#"{"name":"example","dependencies":{"react":"^19.0.0"},"devDependencies":{"vitest":"latest"}}"#,
                "json",
            ))
            .await
            .unwrap();

        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Package && entity.stable_key.0 == "package://npm:example"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Dependency
                && entity.stable_key.0 == "dependency://npm:react"
                && entity.payload["dependency_kind"] == "dependencies"
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Dependency
                && entity.stable_key.0 == "dependency://npm:vitest"
                && entity.payload["dependency_kind"] == "devDependencies"
        }));
    }

    #[cfg(feature = "precision-parser")]
    #[tokio::test]
    async fn precision_mode_records_agreeing_backend_rows_without_changing_ids() {
        let output = JsTsExtractor
            .extract(input(
                "src/auth.ts",
                "import { token } from './token';\nexport function login() {}\ninterface Session {}\n",
                "typescript",
            ))
            .await
            .unwrap();
        let module = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Module)
            .unwrap();

        assert_eq!(
            module.stable_key.0, "module://js-ts:src/auth.ts",
            "precision mode must retain normal canonical ids"
        );
        assert_eq!(module.payload["precision_verification"]["differences"], 0);
    }

    #[cfg(feature = "precision-parser")]
    #[tokio::test]
    async fn precision_mode_does_not_report_the_synthesized_anonymous_default_name() {
        let output = JsTsExtractor
            .extract(input(
                "src/anon.ts",
                "export default function () {}\n",
                "typescript",
            ))
            .await
            .unwrap();
        let module = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Module)
            .unwrap();

        assert_eq!(module.payload["precision_verification"]["differences"], 0);
    }

    #[cfg(feature = "precision-parser")]
    #[test]
    fn precision_comparison_reports_backend_only_and_range_findings() {
        let primary = vec![
            VerificationRow {
                kind: "declaration".to_string(),
                key: "function:login".to_string(),
                range: (2, 2),
            },
            VerificationRow {
                kind: "import".to_string(),
                key: "./primary".to_string(),
                range: (1, 1),
            },
        ];
        let secondary = vec![
            VerificationRow {
                kind: "declaration".to_string(),
                key: "function:login".to_string(),
                range: (2, 3),
            },
            VerificationRow {
                kind: "import".to_string(),
                key: "./secondary".to_string(),
                range: (1, 1),
            },
        ];

        let differences = compare_verification_rows(&primary, &secondary);
        assert!(
            differences
                .iter()
                .any(|difference| difference.kind == "source_range_mismatch")
        );
        assert!(
            differences
                .iter()
                .any(|difference| difference.kind == "primary_backend_only")
        );
        assert!(
            differences
                .iter()
                .any(|difference| difference.kind == "secondary_backend_only")
        );
    }

    #[cfg(feature = "precision-parser")]
    #[test]
    fn precision_comparison_reports_one_backend_parse_failure() {
        let mut differences = Vec::new();
        append_recovery_difference(0, 1, true, &mut differences);

        assert_eq!(differences.len(), 1);
        assert_eq!(differences[0].kind, "recovery_difference");
    }

    fn input(path: &str, content: &str, language_hint: &str) -> ExtractInput {
        ExtractInput {
            repo: RepoId("repo_test".to_string()),
            snapshot: SnapshotId("snap_test".to_string()),
            source: SourceFile {
                path: path.to_string(),
                language_hint: Some(language_hint.to_string()),
                content_hash: Some("hash".to_string()),
                content: Some(content.to_string()),
            },
        }
    }

    fn path_mapping<'a>(entity: &'a Entity, pattern: &str) -> &'a Value {
        let mappings = entity.payload["paths"].as_array().expect("paths array");
        for mapping in mappings {
            if mapping.get("pattern").and_then(Value::as_str) == Some(pattern) {
                return mapping;
            }
        }
        panic!("recorded path mapping for {pattern}")
    }

    #[test]
    fn supports_typescript_configuration_variants() {
        let extractor = JsTsExtractor;
        for path in ["tsconfig.json", "jsconfig.json", "packages/app/tsconfig.base.json"] {
            let source = input(path, "{}", "json").source;
            assert!(extractor.supports(&source), "{path} must be supported");
        }
        for path in ["tsconfig.jsonx", "notes.json", "src/app.ts"] {
            let source = input(path, "{}", "json").source;
            assert!(!extractor.supports(&source), "{path} must not be supported");
        }
    }

    #[tokio::test]
    async fn extracts_typescript_path_aliases_and_base_directory() {
        let output = JsTsExtractor
            .extract(input(
                "tsconfig.json",
                "{\n  // project roots\n  \"compilerOptions\": {\n    \"baseUrl\": \"./src\",\n    \"paths\": {\n      \"@/*\": [\"app/*\"],\n      \"config\": [\"./config.ts\"],\n    }\n  }\n}\n",
                "json",
            ))
            .await
            .unwrap();

        let config = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Feature)
            .expect("module resolution configuration");
        assert_eq!(config.stable_key.0, "config://tsconfig.json#js-ts-module-resolution");
        assert_eq!(config.payload["config_kind"], "js_ts_module_resolution");
        assert_eq!(config.payload["base_dir"], "src");
        assert!(config.payload["extends"].is_null());
        assert_eq!(config.source.as_ref().unwrap().line_start, Some(3));
        assert_eq!(config.payload["omitted_paths"], 0);
        assert_eq!(config.payload["omitted_targets"], 0);

        let alias = path_mapping(config, "@/*");
        assert_eq!(alias["targets"], json!(["app/*"]));
        assert_eq!(alias["line"], 6);
        let exact = path_mapping(config, "config");
        assert_eq!(exact["targets"], json!(["./config.ts"]));
        assert_eq!(exact["line"], 7);
        assert!(output.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn typescript_config_without_base_url_uses_its_own_directory() {
        let output = JsTsExtractor
            .extract(input(
                "packages/app/tsconfig.json",
                r#"{"compilerOptions":{"paths":{"~/*":["./src/*"]}}}"#,
                "json",
            ))
            .await
            .unwrap();

        let config = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Feature)
            .expect("module resolution configuration");
        assert_eq!(config.payload["base_dir"], "packages/app");
        assert_eq!(path_mapping(config, "~/*")["targets"], json!(["./src/*"]));
    }

    #[tokio::test]
    async fn unsupported_base_url_is_reported_without_a_base_directory() {
        let output = JsTsExtractor
            .extract(input(
                "tsconfig.json",
                r#"{"compilerOptions":{"baseUrl":"/absolute","paths":{"@/*":["src/*"]}}}"#,
                "json",
            ))
            .await
            .unwrap();

        let config = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Feature)
            .expect("module resolution configuration");
        assert!(config.payload["base_dir"].is_null());
        assert_eq!(config.payload["paths"].as_array().map(Vec::len), Some(1));
    }

    #[tokio::test]
    async fn inherited_extends_specifier_is_recorded() {
        let output = JsTsExtractor
            .extract(input(
                "tsconfig.json",
                r#"{"extends":"./tsconfig.base.json","compilerOptions":{"target":"es2022"}}"#,
                "json",
            ))
            .await
            .unwrap();

        let config = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Feature)
            .expect("module resolution configuration");
        assert_eq!(config.payload["extends"], "./tsconfig.base.json");
        assert_eq!(config.payload["base_dir"], "");
        assert_eq!(config.payload["paths"].as_array().map(Vec::len), Some(0));
    }

    #[tokio::test]
    async fn configuration_without_module_resolution_evidence_is_not_projected() {
        let output = JsTsExtractor
            .extract(input("tsconfig.json", r#"{"files":["src/main.ts"]}"#, "json"))
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());
        assert!(output.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn malformed_typescript_config_reports_a_parse_error() {
        let output = JsTsExtractor
            .extract(input("tsconfig.json", "{\"compilerOptions\":", "json"))
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics[0].kind,
            DiagnosticKind::Other("js_ts_typescript_config_parse_error".to_string())
        );
    }

    #[tokio::test]
    async fn comment_markers_inside_string_values_are_preserved() {
        let output = JsTsExtractor
            .extract(input(
                "tsconfig.json",
                "{\n  \"compilerOptions\": {\n    \"baseUrl\": \"./src/*not a comment*/pkg\",\n    \"paths\": {\n      \"@/*\": [\"//keep//\"]\n    }\n  }\n}\n",
                "json",
            ))
            .await
            .unwrap();

        let config = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Feature)
            .expect("module resolution configuration");
        assert_eq!(config.payload["base_dir"], "src/*not a comment*/pkg");
        assert_eq!(path_mapping(config, "@/*")["targets"], json!(["//keep//"]));
    }

    #[tokio::test]
    async fn path_mapping_bounds_are_disclosed() {
        let mut entries = Vec::new();
        for index in 0..70 {
            entries.push(format!(
                "\"@alias{index}/*\": [\"src{index}/*\", \"a\", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\", \"h\"]",
            ));
        }
        let content = format!(
            "{{\n  \"compilerOptions\": {{\n    \"paths\": {{\n      {}\n    }}\n  }}\n}}\n",
            entries.join(",\n      ")
        );
        let output = JsTsExtractor
            .extract(input("tsconfig.json", &content, "json"))
            .await
            .unwrap();

        let config = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Feature)
            .expect("module resolution configuration");
        assert_eq!(config.payload["paths"].as_array().map(Vec::len), Some(64));
        assert_eq!(config.payload["omitted_paths"], 6);
        assert_eq!(config.payload["omitted_targets"], 70);
    }

    #[tokio::test]
    async fn records_package_workspaces_and_entry_point() {
        let output = JsTsExtractor
            .extract(input(
                "package.json",
                r#"{"name":"root","private":true,"main":"src/index.ts","workspaces":{"packages":["packages/*","apps/*"]}}"#,
                "json",
            ))
            .await
            .unwrap();

        let package = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Package)
            .expect("package entity");
        assert_eq!(package.payload["main"], "src/index.ts");
        assert_eq!(package.payload["workspaces"], json!(["packages/*", "apps/*"]));
        assert_eq!(package.payload["omitted_workspaces"], 0);
    }

    #[tokio::test]
    async fn records_array_workspaces_and_rejects_non_string_patterns() {
        let output = JsTsExtractor
            .extract(input(
                "package.json",
                r#"{"name":"root","workspaces":["packages/*", 7]}"#,
                "json",
            ))
            .await
            .unwrap();

        let package = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Package)
            .expect("package entity");
        assert_eq!(package.payload["workspaces"], json!(["packages/*"]));
        assert_eq!(package.payload["omitted_workspaces"], 1);
    }
}
