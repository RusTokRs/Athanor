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
pub const FFA_DOCS_STATUS_FACT_KIND: &str = "rustok_ffa_docs_status";
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
    has_native_graphql_fallback: bool,
    first_marker_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DocsStatusMarker {
    schema: String,
    source_kind: String,
    path: String,
    module: String,
    surfaces: String,
    ffa_status: String,
    fba_status: String,
    structural_shape: String,
    line: u32,
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
        if is_ffa_registry_path(&path) || is_module_implementation_plan_path(&path) {
            return true;
        }
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
        if is_ffa_registry_path(&path) {
            return Ok(extract_docs_statuses(&input.source, &input.snapshot, &path));
        }
        if is_module_implementation_plan_path(&path) {
            return Ok(extract_local_plan_status(
                &input.source,
                &input.snapshot,
                &path,
            ));
        }
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
        diagnostics.extend(docs_drift_diagnostics(&input, &surfaces));
        diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        Ok(diagnostics)
    }
}

fn extract_docs_statuses(source: &SourceFile, snapshot: &SnapshotId, path: &str) -> ExtractOutput {
    let file = file_entity(source, &snapshot.0);
    let facts = source
        .content
        .as_deref()
        .unwrap_or_default()
        .lines()
        .enumerate()
        .filter_map(|(index, line)| parse_docs_status_row(line, path, index as u32 + 1))
        .map(|marker| Fact {
            id: FactId(format!(
                "fact_rustok_ffa_docs_status_{:016x}",
                stable_hash(
                    format!(
                        "{}:{}:{}:{}",
                        path, marker.module, marker.surfaces, marker.line
                    )
                    .as_bytes()
                )
            )),
            kind: FactKind::Other(FFA_DOCS_STATUS_FACT_KIND.to_string()),
            subject: file.id.clone(),
            object: None,
            value: serde_json::to_value(&marker).expect("DocsStatusMarker serializes"),
            evidence: vec![evidence_for_file(
                path,
                FFA_EXTRACTOR_ID,
                Some(marker.line),
                Some(marker.line),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: FFA_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        })
        .collect();
    ExtractOutput {
        entities: vec![file],
        facts,
    }
}

fn extract_local_plan_status(
    source: &SourceFile,
    snapshot: &SnapshotId,
    path: &str,
) -> ExtractOutput {
    let file = file_entity(source, &snapshot.0);
    let content = source.content.as_deref().unwrap_or_default();
    let module = path
        .split('/')
        .find(|part| part.starts_with("rustok-"))
        .map(|part| part.trim_start_matches("rustok-").replace('-', "_"));
    let status = module
        .as_deref()
        .and_then(|module| parse_local_plan_status(content, path, module));
    let facts = status
        .into_iter()
        .map(|marker| Fact {
            id: FactId(format!(
                "fact_rustok_ffa_local_status_{:016x}",
                stable_hash(format!("{}:{}", path, marker.module).as_bytes())
            )),
            kind: FactKind::Other(FFA_DOCS_STATUS_FACT_KIND.to_string()),
            subject: file.id.clone(),
            object: None,
            value: serde_json::to_value(&marker).expect("DocsStatusMarker serializes"),
            evidence: vec![evidence_for_file(
                path,
                FFA_EXTRACTOR_ID,
                Some(marker.line),
                Some(marker.line),
            )],
            ownership: ownership_for_file(path),
            snapshot: snapshot.clone(),
            extractor: FFA_EXTRACTOR_ID.to_string(),
            confidence: 1.0,
        })
        .collect();
    ExtractOutput {
        entities: vec![file],
        facts,
    }
}

fn parse_docs_status_row(line: &str, path: &str, line_number: u32) -> Option<DocsStatusMarker> {
    let columns = line.split('|').map(str::trim).collect::<Vec<_>>();
    if columns.len() < 7 || !columns[1].starts_with('`') {
        return None;
    }
    let module = columns[1].trim_matches('`');
    if module.is_empty() || module == "Module slug" {
        return None;
    }
    Some(DocsStatusMarker {
        schema: "rustok.ffa.docs_status.v1".to_string(),
        source_kind: "registry".to_string(),
        path: path.to_string(),
        module: module.to_string(),
        surfaces: columns[2].to_string(),
        ffa_status: columns[3].trim_matches('`').to_string(),
        fba_status: columns[4].trim_matches('`').to_string(),
        structural_shape: columns[5].trim_matches('`').to_string(),
        line: line_number,
    })
}

fn parse_local_plan_status(content: &str, path: &str, module: &str) -> Option<DocsStatusMarker> {
    let mut ffa_status = None;
    let mut fba_status = None;
    let mut structural_shape = None;
    let mut first_line = None;
    for (index, line) in content.lines().enumerate() {
        let line_number = index as u32 + 1;
        if let Some(value) = markdown_status_value(line, "- FFA status:") {
            ffa_status = Some(value);
            first_line.get_or_insert(line_number);
        } else if let Some(value) = markdown_status_value(line, "- FBA status:") {
            fba_status = Some(value);
            first_line.get_or_insert(line_number);
        } else if let Some(value) = markdown_status_value(line, "- Structural shape:") {
            structural_shape = Some(value);
            first_line.get_or_insert(line_number);
        }
    }
    Some(DocsStatusMarker {
        schema: "rustok.ffa.docs_status.v1".to_string(),
        source_kind: "local_plan".to_string(),
        path: path.to_string(),
        module: module.to_string(),
        surfaces: String::new(),
        ffa_status: ffa_status?,
        fba_status: fba_status?,
        structural_shape: structural_shape?,
        line: first_line?,
    })
}

fn markdown_status_value(line: &str, prefix: &str) -> Option<String> {
    line.trim()
        .strip_prefix(prefix)?
        .trim()
        .strip_prefix('`')?
        .split('`')
        .next()
        .map(str::to_string)
}

fn docs_drift_diagnostics(
    input: &CheckInput,
    surfaces: &BTreeMap<(String, String), SurfaceState>,
) -> Vec<Diagnostic> {
    let mut registry = BTreeMap::<(String, String), Vec<(DocsStatusMarker, &Fact)>>::new();
    let mut local_plans = BTreeMap::<String, (DocsStatusMarker, &Fact)>::new();
    for fact in input
        .facts
        .iter()
        .filter(|fact| fact.kind == FactKind::Other(FFA_DOCS_STATUS_FACT_KIND.to_string()))
    {
        let Ok(marker) = serde_json::from_value::<DocsStatusMarker>(fact.value.clone()) else {
            continue;
        };
        if marker.source_kind == "registry" {
            registry
                .entry((marker.module.clone(), marker.surfaces.clone()))
                .or_default()
                .push((marker, fact));
        } else if marker.source_kind == "local_plan" {
            local_plans.insert(marker.module.clone(), (marker, fact));
        }
    }

    let mut diagnostics = Vec::new();
    let registry_latest = registry
        .iter()
        .filter_map(|((module, surfaces), entries)| {
            entries
                .last()
                .map(|(marker, fact)| ((module.clone(), surfaces.clone()), (marker, *fact)))
        })
        .collect::<BTreeMap<_, _>>();

    for ((module, surfaces), entries) in &registry {
        if entries.len() > 1
            && let Some((_, fact)) = entries.last()
        {
            diagnostics.push(docs_drift_diagnostic(
                &input.snapshot,
                module,
                surfaces,
                format!(
                    "{module} {surfaces} appears {} times in the FFA/FBA readiness board",
                    entries.len()
                ),
                fact,
            ));
        }
        let Some((registry_marker, registry_fact)) = entries.last() else {
            continue;
        };
        let Some((local_marker, _)) = local_plans.get(module) else {
            diagnostics.push(docs_drift_diagnostic(
                &input.snapshot,
                module,
                surfaces,
                format!("{module} readiness board entry has no parseable local FFA/FBA status"),
                registry_fact,
            ));
            continue;
        };
        if registry_marker.ffa_status != local_marker.ffa_status
            || registry_marker.fba_status != local_marker.fba_status
            || registry_marker.structural_shape != local_marker.structural_shape
        {
            diagnostics.push(docs_drift_diagnostic(
                &input.snapshot,
                module,
                surfaces,
                format!(
                    "{module} registry status {}/{}/{} differs from local plan {}/{}/{}",
                    registry_marker.ffa_status,
                    registry_marker.fba_status,
                    registry_marker.structural_shape,
                    local_marker.ffa_status,
                    local_marker.fba_status,
                    local_marker.structural_shape
                ),
                registry_fact,
            ));
        }
    }
    for surface in surfaces.values().filter(|surface| {
        !registry_latest.is_empty()
            && surface
                .layers
                .iter()
                .any(|layer| matches!(layer.as_str(), "core" | "transport" | "ui_leptos" | "api"))
    }) {
        let registry_entry = registry_latest.iter().find(|((module, declared), _)| {
            module == &surface.module
                && declared
                    .split_whitespace()
                    .any(|part| part == surface.surface)
        });
        let Some(((_, _), (registry_marker, registry_fact))) = registry_entry else {
            let evidence = surface
                .markers
                .first()
                .map(|(_, evidence)| evidence.clone())
                .unwrap_or_default();
            diagnostics.push(diagnostic(
                &input.snapshot,
                "rustok_ffa_docs_drift",
                Severity::Medium,
                "FFA readiness documentation drift",
                format!(
                    "{} {} code surface has no readiness board entry",
                    surface.module, surface.surface
                ),
                surface,
                None,
                evidence,
            ));
            continue;
        };
        let code_shape = surface_code_shape(surface);
        if registry_marker.structural_shape != code_shape {
            diagnostics.push(docs_drift_diagnostic(
                &input.snapshot,
                &surface.module,
                &surface.surface,
                format!(
                    "{} {} code shape {} differs from readiness board shape {}",
                    surface.module, surface.surface, code_shape, registry_marker.structural_shape
                ),
                registry_fact,
            ));
        }
    }
    diagnostics
}

fn surface_code_shape(surface: &SurfaceState) -> &'static str {
    let has_core = surface.layers.contains("core");
    let has_transport = surface.layers.contains("transport");
    let has_ui = surface.layers.contains("ui_leptos");
    match (has_core, has_transport, has_ui) {
        (true, true, true) => "core_transport_ui",
        (true, true, false) => "core_transport",
        (true, false, true) => "core_ui",
        (false, true, true) => "transport_ui",
        (true, false, false) => "core_only",
        (false, true, false) => "transport_only",
        (false, false, true) => "ui_only",
        (false, false, false) => "none",
    }
}

fn docs_drift_diagnostic(
    snapshot: &SnapshotId,
    module: &str,
    surfaces: &str,
    message: String,
    fact: &Fact,
) -> Diagnostic {
    let surface = SurfaceState {
        module: module.to_string(),
        surface: surfaces.to_string(),
        markers: Vec::new(),
        layers: BTreeSet::new(),
        ownership: fact
            .ownership
            .iter()
            .map(|owner| owner.source_file.clone())
            .collect(),
    };
    diagnostic(
        snapshot,
        "rustok_ffa_docs_drift",
        Severity::Medium,
        "FFA readiness documentation drift",
        message,
        &surface,
        None,
        fact.evidence.clone(),
    )
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
        if matches!(marker.role.as_str(), "ui_leptos" | "ui_support")
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
                    || marker.has_native_graphql_fallback
                    || has_api)
        })
    {
        if let Some((marker, evidence)) = surface
            .markers
            .iter()
            .find(|(marker, _)| marker.role == "transport")
        {
            diagnostics.push(diagnostic(
                snapshot,
                "rustok_ffa_transport_profile_missing",
                Severity::Medium,
                "FFA transport profile is missing",
                format!(
                    "{} {} transport layer does not expose a native, GraphQL, REST, or API fallback profile",
                    surface.module, surface.surface
                ),
                surface,
                Some(marker),
                evidence.clone(),
            ));
        } else {
            diagnostics.push(layer_missing_diagnostic(
                snapshot,
                "rustok_ffa_transport_profile_missing",
                "FFA transport profile is missing",
                surface,
            ));
        }
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
        has_server_fn: contains_server_marker(content),
        has_leptos_graphql: content.contains("leptos_graphql"),
        has_execute_graphql: content.contains("execute_graphql")
            || content.contains("GraphQLRequest"),
        calls_raw_api: contains_raw_api_reference(content),
        calls_transport_facade: contains_any(
            content,
            &["crate::transport", "super::transport", "transport::"],
        ),
        uses_ui_leptos: contains_ui_leptos_reference(content),
        has_native_server_adapter: path.contains("native_server_adapter")
            || content.contains("native_server_adapter"),
        has_graphql_adapter: path.contains("graphql_adapter")
            || content.contains("graphql_adapter")
            || content.contains("leptos_graphql"),
        has_rest_adapter: path.contains("rest_adapter") || content.contains("rest_adapter"),
        has_native_graphql_fallback: content.contains("ServerFn(")
            && content.contains("Graphql(")
            && content.contains("_with_fallback"),
        first_marker_line: first_marker_line(content),
    }
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn contains_raw_api_reference(content: &str) -> bool {
    content.lines().any(|line| {
        line.contains("crate::api")
            || line.contains("super::api")
            || line.contains("self::api")
            || line.trim_start().starts_with("api::")
            || line.contains("{api::")
            || line.contains("(api::")
            || line.contains(" api::")
    })
}

fn contains_ui_leptos_reference(content: &str) -> bool {
    contains_any(
        content,
        &[
            "ui::leptos",
            "::ui::leptos",
            "ui::admin",
            "::ui::admin",
            "ui::storefront",
            "::ui::storefront",
        ],
    )
}

fn contains_server_marker(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("#[server") || trimmed.starts_with("server_fn::")
    })
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
    if path.contains(&format!("{surface_root}src/ui/leptos.rs"))
        || path.contains(&format!("{surface_root}src/ui/leptos/"))
    {
        return "ui_leptos".to_string();
    }
    if path.contains(&format!("{surface_root}src/ui/")) {
        return "ui_support".to_string();
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

fn is_ffa_registry_path(path: &str) -> bool {
    path == "docs/modules/registry.md" || path.ends_with("/docs/modules/registry.md")
}

fn is_module_implementation_plan_path(path: &str) -> bool {
    (path.starts_with("crates/rustok-") || path.contains("/crates/rustok-"))
        && path.ends_with("/docs/implementation-plan.md")
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

    #[test]
    fn marker_detection_does_not_treat_rustok_api_or_plain_leptos_as_boundary_bypass() {
        let classified = ClassifiedPath {
            module: "blog".to_string(),
            surface: "admin".to_string(),
            role: "ui_leptos".to_string(),
            canonical_ui_adapter: true,
            host_wiring: false,
        };
        let marker = detect_markers(
            "use leptos::prelude::*;\nuse rustok_api::{AdminQueryKey, UiRouteContext};",
            &classified,
            "crates/rustok-blog/admin/src/ui/leptos.rs",
        );

        assert!(!marker.calls_raw_api);
        assert!(!marker.uses_ui_leptos);
    }

    #[test]
    fn marker_detection_ignores_server_marker_inside_copy_text() {
        let classified = ClassifiedPath {
            module: "media".to_string(),
            surface: "admin".to_string(),
            role: "ui_leptos".to_string(),
            canonical_ui_adapter: true,
            host_wiring: false,
        };
        let marker = detect_markers(
            r#"let subtitle = "Native #[server] calls cover the read flow.";"#,
            &classified,
            "crates/rustok-media/admin/src/ui/leptos.rs",
        );

        assert!(!marker.has_server_fn);
    }

    #[test]
    fn only_canonical_leptos_path_satisfies_ui_adapter_role() {
        assert_eq!(
            role_for_module_path("crates/rustok-auth/admin/src/ui/leptos.rs", "admin"),
            "ui_leptos"
        );
        assert_eq!(
            role_for_module_path("crates/rustok-auth/admin/src/ui/leptos/mod.rs", "admin"),
            "ui_leptos"
        );
        assert_eq!(
            role_for_module_path("crates/rustok-auth/admin/src/ui/reset.rs", "admin"),
            "ui_support"
        );
    }

    #[test]
    fn module_layout_variants_map_to_stable_roles() {
        let cases = [
            ("crates/rustok-auth/admin/src/core.rs", "core"),
            ("crates/rustok-auth/admin/src/core/mod.rs", "core"),
            ("crates/rustok-auth/admin/src/core/state/model.rs", "core"),
            ("crates/rustok-auth/admin/src/transport.rs", "transport"),
            ("crates/rustok-auth/admin/src/transport/mod.rs", "transport"),
            (
                "crates/rustok-auth/admin/src/transport/native/client.rs",
                "transport",
            ),
            ("crates/rustok-auth/admin/src/ui/leptos.rs", "ui_leptos"),
            ("crates/rustok-auth/admin/src/ui/leptos/mod.rs", "ui_leptos"),
            (
                "crates/rustok-auth/admin/src/ui/leptos/components/form.rs",
                "ui_leptos",
            ),
            ("crates/rustok-auth/admin/src/ui/reset.rs", "ui_support"),
            ("crates/rustok-auth/admin/src/api/session.rs", "api"),
            ("crates/rustok-auth/admin/src/lib.rs", "crate_root"),
            ("crates/rustok-auth/admin/Cargo.toml", "manifest"),
            ("crates/rustok-auth/admin/rustok-module.toml", "manifest"),
        ];

        for (path, expected_role) in cases {
            assert_eq!(role_for_module_path(path, "admin"), expected_role, "{path}");
        }
    }

    #[tokio::test]
    async fn nested_layer_files_link_to_same_canonical_layer() {
        let inputs = [
            source("crates/rustok-blog/admin/src/core/mod.rs", "mod state;"),
            source(
                "crates/rustok-blog/admin/src/core/state.rs",
                "pub struct State;",
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

        let core_layer_id = entities
            .iter()
            .find(|entity| entity.stable_key.0 == "ffa_layer://blog/admin/core")
            .map(|entity| entity.id.clone())
            .expect("core layer entity is extracted");

        let relations = RustokFfaLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: std::sync::Arc::new(entities),
                facts: std::sync::Arc::new(facts),
                affected: Default::default(),
            })
            .await
            .unwrap();

        let implemented_core_files = relations
            .iter()
            .filter(|relation| relation.from == core_layer_id)
            .filter(|relation| matches!(relation.kind, RelationKind::ImplementedBy))
            .count();
        assert_eq!(implemented_core_files, 2);
    }

    #[test]
    fn marker_detection_keeps_real_raw_api_and_ui_leptos_references() {
        let classified = ClassifiedPath {
            module: "blog".to_string(),
            surface: "admin".to_string(),
            role: "ui_leptos".to_string(),
            canonical_ui_adapter: true,
            host_wiring: false,
        };
        let marker = detect_markers(
            "use crate::api;\npub fn render() { rustok_blog_admin::ui::leptos::BlogAdmin(); }",
            &classified,
            "crates/rustok-blog/admin/src/ui/leptos.rs",
        );

        assert!(marker.calls_raw_api);
        assert!(marker.uses_ui_leptos);
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

    #[tokio::test]
    async fn clean_directory_layout_surface_has_no_missing_layer_diagnostics() {
        let inputs = [
            source("crates/rustok-blog/admin/src/core/mod.rs", "pub mod state;"),
            source(
                "crates/rustok-blog/admin/src/transport/mod.rs",
                "pub mod native_server_adapter;",
            ),
            source(
                "crates/rustok-blog/admin/src/ui/leptos/mod.rs",
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

        assert!(!diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.kind,
            DiagnosticKind::Other(ref kind)
                if kind == "rustok_ffa_surface_missing_core"
                    || kind == "rustok_ffa_surface_missing_transport"
                    || kind == "rustok_ffa_surface_missing_ui_adapter"
                    || kind == "rustok_ffa_forgotten_surface"
        )));
    }

    #[tokio::test]
    async fn native_graphql_fallback_facade_declares_transport_profile() {
        let inputs = [
            source(
                "crates/rustok-order/storefront/src/core.rs",
                "pub struct State;",
            ),
            source(
                "crates/rustok-order/storefront/src/transport.rs",
                r#"
pub enum TransportError {
    Graphql(String),
    ServerFn(String),
}

pub async fn complete_checkout_with_fallback() {}
"#,
            ),
            source(
                "crates/rustok-order/storefront/src/ui/leptos.rs",
                "#[component]\npub fn View() { crate::transport::complete_checkout_with_fallback(); }",
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

        assert!(!diagnostics.iter().any(|diagnostic| {
            diagnostic.kind
                == DiagnosticKind::Other("rustok_ffa_transport_profile_missing".to_string())
        }));
    }

    #[tokio::test]
    async fn duplicate_readiness_rows_emit_docs_drift() {
        let registry = source(
            "docs/modules/registry.md",
            r#"
| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |
|---|---|---|---|---|---|
| `blog` | admin + storefront | `in_progress` | `not_started` | `core_transport_ui` | plan |
| `blog` | admin + storefront | `in_progress` | `not_started` | `core_transport_ui` | newer plan |
"#,
        );
        let mut output = RustokFfaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source: registry,
            })
            .await
            .unwrap();
        let local = RustokFfaExtractor
            .extract(ExtractInput {
                repo: RepoId("repo".to_string()),
                snapshot: SnapshotId("snap".to_string()),
                source: source(
                    "crates/rustok-blog/docs/implementation-plan.md",
                    r#"
- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
"#,
                ),
            })
            .await
            .unwrap();
        output.entities.extend(local.entities);
        output.facts.extend(local.facts);

        let diagnostics = RustokFfaChecker
            .check(CheckInput {
                snapshot: SnapshotId("snap".to_string()),
                entities: std::sync::Arc::new(output.entities),
                facts: std::sync::Arc::new(output.facts),
                relations: std::sync::Arc::new(Vec::new()),
                affected: Default::default(),
            })
            .await
            .unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            DiagnosticKind::Other("rustok_ffa_docs_drift".to_string())
        );
        assert_eq!(diagnostics[0].payload["module"], "blog");
        assert_eq!(diagnostics[0].payload["surface"], "admin + storefront");
    }

    #[tokio::test]
    async fn registry_and_local_status_mismatch_emits_docs_drift() {
        let inputs = [
            source(
                "docs/modules/registry.md",
                "| `auth` | admin | `in_progress` | `not_started` | `core_transport_ui` | plan |",
            ),
            source(
                "crates/rustok-auth/docs/implementation-plan.md",
                r#"
- FFA status: `in_progress`
- FBA status: `not_applicable`
- Structural shape: `core_transport_ui`
"#,
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

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("not_applicable"));
    }
}
