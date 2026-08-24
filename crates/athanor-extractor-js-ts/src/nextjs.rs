use async_trait::async_trait;
use athanor_core::{CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile};
use athanor_domain::{
    Entity, EntityId, EntityKind, Fact, FactId, FactKind, LanguageCode, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::json;

const NEXTJS_EXTRACTOR_NAME: &str = "nextjs";

#[derive(Debug, Clone, Default)]
pub struct NextJsExtractor;

#[async_trait]
impl Extractor for NextJsExtractor {
    fn name(&self) -> &'static str {
        NEXTJS_EXTRACTOR_NAME
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
    }

    fn supports(&self, source: &SourceFile) -> bool {
        next_route(&source.path).is_some()
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(route) = next_route(&input.source.path) else {
            return Ok(ExtractOutput::default());
        };
        let normalized_path = normalize_path(&input.source.path);
        let stable_key = StableKey(format!("feature://nextjs:{normalized_path}#route"));
        let entity_id = EntityId(format!(
            "ent_nextjs_route_{:016x}",
            stable_hash(stable_key.0.as_bytes())
        ));
        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let ownership = ownership_for_file(&input.source.path);
        let line_end = input
            .source
            .content
            .as_deref()
            .map(|content| content.lines().count().max(1) as u32)
            .unwrap_or(1);

        let entity = Entity {
            id: entity_id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::Feature,
            name: route.route.clone(),
            title: Some(format!("Next.js {} {}", route.kind.label(), route.route)),
            source: Some(SourceLocation {
                path: input.source.path.clone(),
                line_start: Some(1),
                line_end: Some(line_end),
            }),
            language: Some(LanguageCode("nextjs".to_string())),
            aliases: Vec::new(),
            ownership: ownership.clone(),
            payload: json!({
                "feature_kind": "nextjs_route",
                "framework": "nextjs",
                "router": route.router.as_str(),
                "route_kind": route.kind.as_str(),
                "route": route.route,
                "dynamic": route.dynamic,
                "source_path": normalized_path,
            }),
        };

        let fact = Fact {
            id: FactId(format!(
                "fact_nextjs_route_declared_{:016x}",
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::RouteDeclared,
            subject: entity_id,
            object: Some(file_id),
            value: json!({
                "stable_key": stable_key.0,
                "framework": "nextjs",
                "router": route.router.as_str(),
                "route_kind": route.kind.as_str(),
                "route": route.route,
                "dynamic": route.dynamic,
                "source_path": normalized_path,
            }),
            evidence: vec![evidence_for_file(
                &input.source.path,
                self.name(),
                Some(1),
                Some(line_end),
            )],
            ownership,
            snapshot: input.snapshot,
            extractor: self.name().to_string(),
            confidence: 1.0,
        };

        Ok(ExtractOutput {
            entities: vec![entity],
            facts: vec![fact],
            diagnostics: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NextRouter {
    App,
    Pages,
}

impl NextRouter {
    fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Pages => "pages",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NextRouteKind {
    Page,
    RouteHandler,
    ApiPage,
}

impl NextRouteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::RouteHandler => "route_handler",
            Self::ApiPage => "api_page",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::RouteHandler => "route handler",
            Self::ApiPage => "API page",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NextRoute {
    router: NextRouter,
    kind: NextRouteKind,
    route: String,
    dynamic: bool,
}

fn next_route(path: &str) -> Option<NextRoute> {
    let normalized = normalize_path(path);
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let (router, prefix_len) = match parts.as_slice() {
        ["app", ..] => (NextRouter::App, 1),
        ["src", "app", ..] => (NextRouter::App, 2),
        ["pages", ..] => (NextRouter::Pages, 1),
        ["src", "pages", ..] => (NextRouter::Pages, 2),
        _ => return None,
    };
    if parts.len() <= prefix_len {
        return None;
    }

    let file = parts.last().copied()?;
    let (stem, extension) = split_extension(file)?;
    if !is_next_source_extension(extension) {
        return None;
    }

    match router {
        NextRouter::App => app_route(&parts[prefix_len..parts.len() - 1], stem),
        NextRouter::Pages => pages_route(&parts[prefix_len..parts.len() - 1], stem),
    }
}

fn app_route(directories: &[&str], stem: &str) -> Option<NextRoute> {
    let kind = match stem {
        "page" => NextRouteKind::Page,
        "route" => NextRouteKind::RouteHandler,
        _ => return None,
    };
    let mut segments = Vec::new();
    for segment in directories {
        if segment.starts_with('_') {
            return None;
        }
        if segment.starts_with('@') {
            continue;
        }
        if segment.starts_with('(') {
            if segment.ends_with(')') && segment.len() > 2 {
                continue;
            }
            return None;
        }
        segments.push(*segment);
    }
    let route = route_path(&segments);
    Some(NextRoute {
        router: NextRouter::App,
        kind,
        dynamic: route.contains('['),
        route,
    })
}

fn pages_route(directories: &[&str], stem: &str) -> Option<NextRoute> {
    if matches!(stem, "_app" | "_document" | "_error") {
        return None;
    }
    let mut segments = directories.to_vec();
    if stem != "index" {
        segments.push(stem);
    }
    let kind = if segments.first().copied() == Some("api") {
        NextRouteKind::ApiPage
    } else {
        NextRouteKind::Page
    };
    let route = route_path(&segments);
    Some(NextRoute {
        router: NextRouter::Pages,
        kind,
        dynamic: route.contains('['),
        route,
    })
}

fn route_path(segments: &[&str]) -> String {
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn split_extension(file: &str) -> Option<(&str, &str)> {
    let (stem, extension) = file.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty()).then_some((stem, extension))
}

fn is_next_source_extension(extension: &str) -> bool {
    matches!(extension.to_ascii_lowercase().as_str(), "js" | "jsx" | "ts" | "tsx")
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    fn source(path: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            language_hint: Some("typescriptreact".to_string()),
            content_hash: Some("hash".to_string()),
            content: Some("export default function Page() { return null; }\n".to_string()),
        }
    }

    async fn extract(path: &str) -> ExtractOutput {
        NextJsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: source(path),
            })
            .await
            .unwrap()
    }

    #[test]
    fn supports_only_bounded_nextjs_route_conventions() {
        let extractor = NextJsExtractor;
        for path in [
            "app/page.tsx",
            "src/app/api/users/route.ts",
            "pages/index.tsx",
            "src/pages/blog/[slug].tsx",
            "pages/api/health.ts",
        ] {
            assert!(extractor.supports(&source(path)), "did not support {path}");
        }
        for path in [
            "src/components/Button.tsx",
            "next.config.mjs",
            "app/layout.tsx",
            "app/_private/page.tsx",
            "app/(.)photo/page.tsx",
            "pages/_app.tsx",
        ] {
            assert!(!extractor.supports(&source(path)), "unexpectedly supported {path}");
        }
    }

    #[tokio::test]
    async fn extracts_app_router_page_with_groups_and_dynamic_segments() {
        let output = extract("src/app/(marketing)/products/[id]/page.tsx").await;
        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.facts.len(), 1);
        let entity = &output.entities[0];
        assert_eq!(entity.kind, EntityKind::Feature);
        assert_eq!(entity.payload["framework"], json!("nextjs"));
        assert_eq!(entity.payload["router"], json!("app"));
        assert_eq!(entity.payload["route_kind"], json!("page"));
        assert_eq!(entity.payload["route"], json!("/products/[id]"));
        assert_eq!(entity.payload["dynamic"], json!(true));
        assert_eq!(output.facts[0].kind, FactKind::RouteDeclared);
        assert_eq!(output.facts[0].extractor, NEXTJS_EXTRACTOR_NAME);
        assert!(
            output.facts[0]
                .object
                .as_ref()
                .unwrap()
                .0
                .starts_with("ent_file_")
        );
        assert!(!output.facts[0].evidence.is_empty());
        assert!(!output.facts[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn extracts_app_route_handler_without_inferring_http_methods() {
        let output = extract("app/api/users/route.ts").await;
        let entity = &output.entities[0];
        assert_eq!(entity.payload["route"], json!("/api/users"));
        assert_eq!(entity.payload["route_kind"], json!("route_handler"));
        assert!(entity.payload.get("methods").is_none());
    }

    #[tokio::test]
    async fn extracts_pages_router_index_dynamic_and_api_routes() {
        for (path, expected_route, expected_kind) in [
            ("pages/index.tsx", "/", "page"),
            ("pages/blog/[slug].tsx", "/blog/[slug]", "page"),
            ("src/pages/api/health.ts", "/api/health", "api_page"),
        ] {
            let output = extract(path).await;
            let entity = &output.entities[0];
            assert_eq!(entity.payload["route"], json!(expected_route));
            assert_eq!(entity.payload["route_kind"], json!(expected_kind));
        }
    }

    #[tokio::test]
    async fn normalizes_windows_paths_in_stable_keys_and_payloads() {
        let output = extract("src\\app\\dashboard\\page.tsx").await;
        let entity = &output.entities[0];
        assert_eq!(entity.payload["route"], json!("/dashboard"));
        assert_eq!(entity.payload["source_path"], json!("src/app/dashboard/page.tsx"));
        assert_eq!(
            entity.stable_key.0,
            "feature://nextjs:src/app/dashboard/page.tsx#route"
        );
    }

    #[tokio::test]
    async fn route_inventory_does_not_require_source_content() {
        let output = NextJsExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    content: None,
                    ..source("app/page.tsx")
                },
            })
            .await
            .unwrap();
        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.facts.len(), 1);
        assert_eq!(output.entities[0].source.as_ref().unwrap().line_end, Some(1));
    }
}
