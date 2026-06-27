use std::borrow::Cow;

use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, SourceFile};
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

    fn supports(&self, source: &SourceFile) -> bool {
        js_ts_language(source).is_some() || is_package_json(source)
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };

        if is_package_json(&input.source) {
            return Ok(extract_package_json(self.name(), &input, content));
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
            payload: json!({
                "language_hint": input.source.language_hint,
                "parser": language.parser_name,
                "imports": imports,
                "exports": exports,
                "declaration_count": declarations.len(),
            }),
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
                payload: json!({
                    "symbol_kind": declaration.symbol_kind,
                    "qualified_name": declaration.qualified_name,
                    "exported": declaration.exported,
                    "async": declaration.is_async,
                    "source_node": declaration.source_node,
                }),
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
                });
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            collect_variable_functions(node, bytes, exported, declarations);
        }
        _ => {}
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
            });
        }
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
    json!({
        "source": child_text(node, "source", bytes),
        "line_start": node.start_position().row as u32 + 1,
        "line_end": node.end_position().row as u32 + 1,
    })
}

fn export_payload(node: Node, bytes: &[u8]) -> Value {
    json!({
        "source": child_text(node, "source", bytes),
        "line_start": node.start_position().row as u32 + 1,
        "line_end": node.end_position().row as u32 + 1,
    })
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
}
