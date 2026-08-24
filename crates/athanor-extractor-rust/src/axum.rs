use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use athanor_core::{CoreError, CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::json;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprMethodCall, Item, Lit, PathArguments, UseTree};

const AXUM_EXTRACTOR_NAME: &str = "axum";
const AXUM_ROUTE_ENTITY_KIND: &str = "axum_route";
const ROUTING_METHODS: [&str; 8] = [
    "get", "post", "put", "delete", "patch", "head", "options", "trace",
];

#[derive(Debug, Clone, Default)]
pub struct AxumExtractor;

#[async_trait]
impl Extractor for AxumExtractor {
    fn name(&self) -> &'static str {
        AXUM_EXTRACTOR_NAME
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
    }

    fn supports(&self, source: &SourceFile) -> bool {
        source.language_hint.as_deref() == Some("rust")
            && source
                .content
                .as_deref()
                .is_some_and(|content| content.contains("axum"))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };
        let syntax = syn::parse_file(content).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to parse Axum candidate Rust source {}: {error}",
                input.source.path
            ))
        })?;
        let imports = AxumRoutingImports::from_file(&syntax);
        let mut visitor = AxumRouteVisitor {
            imports: &imports,
            routes: Vec::new(),
        };
        visitor.visit_file(&syntax);

        let normalized_path = normalize_path(&input.source.path);
        let mut routes = BTreeMap::new();
        for route in visitor.routes {
            let stable_key = route_stable_key(&normalized_path, &route);
            routes.entry(stable_key).or_insert(route);
        }
        if routes.is_empty() {
            return Ok(ExtractOutput::default());
        }

        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let ownership = ownership_for_file(&input.source.path);
        let mut entities = Vec::with_capacity(routes.len());
        let mut facts = Vec::with_capacity(routes.len());

        for (stable_key_value, route) in routes {
            let stable_key = StableKey(stable_key_value);
            let entity_id = EntityId(format!(
                "ent_axum_route_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            ));
            entities.push(Entity {
                id: entity_id.clone(),
                stable_key: stable_key.clone(),
                kind: EntityKind::Other(AXUM_ROUTE_ENTITY_KIND.to_string()),
                name: format!("{} {}", route.method, route.path),
                title: Some(format!("Axum {} {}", route.method, route.path)),
                source: Some(SourceLocation {
                    path: input.source.path.clone(),
                    line_start: Some(route.line_start),
                    line_end: Some(route.line_end),
                }),
                language: Some(LanguageCode("rust".to_string())),
                aliases: Vec::new(),
                ownership: ownership.clone(),
                payload: json!({
                    "framework": "axum",
                    "method": route.method,
                    "path": route.path,
                    "handler": route.handler,
                    "source_path": normalized_path,
                }),
            });

            facts.push(Fact {
                id: FactId(format!(
                    "fact_axum_route_declared_{:016x}",
                    stable_hash(stable_key.0.as_bytes())
                )),
                kind: FactKind::RouteDeclared,
                subject: entity_id,
                object: Some(file_id.clone()),
                value: json!({
                    "stable_key": stable_key.0,
                    "framework": "axum",
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
struct AxumRoutingImports {
    direct_methods: BTreeMap<String, String>,
    routing_namespaces: BTreeSet<String>,
}

impl AxumRoutingImports {
    fn from_file(file: &syn::File) -> Self {
        let mut imports = Self::default();
        for item in &file.items {
            let Item::Use(item_use) = item else {
                continue;
            };
            let mut prefix = Vec::new();
            collect_axum_use_tree(&item_use.tree, &mut prefix, &mut imports);
        }
        imports
    }

    fn resolve_method_path(&self, path: &syn::Path) -> Option<&'static str> {
        if path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
        {
            return None;
        }
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();

        match segments.as_slice() {
            [name] => self
                .direct_methods
                .get(name)
                .and_then(|method| canonical_http_method(method)),
            [namespace, method] if self.routing_namespaces.contains(namespace) => {
                canonical_http_method(method)
            }
            [axum, routing, method] if axum == "axum" && routing == "routing" => {
                canonical_http_method(method)
            }
            _ => None,
        }
    }
}

fn collect_axum_use_tree(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    imports: &mut AxumRoutingImports,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_axum_use_tree(&path.tree, prefix, imports);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut full = prefix.clone();
            full.push(name.ident.to_string());
            if full.as_slice() == ["axum", "routing"] {
                imports.routing_namespaces.insert("routing".to_string());
            } else if let [axum, routing, method] = full.as_slice()
                && axum == "axum"
                && routing == "routing"
                && canonical_http_method(method).is_some()
            {
                imports
                    .direct_methods
                    .insert(method.clone(), method.clone());
            }
        }
        UseTree::Rename(rename) => {
            let mut full = prefix.clone();
            full.push(rename.ident.to_string());
            if full.as_slice() == ["axum", "routing"] {
                imports
                    .routing_namespaces
                    .insert(rename.rename.to_string());
            } else if let [axum, routing, method] = full.as_slice()
                && axum == "axum"
                && routing == "routing"
                && canonical_http_method(method).is_some()
            {
                imports
                    .direct_methods
                    .insert(rename.rename.to_string(), method.clone());
            }
        }
        UseTree::Glob(_) => {
            if prefix.as_slice() == ["axum", "routing"] {
                for method in ROUTING_METHODS {
                    imports
                        .direct_methods
                        .insert(method.to_string(), method.to_string());
                }
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_axum_use_tree(item, prefix, imports);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AxumRoute {
    method: String,
    path: String,
    handler: String,
    line_start: u32,
    line_end: u32,
}

struct AxumRouteVisitor<'a> {
    imports: &'a AxumRoutingImports,
    routes: Vec<AxumRoute>,
}

impl<'ast> Visit<'ast> for AxumRouteVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if node.method == "route"
            && let Some(route) = route_from_method_call(node, self.imports)
        {
            self.routes.push(route);
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn route_from_method_call(
    node: &ExprMethodCall,
    imports: &AxumRoutingImports,
) -> Option<AxumRoute> {
    if node.args.len() != 2 {
        return None;
    }
    let mut args = node.args.iter();
    let path = match args.next()? {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Str(path) => path.value(),
            _ => return None,
        },
        _ => return None,
    };
    let (method, handler) = method_router_call(args.next()?, imports)?;
    let start = node.span().start().line.max(1) as u32;
    let end = node.span().end().line.max(start as usize) as u32;
    Some(AxumRoute {
        method: method.to_string(),
        path,
        handler,
        line_start: start,
        line_end: end,
    })
}

fn method_router_call(
    expression: &Expr,
    imports: &AxumRoutingImports,
) -> Option<(&'static str, String)> {
    let Expr::Call(ExprCall { func, args, .. }) = expression else {
        return None;
    };
    if args.len() != 1 {
        return None;
    }
    let Expr::Path(method_path) = func.as_ref() else {
        return None;
    };
    if method_path.qself.is_some() {
        return None;
    }
    let method = imports.resolve_method_path(&method_path.path)?;
    let handler = handler_path(args.first()?)?;
    Some((method, handler))
}

fn handler_path(expression: &Expr) -> Option<String> {
    let Expr::Path(path) = expression else {
        return None;
    };
    if path.qself.is_some()
        || path
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return None;
    }
    Some(
        path.path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
    )
}

fn canonical_http_method(method: &str) -> Option<&'static str> {
    match method {
        "get" => Some("GET"),
        "post" => Some("POST"),
        "put" => Some("PUT"),
        "delete" => Some("DELETE"),
        "patch" => Some("PATCH"),
        "head" => Some("HEAD"),
        "options" => Some("OPTIONS"),
        "trace" => Some("TRACE"),
        _ => None,
    }
}

fn route_stable_key(source_path: &str, route: &AxumRoute) -> String {
    format!(
        "axum-route://{source_path}#{}:{}:{}",
        route.method, route.path, route.handler
    )
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    fn source(path: &str, content: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            language_hint: Some("rust".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some(content.to_string()),
        }
    }

    async fn extract(path: &str, content: &str) -> ExtractOutput {
        AxumExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: source(path, content),
            })
            .await
            .unwrap()
    }

    #[test]
    fn supports_only_rust_sources_that_reference_axum() {
        let extractor = AxumExtractor;
        assert!(extractor.supports(&source(
            "src/routes.rs",
            "use axum::{Router, routing::get};"
        )));
        assert!(!extractor.supports(&source(
            "src/routes.rs",
            "fn route() {}"
        )));
        let mut javascript = source("src/routes.js", "import axum from 'axum';");
        javascript.language_hint = Some("javascript".to_string());
        assert!(!extractor.supports(&javascript));
    }

    #[tokio::test]
    async fn extracts_static_routes_from_direct_axum_routing_imports() {
        let output = extract(
            "src/routes.rs",
            r#"
use axum::{Router, routing::{get, post}};

async fn list_users() {}
async fn create_user() {}

fn router() -> Router {
    Router::new()
        .route("/users", get(list_users))
        .route("/users", post(create_user))
}
"#,
        )
        .await;

        assert_eq!(output.entities.len(), 2);
        assert_eq!(output.facts.len(), 2);
        assert_eq!(
            output.entities[0].kind,
            EntityKind::Other(AXUM_ROUTE_ENTITY_KIND.to_string())
        );
        assert_eq!(output.entities[0].payload["framework"], json!("axum"));
        assert_eq!(output.entities[0].payload["method"], json!("GET"));
        assert_eq!(output.entities[0].payload["path"], json!("/users"));
        assert_eq!(output.entities[0].payload["handler"], json!("list_users"));
        assert_eq!(output.facts[0].kind, FactKind::RouteDeclared);
        assert_eq!(output.facts[0].extractor, AXUM_EXTRACTOR_NAME);
        assert!(!output.facts[0].evidence.is_empty());
        assert!(!output.facts[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn supports_alias_namespace_and_fully_qualified_routing_methods() {
        for (content, expected_method) in [
            (
                "use axum::routing::get as axum_get; fn r() { app.route(\"/health\", axum_get(health)); }",
                "GET",
            ),
            (
                "use axum::routing as ar; fn r() { app.route(\"/items\", ar::put(update)); }",
                "PUT",
            ),
            (
                "fn r() { axum::Router::new().route(\"/audit\", axum::routing::delete(remove)); }",
                "DELETE",
            ),
        ] {
            let output = extract("src/routes.rs", content).await;
            assert_eq!(output.entities.len(), 1);
            assert_eq!(output.entities[0].payload["method"], json!(expected_method));
        }
    }

    #[tokio::test]
    async fn rejects_dynamic_paths_chained_method_routers_and_non_axum_get_helpers() {
        for content in [
            "use axum::routing::get; fn r() { app.route(path, get(handler)); }",
            "use axum::routing::{get, post}; fn r() { app.route(\"/users\", get(list).post(create)); }",
            "use axum::Router; use other::get; fn r() { app.route(\"/users\", get(list)); }",
            "use axum::routing::get; fn r() { app.route(\"/users\", get(|| async {})); }",
        ] {
            let output = extract("src/routes.rs", content).await;
            assert!(output.entities.is_empty());
            assert!(output.facts.is_empty());
        }
    }

    #[tokio::test]
    async fn normalizes_windows_source_paths_in_stable_keys_and_payloads() {
        let output = extract(
            "src\\http\\routes.rs",
            "use axum::routing::get; fn r() { app.route(\"/health\", get(health)); }",
        )
        .await;
        assert_eq!(output.entities.len(), 1);
        assert_eq!(
            output.entities[0].stable_key.0,
            "axum-route://src/http/routes.rs#GET:/health:health"
        );
        assert_eq!(
            output.entities[0].payload["source_path"],
            json!("src/http/routes.rs")
        );
    }

    #[tokio::test]
    async fn deduplicates_identical_static_route_declarations() {
        let output = extract(
            "src/routes.rs",
            r#"
use axum::routing::get;
fn r() {
    app.route("/health", get(health));
    app.route("/health", get(health));
}
"#,
        )
        .await;
        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.facts.len(), 1);
    }
}
