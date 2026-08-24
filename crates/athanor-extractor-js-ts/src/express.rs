use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::json;
use tree_sitter::{Node, Parser};

use super::{ParserLanguage, js_ts_language, normalized_parse_content};

const EXPRESS_EXTRACTOR_NAME: &str = "express";
const EXPRESS_ROUTE_ENTITY_KIND: &str = "express_route";
const ROUTE_METHODS: [&str; 7] = ["get", "post", "put", "delete", "patch", "head", "options"];

#[derive(Debug, Clone, Default)]
pub struct ExpressExtractor;

#[async_trait]
impl Extractor for ExpressExtractor {
    fn name(&self) -> &'static str {
        EXPRESS_EXTRACTOR_NAME
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
    }

    fn supports(&self, source: &SourceFile) -> bool {
        js_ts_language(source).is_some()
            && source
                .content
                .as_deref()
                .is_some_and(|content| content.contains("express"))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let Some(language) = js_ts_language(&input.source) else {
            return Ok(ExtractOutput::default());
        };

        let mut parser = Parser::new();
        match language.parser {
            ParserLanguage::Javascript => parser
                .set_language(&tree_sitter_javascript::LANGUAGE.into())
                .map_err(|error| athanor_core::CoreError::Adapter(error.to_string()))?,
            ParserLanguage::Typescript => parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                .map_err(|error| athanor_core::CoreError::Adapter(error.to_string()))?,
            ParserLanguage::Tsx => parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                .map_err(|error| athanor_core::CoreError::Adapter(error.to_string()))?,
        }

        let parse_content = normalized_parse_content(content);
        let bytes = parse_content.as_bytes();
        let Some(tree) = parser.parse(bytes, None) else {
            return Ok(ExtractOutput::default());
        };
        let root = tree.root_node();
        if root.has_error() {
            return Ok(ExtractOutput::default());
        }

        let mut bindings = ExpressBindings::default();
        collect_import_bindings(root, bytes, &mut bindings);
        if bindings.is_empty() {
            return Ok(ExtractOutput::default());
        }
        collect_route_receivers(root, bytes, &mut bindings);
        if bindings.route_receivers.is_empty() {
            return Ok(ExtractOutput::default());
        }

        let mut routes = BTreeMap::new();
        collect_routes(root, bytes, &bindings, &mut routes);
        if routes.is_empty() {
            return Ok(ExtractOutput::default());
        }

        let normalized_path = normalize_path(&input.source.path);
        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let ownership = ownership_for_file(&input.source.path);
        let mut entities = Vec::with_capacity(routes.len());
        let mut facts = Vec::with_capacity(routes.len());

        for route in routes.into_values() {
            let stable_key = StableKey(route_stable_key(&normalized_path, &route));
            let entity_id = EntityId(format!(
                "ent_express_route_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            ));
            entities.push(Entity {
                id: entity_id.clone(),
                stable_key: stable_key.clone(),
                kind: EntityKind::Other(EXPRESS_ROUTE_ENTITY_KIND.to_string()),
                name: format!("{} {}", route.method, route.path),
                title: Some(format!("Express {} {}", route.method, route.path)),
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(route.line_start),
                    line_end: Some(route.line_end),
                }),
                language: Some(LanguageCode(language.hint.to_string())),
                aliases: Vec::new(),
                ownership: ownership.clone(),
                payload: json!({
                    "framework": "express",
                    "receiver": route.receiver,
                    "method": route.method,
                    "path": route.path,
                    "handler": route.handler,
                    "source_path": normalized_path,
                }),
            });
            facts.push(Fact {
                id: FactId(format!(
                    "fact_express_route_declared_{:016x}",
                    stable_hash(stable_key.0.as_bytes())
                )),
                kind: FactKind::RouteDeclared,
                subject: entity_id,
                object: Some(file_id.clone()),
                value: json!({
                    "stable_key": stable_key.0,
                    "framework": "express",
                    "receiver": route.receiver,
                    "method": route.method,
                    "path": route.path,
                    "handler": route.handler,
                    "source_path": normalized_path,
                }),
                evidence: vec![evidence_for_file(
                    &input.source.path,
                    self.name(),
                    Some(route.line_start),
                    Some(route.line_end),
                )],
                ownership: ownership.clone(),
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

#[derive(Debug, Default)]
struct ExpressBindings {
    express_factories: BTreeSet<String>,
    express_namespaces: BTreeSet<String>,
    router_factories: BTreeSet<String>,
    route_receivers: BTreeSet<String>,
}

impl ExpressBindings {
    fn is_empty(&self) -> bool {
        self.express_factories.is_empty()
            && self.express_namespaces.is_empty()
            && self.router_factories.is_empty()
    }
}

fn collect_import_bindings(node: Node<'_>, bytes: &[u8], bindings: &mut ExpressBindings) {
    if node.kind() == "import_statement"
        && child_text(node, "source", bytes).as_deref() == Some("express")
    {
        if let Ok(text) = node.utf8_text(bytes) {
            parse_esm_import(text, bindings);
        }
    } else if node.kind() == "variable_declarator" {
        collect_commonjs_binding(node, bytes, bindings);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_import_bindings(child, bytes, bindings);
    }
}

fn parse_esm_import(text: &str, bindings: &mut ExpressBindings) {
    let Some(body) = text.trim().strip_prefix("import ") else {
        return;
    };
    let Some((binding_text, _)) = body.split_once(" from ") else {
        return;
    };
    let binding_text = binding_text.trim();

    if let Some(namespace) = binding_text.strip_prefix("* as ").map(str::trim) {
        if is_identifier(namespace) {
            bindings.express_namespaces.insert(namespace.to_string());
        }
        return;
    }

    let (default_binding, named_binding) = if binding_text.starts_with('{') {
        (None, Some(binding_text))
    } else if let Some((default, named)) = binding_text.split_once(',') {
        (Some(default.trim()), Some(named.trim()))
    } else {
        (Some(binding_text), None)
    };

    if let Some(default_binding) = default_binding.filter(|name| is_identifier(name)) {
        bindings
            .express_factories
            .insert(default_binding.to_string());
        bindings
            .express_namespaces
            .insert(default_binding.to_string());
    }
    if let Some(named_binding) = named_binding {
        parse_named_router_binding(named_binding, bindings);
    }
}

fn parse_named_router_binding(text: &str, bindings: &mut ExpressBindings) {
    let Some(inner) = text.trim().strip_prefix('{').and_then(|value| value.strip_suffix('}')) else {
        return;
    };
    for item in inner.split(',') {
        let item = item.trim();
        let (name, alias) = item
            .split_once(" as ")
            .map_or((item, item), |(name, alias)| (name.trim(), alias.trim()));
        if name == "Router" && is_identifier(alias) {
            bindings.router_factories.insert(alias.to_string());
        }
    }
}

fn collect_commonjs_binding(node: Node<'_>, bytes: &[u8], bindings: &mut ExpressBindings) {
    let Some(name) = node.child_by_field_name("name") else {
        return;
    };
    let Some(value) = node.child_by_field_name("value") else {
        return;
    };
    if !is_require_express(value, bytes) {
        return;
    }
    let Ok(name_text) = name.utf8_text(bytes) else {
        return;
    };
    let name_text = name_text.trim();
    if is_identifier(name_text) {
        bindings.express_factories.insert(name_text.to_string());
        bindings.express_namespaces.insert(name_text.to_string());
    } else {
        parse_commonjs_router_pattern(name_text, bindings);
    }
}

fn parse_commonjs_router_pattern(text: &str, bindings: &mut ExpressBindings) {
    let Some(inner) = text.strip_prefix('{').and_then(|value| value.strip_suffix('}')) else {
        return;
    };
    for item in inner.split(',') {
        let item = item.trim();
        let (name, alias) = item
            .split_once(':')
            .map_or((item, item), |(name, alias)| (name.trim(), alias.trim()));
        if name == "Router" && is_identifier(alias) {
            bindings.router_factories.insert(alias.to_string());
        }
    }
}

fn is_require_express(node: Node<'_>, bytes: &[u8]) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let Some(function) = node.child_by_field_name("function") else {
        return false;
    };
    if function.utf8_text(bytes).ok() != Some("require") {
        return false;
    }
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return false;
    };
    let mut cursor = arguments.walk();
    let args = arguments.named_children(&mut cursor).collect::<Vec<_>>();
    args.len() == 1 && string_literal(args[0], bytes).as_deref() == Some("express")
}

fn collect_route_receivers(node: Node<'_>, bytes: &[u8], bindings: &mut ExpressBindings) {
    if node.kind() == "variable_declarator" {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let Some(value_node) = node.child_by_field_name("value") else {
            return;
        };
        let Ok(name) = name_node.utf8_text(bytes) else {
            return;
        };
        let name = name.trim();
        if is_identifier(name) && is_route_receiver_factory(value_node, bytes, bindings) {
            bindings.route_receivers.insert(name.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_route_receivers(child, bytes, bindings);
    }
}

fn is_route_receiver_factory(node: Node<'_>, bytes: &[u8], bindings: &ExpressBindings) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let Some(function) = node.child_by_field_name("function") else {
        return false;
    };
    match function.kind() {
        "identifier" => function.utf8_text(bytes).ok().is_some_and(|name| {
            bindings.express_factories.contains(name) || bindings.router_factories.contains(name)
        }),
        "member_expression" => member_parts(function, bytes).is_some_and(|(object, property)| {
            property == "Router" && bindings.express_namespaces.contains(&object)
        }),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressRoute {
    receiver: String,
    method: String,
    path: String,
    handler: String,
    line_start: u32,
    line_end: u32,
}

fn collect_routes(
    node: Node<'_>,
    bytes: &[u8],
    bindings: &ExpressBindings,
    routes: &mut BTreeMap<String, ExpressRoute>,
) {
    if node.kind() == "call_expression"
        && let Some(route) = route_from_call(node, bytes, bindings)
    {
        let key = format!(
            "{}:{}:{}:{}",
            route.receiver, route.method, route.path, route.handler
        );
        routes.entry(key).or_insert(route);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_routes(child, bytes, bindings, routes);
    }
}

fn route_from_call(
    node: Node<'_>,
    bytes: &[u8],
    bindings: &ExpressBindings,
) -> Option<ExpressRoute> {
    let function = node.child_by_field_name("function")?;
    if function.kind() != "member_expression" {
        return None;
    }
    let (receiver, method) = member_parts(function, bytes)?;
    if !bindings.route_receivers.contains(&receiver) || !ROUTE_METHODS.contains(&method.as_str()) {
        return None;
    }
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let args = arguments.named_children(&mut cursor).collect::<Vec<_>>();
    if args.len() != 2 {
        return None;
    }
    let path = string_literal(args[0], bytes)?;
    if !path.starts_with('/') {
        return None;
    }
    let handler = handler_path(args[1], bytes)?;
    Some(ExpressRoute {
        receiver,
        method: method.to_ascii_uppercase(),
        path,
        handler,
        line_start: node.start_position().row as u32 + 1,
        line_end: node.end_position().row as u32 + 1,
    })
}

fn member_parts(node: Node<'_>, bytes: &[u8]) -> Option<(String, String)> {
    let object = node.child_by_field_name("object")?;
    let property = node.child_by_field_name("property")?;
    if object.kind() != "identifier" || !matches!(property.kind(), "property_identifier" | "identifier") {
        return None;
    }
    Some((
        object.utf8_text(bytes).ok()?.to_string(),
        property.utf8_text(bytes).ok()?.to_string(),
    ))
}

fn handler_path(node: Node<'_>, bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => node.utf8_text(bytes).ok().map(ToString::to_string),
        "member_expression" => {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;
            if !matches!(property.kind(), "property_identifier" | "identifier") {
                return None;
            }
            let object = handler_path(object, bytes)?;
            let property = property.utf8_text(bytes).ok()?;
            Some(format!("{object}.{property}"))
        }
        _ => None,
    }
}

fn string_literal(node: Node<'_>, bytes: &[u8]) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }
    node.utf8_text(bytes)
        .ok()
        .map(|value| value.trim_matches(['\'', '"']).to_string())
}

fn child_text(node: Node<'_>, field: &str, bytes: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(bytes).ok())
        .map(|value| value.trim_matches(['\'', '"']).to_string())
}

fn route_stable_key(source_path: &str, route: &ExpressRoute) -> String {
    format!(
        "express-route://{source_path}#{}:{}:{}:{}",
        route.receiver, route.method, route.path, route.handler
    )
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    fn source(path: &str, language: &str, content: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            language_hint: Some(language.to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(content.to_string()),
        }
    }

    async fn extract(path: &str, language: &str, content: &str) -> ExtractOutput {
        ExpressExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: source(path, language, content),
            })
            .await
            .unwrap()
    }

    #[test]
    fn supports_only_js_ts_sources_that_reference_express() {
        let extractor = ExpressExtractor;
        assert!(extractor.supports(&source(
            "src/server.ts",
            "typescript",
            "import express from 'express';"
        )));
        assert!(!extractor.supports(&source(
            "src/server.ts",
            "typescript",
            "export const value = 1;"
        )));
        assert!(!extractor.supports(&source(
            "src/server.rs",
            "rust",
            "// express"
        )));
    }

    #[tokio::test]
    async fn extracts_static_routes_from_default_import_application() {
        let output = extract(
            "src/server.ts",
            "typescript",
            r#"
import express from "express";
const app = express();
app.get("/users", handlers.list);
app.post("/users", createUser);
"#,
        )
        .await;

        assert_eq!(output.entities.len(), 2);
        assert_eq!(output.facts.len(), 2);
        assert_eq!(
            output.entities[0].kind,
            EntityKind::Other(EXPRESS_ROUTE_ENTITY_KIND.to_string())
        );
        assert_eq!(output.entities[0].payload["framework"], json!("express"));
        assert_eq!(output.facts[0].kind, FactKind::RouteDeclared);
        assert_eq!(output.facts[0].extractor, EXPRESS_EXTRACTOR_NAME);
        assert!(!output.facts[0].evidence.is_empty());
        assert!(!output.facts[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn supports_named_router_alias_and_commonjs_factory() {
        for (content, expected_receiver) in [
            (
                "import { Router as ExpressRouter } from 'express'; const api = ExpressRouter(); api.put('/items', update);",
                "api",
            ),
            (
                "const express = require('express'); const app = express(); app.delete('/items/:id', remove);",
                "app",
            ),
            (
                "const { Router: MakeRouter } = require('express'); const router = MakeRouter(); router.patch('/items/:id', patchItem);",
                "router",
            ),
        ] {
            let output = extract("src/routes.js", "javascript", content).await;
            assert_eq!(output.entities.len(), 1);
            assert_eq!(
                output.entities[0].payload["receiver"],
                json!(expected_receiver)
            );
        }
    }

    #[tokio::test]
    async fn rejects_dynamic_paths_extra_middleware_and_non_express_receivers() {
        for content in [
            "import express from 'express'; const app = express(); app.get(path, handler);",
            "import express from 'express'; const app = express(); app.get('/x', auth, handler);",
            "import express from 'express'; const app = express(); other.get('/x', handler);",
            "import express from 'express'; const app = express(); app.get('/x', () => ok);",
            "const app = fakeExpress(); app.get('/x', handler);",
        ] {
            let output = extract("src/server.ts", "typescript", content).await;
            assert!(output.entities.is_empty());
            assert!(output.facts.is_empty());
        }
    }

    #[tokio::test]
    async fn normalizes_windows_source_paths_in_stable_keys_and_payloads() {
        let output = extract(
            "src\\http\\server.ts",
            "typescript",
            "import express from 'express'; const app = express(); app.head('/health', health);",
        )
        .await;
        assert_eq!(output.entities.len(), 1);
        assert_eq!(
            output.entities[0].stable_key.0,
            "express-route://src/http/server.ts#app:HEAD:/health:health"
        );
        assert_eq!(
            output.entities[0].payload["source_path"],
            json!("src/http/server.ts")
        );
    }

    #[tokio::test]
    async fn deduplicates_identical_static_route_declarations() {
        let output = extract(
            "src/server.js",
            "javascript",
            r#"
const express = require("express");
const app = express();
app.get("/health", health);
app.get("/health", health);
"#,
        )
        .await;
        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.facts.len(), 1);
    }
}
