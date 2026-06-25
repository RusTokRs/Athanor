use std::collections::{BTreeMap, BTreeSet, HashMap};

use async_trait::async_trait;
use athanor_core::{
    CheckInput, Checker, CoreResult, ExtractInput, ExtractOutput, Extractor, LinkInput, Linker,
    SourceFile,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, Fact, FactId, FactKind, LanguageCode, Relation, RelationId, RelationKind,
    RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const FFA_EXTRACTOR_ID: &str = "rustok_ffa";
pub const FFA_LINKER_ID: &str = "rustok_ffa_linker";
pub const FFA_CHECKER_ID: &str = "rustok_ffa_checker";
pub const FFA_MARKER_FACT_KIND: &str = "rustok_ffa_source_marker";
pub const FFA_LAYER_RELATION_KIND: &str = "rustok_ffa_implemented_by";
pub const FFA_SURFACE_ENTITY_KIND: &str = "rustok_ffa_surface";
pub const FFA_LAYER_ENTITY_KIND: &str = "rustok_ffa_layer";

#[derive(Debug, Clone, Default)]
pub struct RustokFfaExtractor;

#[derive(Debug, Clone, Default)]
pub struct RustokFfaLinker;

#[derive(Debug, Clone, Default)]
pub struct RustokFfaChecker;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceMarker {
    schema: String,
    path: String,
    module: String,
    surface: String,
    role: String,
    canonical_ui_adapter: bool,
    host_wiring: bool,
    has_leptos_import: bool,
    has_component: bool,
    has_server_fn: bool,
    has_leptos_graphql: bool,
    has_execute_graphql: bool,
    calls_raw_api: bool,
    calls_transport_facade: bool,
    uses_ui_leptos: bool,
    has_native_server_adapter: bool,
    has_graphql_adapter: bool,
    has_rest_adapter: bool,
    first_marker_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassifiedPath {
    module: String,
    surface: String,
    role: String,
    canonical_ui_adapter: bool,
    host_wiring: bool,
}

#[async_trait]
impl Extractor for RustokFfaExtractor {
    fn name(&self) -> &str {
        FFA_EXTRACTOR_ID
    }

    fn supports(&self, source: &SourceFile) -> bool {
        let path = normalize_path(&source.path);
        if is_rustok_module_surface_path(&path) || is_rustok_host_path(&path) {
            return matches!(
                path.rsplit('.').next(),
                Some("rs" | "toml" | "tsx" | "ts" | "jsx" | "js")
            ) || path.ends_with("Cargo.toml")
                || path.ends_with("rustok-module.toml");
        }
        false
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let path = normalize_path(&input.source.path);
        let Some(classified) = classify_path(&path) else {
            return Ok(ExtractOutput::default());
        };

        let content = input.source.content.as_deref().unwrap_or_default();
        let markers = detect_markers(content, &classified, &path);
        let file = file_entity(&input.source, &input.snapshot.0);
        let surface = surface_entity(
            &classified.module,
            &classified.surface,
            &input.snapshot,
            Some(&path),
        );
        let layer = layer_entity(
            &classified.module,
            &classified.surface,
            &classified.role,
            &input.snapshot,
            Some(&path),
        );
        let fact = Fact {
            id: FactId(format!(
                "fact_rustok_ffa_marker_{:016x}",
                stable_hash(format!("{}:{}", file.stable_key.0, classified.role).as_bytes())
            )),
            kind: FactKind::Other(FFA_MARKER_FACT_KIND.to_string()),
            subject: file.id.clone(),
            object: Some(layer.id.clone()),
            value: serde_json::to_value(markers).expect("SourceMarker serializes"),
            evidence: vec![evidence_for_file(
                &path,
                self.name(),
                first_marker_line(content),
                first_marker_line(content),
            )],
            ownership: ownership_for_file(&path),
            snapshot: input.snapshot,
            extractor: self.name().to_string(),
            confidence: 1.0,
        };

        Ok(ExtractOutput {
            entities: vec![file, surface, layer],
            facts: vec![fact],
        })
    }
}

#[async_trait]
impl Linker for RustokFfaLinker {
    fn name(&self) -> &str {
        FFA_LINKER_ID
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let entity_by_id = input
            .entities
            .iter()
            .map(|entity| (entity.id.0.as_str(), entity))
            .collect::<HashMap<_, _>>();
        let mut relations = Vec::new();
        let mut seen = BTreeSet::new();

        for fact in input.facts.iter().filter(|fact| is_ffa_marker_fact(fact)) {
            let Some(marker) = marker_from_fact(fact) else {
                continue;
            };
            let Some(file) = entity_by_id.get(fact.subject.0.as_str()) else {
                continue;
            };
            let surface = surface_entity(&marker.module, &marker.surface, &input.snapshot, None);
            let layer = layer_entity(
                &marker.module,
                &marker.surface,
                &marker.role,
                &input.snapshot,
                Some(&marker.path),
            );

            push_relation(
                &mut relations,
                &mut seen,
                &input.snapshot,
                RelationKind::Contains,
                &surface.id,
                &layer.id,
                fact.evidence.clone(),
                fact.ownership.clone(),
                json!({
                    "schema": "rustok.ffa.relation.v1",
                    "module": marker.module,
                    "surface": marker.surface,
                    "role": marker.role,
                }),
            );
            push_relation(
                &mut relations,
                &mut seen,
                &input.snapshot,
                RelationKind::ImplementedBy,
                &layer.id,
                &file.id,
                fact.evidence.clone(),
                fact.ownership.clone(),
                json!({
                    "schema": "rustok.ffa.relation.v1",
                    "kind": FFA_LAYER_RELATION_KIND,
                    "module": marker.module,
                    "surface": marker.surface,
                    "role": marker.role,
                    "path": marker.path,
                }),
            );
        }

        Ok(relations)
    }
}

#[async_trait]
impl Checker for RustokFfaChecker {
    fn name(&self) -> &str {
        FFA_CHECKER_ID
    }

    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>> {
        let mut surfaces = BTreeMap::<(String, String), SurfaceState>::new();
        for fact in input.facts.iter().filter(|fact| is_ffa_marker_fact(fact)) {
            let Some(marker) = marker_from_fact(fact) else {
                continue;
            };
            surfaces
                .entry((marker.module.clone(), marker.surface.clone()))
                .or_insert_with(|| SurfaceState::new(marker.module.clone(), marker.surface.clone()))
                .add_marker(marker, fact);
        }

        let mut diagnostics = Vec::new();
        for surface in surfaces.values() {
            diagnostics.extend(surface_diagnostics(surface, &input.snapshot));
        }
        diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        Ok(diagnostics)
    }
}

#[derive(Debug, Clone, Default)]
struct SurfaceState {
    module: String,
    surface: String,
    markers: Vec<(SourceMarker, Vec<Evidence>)>,
    layers: BTreeSet<String>,
    ownership: BTreeSet<String>,
}

impl SurfaceState {
    fn new(module: String, surface: String) -> Self {
        Self {
            module,
            surface,
            markers: Vec::new(),
            layers: BTreeSet::new(),
            ownership: BTreeSet::new(),
        }
    }

    fn add_marker(&mut self, marker: SourceMarker, fact: &Fact) {
        self.layers.insert(marker.role.clone());
        for owner in &fact.ownership {
            self.ownership.insert(owner.source_file.clone());
        }
        self.markers.push((marker, fact.evidence.clone()));
    }
}

fn surface_diagnostics(surface: &SurfaceState, snapshot: &SnapshotId) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let has_core = surface.layers.contains("core");
    let has_transport = surface.layers.contains("transport");
    let has_ui = surface.layers.contains("ui_leptos");
    let has_api = surface.layers.contains("api");
    let has_real_surface_code = has_core || has_transport || has_ui || has_api;

    for (marker, evidence) in &surface.markers {
        if marker.role == "core"
            && (marker.has_leptos_import || marker.has_component || marker.has_server_fn)
        {
            diagnostics.push(diagnostic(
                snapshot,
                "rustok_ffa_core_depends_on_leptos",
                Severity::High,
                "FFA core depends on Leptos",
                format!(
                    "{} {} core imports Leptos/component/server markers",
                    surface.module, surface.surface
                ),
                surface,
                Some(marker),
                evidence.clone(),
            ));
        }
        if marker.role == "ui_leptos"
            && (marker.calls_raw_api
                || marker.has_leptos_graphql
                || marker.has_execute_graphql
                || marker.has_server_fn)
        {
            diagnostics.push(diagnostic(
                snapshot,
                "rustok_ffa_ui_calls_raw_transport",
                Severity::High,
                "FFA UI bypasses transport facade",
                format!(
                    "{} {} UI calls raw api/graphql/server transport",
                    surface.module, surface.surface
                ),
                surface,
                Some(marker),
                evidence.clone(),
            ));
        }
        if marker.host_wiring && marker.has_component && marker.uses_ui_leptos {
            diagnostics.push(diagnostic(
                snapshot,
                "rustok_ffa_host_owns_module_ui",
                Severity::Medium,
                "Host owns module UI",
                format!(
                    "{} host contains component code for a module-owned FFA surface",
                    surface.surface
                ),
                surface,
                Some(marker),
                evidence.clone(),
            ));
        }
    }

    if has_real_surface_code && !has_core {
        diagnostics.push(layer_missing_diagnostic(
            snapshot,
            "rustok_ffa_surface_missing_core",
            "FFA surface is missing core layer",
            surface,
        ));
    }
    if has_ui && !has_transport {
        diagnostics.push(layer_missing_diagnostic(
            snapshot,
            "rustok_ffa_surface_missing_transport",
            "FFA surface is missing transport layer",
            surface,
        ));
    }
    if (has_core || has_transport || has_api) && !has_ui {
        diagnostics.push(layer_missing_diagnostic(
            snapshot,
            "rustok_ffa_surface_missing_ui_adapter",
            "FFA surface is missing UI adapter",
            surface,
        ));
    }
    if has_transport
        && !surface.markers.iter().any(|(marker, _)| {
            marker.role == "transport"
                && (marker.has_native_server_adapter
                    || marker.has_graphql_adapter
                    || marker.has_rest_adapter
                    || marker.has_leptos_graphql
                    || has_api)
        })
    {
        diagnostics.push(layer_missing_diagnostic(
            snapshot,
            "rustok_ffa_transport_profile_missing",
            "FFA transport profile is missing",
            surface,
        ));
    }
    if has_ui && (!has_core || !has_transport) {
        diagnostics.push(layer_missing_diagnostic(
            snapshot,
            "rustok_ffa_forgotten_surface",
            "FFA surface is partially translated and needs a complete layer shape",
            surface,
        ));
    }

    diagnostics
}

#[allow(clippy::too_many_arguments)]
fn diagnostic(
    snapshot: &SnapshotId,
    kind: &str,
    severity: Severity,
    title: &str,
    message: String,
    surface: &SurfaceState,
    marker: Option<&SourceMarker>,
    evidence: Vec<Evidence>,
) -> Diagnostic {
    let stable = marker.map_or_else(
        || format!("{kind}:{}:{}", surface.module, surface.surface),
        |marker| {
            format!(
                "{kind}:{}:{}:{}",
                surface.module, surface.surface, marker.path
            )
        },
    );
    Diagnostic {
        id: DiagnosticId(format!("diag_{:016x}", stable_hash(stable.as_bytes()))),
        kind: DiagnosticKind::Other(kind.to_string()),
        severity,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message,
        entities: Vec::new(),
        evidence,
        ownership: surface
            .ownership
            .iter()
            .map(|path| athanor_domain::Ownership {
                source_file: path.clone(),
            })
            .collect(),
        snapshot: snapshot.clone(),
        suggested_fix: None,
        payload: json!({
            "schema": "rustok.ffa.diagnostic.v1",
            "module": surface.module,
            "surface": surface.surface,
            "role": marker.map(|marker| marker.role.as_str()),
            "path": marker.map(|marker| marker.path.as_str()),
        }),
    }
}

fn layer_missing_diagnostic(
    snapshot: &SnapshotId,
    kind: &str,
    title: &str,
    surface: &SurfaceState,
) -> Diagnostic {
    let evidence = surface
        .markers
        .first()
        .map(|(_, evidence)| evidence.clone())
        .unwrap_or_default();
    diagnostic(
        snapshot,
        kind,
        Severity::Medium,
        title,
        format!(
            "{} {} does not have a complete FFA layer shape",
            surface.module, surface.surface
        ),
        surface,
        None,
        evidence,
    )
}

#[allow(clippy::too_many_arguments)]
fn push_relation(
    relations: &mut Vec<Relation>,
    seen: &mut BTreeSet<String>,
    snapshot: &SnapshotId,
    kind: RelationKind,
    from: &EntityId,
    to: &EntityId,
    evidence: Vec<Evidence>,
    ownership: Vec<athanor_domain::Ownership>,
    payload: Value,
) {
    let key = format!("{}:{}:{}", serialized_relation_kind(&kind), from.0, to.0);
    if !seen.insert(key.clone()) {
        return;
    }
    relations.push(Relation {
        id: RelationId(format!("rel_{:016x}", stable_hash(key.as_bytes()))),
        kind,
        from: from.clone(),
        to: to.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence,
        ownership,
        snapshot: snapshot.clone(),
        payload,
    });
}

fn surface_entity(
    module: &str,
    surface: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let stable_key = format!("ffa_surface://{module}/{surface}");
    Entity {
        id: EntityId(format!(
            "ent_ffa_surface_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FFA_SURFACE_ENTITY_KIND.to_string()),
        name: format!("{module}/{surface}"),
        title: Some(format!("RusTok FFA {module} {surface} surface")),
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({
            "schema": "rustok.ffa.surface.v1",
            "module": module,
            "surface": surface,
            "snapshot": snapshot.0,
        }),
    }
}

fn layer_entity(
    module: &str,
    surface: &str,
    role: &str,
    snapshot: &SnapshotId,
    path: Option<&str>,
) -> Entity {
    let stable_key = format!("ffa_layer://{module}/{surface}/{role}");
    Entity {
        id: EntityId(format!(
            "ent_ffa_layer_{:016x}",
            stable_hash(stable_key.as_bytes())
        )),
        stable_key: StableKey(stable_key),
        kind: EntityKind::Other(FFA_LAYER_ENTITY_KIND.to_string()),
        name: format!("{module}/{surface}/{role}"),
        title: Some(format!("RusTok FFA {module} {surface} {role} layer")),
        source: path.map(|path| SourceLocation {
            path: path.to_string(),
            line_start: None,
            line_end: None,
        }),
        language: Some(LanguageCode("rust".to_string())),
        aliases: Vec::new(),
        ownership: path.map_or_else(Vec::new, ownership_for_file),
        payload: json!({
            "schema": "rustok.ffa.layer.v1",
            "module": module,
            "surface": surface,
            "role": role,
            "snapshot": snapshot.0,
        }),
    }
}

fn detect_markers(content: &str, classified: &ClassifiedPath, path: &str) -> SourceMarker {
    SourceMarker {
        schema: "rustok.ffa.source_marker.v1".to_string(),
        path: path.to_string(),
        module: classified.module.clone(),
        surface: classified.surface.clone(),
        role: classified.role.clone(),
        canonical_ui_adapter: classified.canonical_ui_adapter,
        host_wiring: classified.host_wiring,
        has_leptos_import: contains_any(
            content,
            &["use leptos", "leptos::", "leptos_meta", "leptos_router"],
        ),
        has_component: content.contains("#[component]"),
        has_server_fn: content.contains("#[server") || content.contains("server_fn::"),
        has_leptos_graphql: content.contains("leptos_graphql"),
        has_execute_graphql: content.contains("execute_graphql")
            || content.contains("GraphQLRequest"),
        calls_raw_api: contains_any(content, &["crate::api", "super::api", "api::"]),
        calls_transport_facade: contains_any(
            content,
            &["crate::transport", "super::transport", "transport::"],
        ),
        uses_ui_leptos: contains_any(content, &["ui::leptos", "leptos::"]),
        has_native_server_adapter: path.contains("native_server_adapter")
            || content.contains("native_server_adapter"),
        has_graphql_adapter: path.contains("graphql_adapter")
            || content.contains("graphql_adapter")
            || content.contains("leptos_graphql"),
        has_rest_adapter: path.contains("rest_adapter") || content.contains("rest_adapter"),
        first_marker_line: first_marker_line(content),
    }
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn first_marker_line(content: &str) -> Option<u32> {
    content.lines().enumerate().find_map(|(index, line)| {
        contains_any(
            line,
            &[
                "#[component]",
                "#[server",
                "leptos",
                "leptos_graphql",
                "crate::api",
                "transport::",
                "ui::leptos",
            ],
        )
        .then_some(index as u32 + 1)
    })
}

fn classify_path(path: &str) -> Option<ClassifiedPath> {
    if let Some(host) = classify_host_path(path) {
        return Some(host);
    }
    let parts = path.split('/').collect::<Vec<_>>();
    let crates_index = parts.iter().position(|part| *part == "crates")?;
    let crate_name = *parts.get(crates_index + 1)?;
    let surface = *parts.get(crates_index + 2)?;
    if !crate_name.starts_with("rustok-") || !matches!(surface, "admin" | "storefront") {
        return None;
    }
    let module = crate_name.trim_start_matches("rustok-").to_string();
    let role = role_for_module_path(path, surface);
    Some(ClassifiedPath {
        module,
        surface: surface.to_string(),
        canonical_ui_adapter: path.contains("/src/ui/leptos.rs")
            || path.contains("/src/ui/leptos/"),
        host_wiring: false,
        role,
    })
}

fn classify_host_path(path: &str) -> Option<ClassifiedPath> {
    let parts = path.split('/').collect::<Vec<_>>();
    let apps_index = parts.iter().position(|part| *part == "apps")?;
    let surface = *parts.get(apps_index + 1)?;
    if !matches!(surface, "admin" | "storefront") {
        return None;
    }
    Some(ClassifiedPath {
        module: "host".to_string(),
        surface: surface.to_string(),
        role: "host_wiring".to_string(),
        canonical_ui_adapter: false,
        host_wiring: true,
    })
}

fn role_for_module_path(path: &str, surface: &str) -> String {
    let surface_root = format!("/{surface}/");
    if path.ends_with(&format!("{surface}/Cargo.toml")) || path.ends_with("rustok-module.toml") {
        return "manifest".to_string();
    }
    if path.contains(&format!("{surface_root}src/core.rs"))
        || path.contains(&format!("{surface_root}src/core/"))
    {
        return "core".to_string();
    }
    if path.contains(&format!("{surface_root}src/transport.rs"))
        || path.contains(&format!("{surface_root}src/transport/"))
    {
        return "transport".to_string();
    }
    if path.contains(&format!("{surface_root}src/ui/")) {
        return "ui_leptos".to_string();
    }
    if path.contains(&format!("{surface_root}src/api.rs"))
        || path.contains(&format!("{surface_root}src/api/"))
    {
        return "api".to_string();
    }
    if path.contains(&format!("{surface_root}src/lib.rs")) {
        return "crate_root".to_string();
    }
    "other".to_string()
}

fn is_rustok_module_surface_path(path: &str) -> bool {
    (path.starts_with("crates/rustok-") || path.contains("/crates/rustok-"))
        && (path.contains("/admin/") || path.contains("/storefront/"))
}

fn is_rustok_host_path(path: &str) -> bool {
    path.starts_with("apps/admin/")
        || path.starts_with("apps/storefront/")
        || path.contains("/apps/admin/")
        || path.contains("/apps/storefront/")
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn is_ffa_marker_fact(fact: &Fact) -> bool {
    matches!(&fact.kind, FactKind::Other(kind) if kind == FFA_MARKER_FACT_KIND)
}

fn marker_from_fact(fact: &Fact) -> Option<SourceMarker> {
    serde_json::from_value(fact.value.clone()).ok()
}

fn serialized_relation_kind(kind: &RelationKind) -> String {
    match kind {
        RelationKind::Contains => "contains".to_string(),
        RelationKind::ImplementedBy => "implemented_by".to_string(),
        RelationKind::Other(value) => value.clone(),
        other => serde_json::to_value(other)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "relation".to_string()),
    }
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

    #[tokio::test]
    async fn discovers_admin_surface_from_windows_path() {
        let output = RustokFfaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source: source(
                    "crates\\rustok-blog\\admin\\src\\core.rs",
                    "pub struct BlogAdminState;",
                ),
            })
            .await
            .unwrap();

        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "ffa_surface://blog/admin")
        );
        assert!(
            output
                .entities
                .iter()
                .any(|entity| entity.stable_key.0 == "ffa_layer://blog/admin/core")
        );
    }

    #[test]
    fn supports_project_relative_windows_and_unix_surface_paths() {
        let extractor = RustokFfaExtractor;

        assert!(extractor.supports(&source(
            "crates/rustok-blog/admin/src/core.rs",
            "pub struct State;",
        )));
        assert!(extractor.supports(&source(
            "crates\\rustok-blog\\storefront\\src\\ui\\leptos.rs",
            "#[component] pub fn View() {}",
        )));
        assert!(extractor.supports(&source(
            "apps/admin/src/module_shell.rs",
            "use rustok_blog::ui::leptos;",
        )));
    }

    #[tokio::test]
    async fn core_leptos_import_emits_diagnostic() {
        let extractor = RustokFfaExtractor;
        let extracted = extractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source: source(
                    "crates/rustok-blog/admin/src/core.rs",
                    "use leptos::prelude::*;\npub struct State;",
                ),
            })
            .await
            .unwrap();

        let diagnostics = RustokFfaChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: std::sync::Arc::new(extracted.entities),
                facts: std::sync::Arc::new(extracted.facts),
                relations: std::sync::Arc::new(Vec::new()),
                affected: Default::default(),
            })
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind
                == DiagnosticKind::Other("rustok_ffa_core_depends_on_leptos".to_string())
        }));
    }

    #[tokio::test]
    async fn ui_raw_api_call_emits_diagnostic() {
        let extracted = RustokFfaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source: source(
                    "crates/rustok-blog/admin/src/ui/leptos.rs",
                    "#[component]\npub fn View() { crate::api::load(); }",
                ),
            })
            .await
            .unwrap();

        let diagnostics = RustokFfaChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: std::sync::Arc::new(extracted.entities),
                facts: std::sync::Arc::new(extracted.facts),
                relations: std::sync::Arc::new(Vec::new()),
                affected: Default::default(),
            })
            .await
            .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind
                == DiagnosticKind::Other("rustok_ffa_ui_calls_raw_transport".to_string())
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::Other("rustok_ffa_surface_missing_core".to_string())
        }));
    }

    #[tokio::test]
    async fn clean_core_transport_ui_surface_has_no_diagnostics() {
        let inputs = [
            source("crates/rustok-blog/admin/src/core.rs", "pub struct State;"),
            source(
                "crates/rustok-blog/admin/src/transport/native_server_adapter.rs",
                "pub async fn load() {}",
            ),
            source(
                "crates/rustok-blog/admin/src/ui/leptos.rs",
                "#[component]\npub fn View() { crate::transport::load(); }",
            ),
        ];
        let mut entities = Vec::new();
        let mut facts = Vec::new();
        for source in inputs {
            let output = RustokFfaExtractor
                .extract(ExtractInput {
                    repo: RepoId("repo".to_string()),
                    snapshot: SnapshotId("snap".to_string()),
                    source,
                })
                .await
                .unwrap();
            entities.extend(output.entities);
            facts.extend(output.facts);
        }

        let diagnostics = RustokFfaChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: std::sync::Arc::new(entities),
                facts: std::sync::Arc::new(facts),
                relations: std::sync::Arc::new(Vec::new()),
                affected: Default::default(),
            })
            .await
            .unwrap();

        assert!(diagnostics.is_empty());
    }
}
