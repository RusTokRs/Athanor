use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use anyhow::{Context, Result};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{DiagnosticStatus, Entity, EntityKind, RelationKind, Severity};
use serde::Serialize;

pub const OVERVIEW_SCHEMA: &str = crate::json_contract::OVERVIEW_SCHEMA_V1;

#[derive(Debug, Clone)]
pub struct OverviewOptions {
    pub root: PathBuf,
    pub top: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryOverview {
    pub schema: String,
    pub snapshot: String,
    pub totals: OverviewTotals,
    pub entity_kinds: Vec<NamedCount>,
    pub relation_kinds: Vec<NamedCount>,
    pub source_roots: Vec<NamedCount>,
    pub api: ApiOverview,
    pub docs: DocsOverview,
    pub operations: OperationsOverview,
    pub module_structure: Vec<ModuleOverview>,
    pub integration_boundaries: Vec<IntegrationBoundaryOverview>,
    pub graph_hubs: Vec<EntityOverview>,
    pub open_diagnostics: Vec<DiagnosticOverview>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModuleOverview {
    pub stable_key: String,
    pub name: String,
    pub source: Option<String>,
    pub direct_members: usize,
    pub relation_ids: Vec<String>,
    pub omitted_relation_ids: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IntegrationBoundaryOverview {
    pub from_root: String,
    pub to_root: String,
    pub relations: usize,
    pub relation_kinds: Vec<NamedCount>,
    pub relation_ids: Vec<String>,
    pub omitted_relation_ids: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct OverviewTotals {
    pub entities: usize,
    pub facts: usize,
    pub relations: usize,
    pub diagnostics: usize,
    pub open_diagnostics: usize,
    pub source_files: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NamedCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct ApiOverview {
    pub endpoints: usize,
    pub schemas: usize,
    pub examples: usize,
    pub documented_endpoints: usize,
    pub implemented_endpoints: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct DocsOverview {
    pub pages: usize,
    pub sections: usize,
    pub runbooks: usize,
    pub operation_steps: usize,
    pub operations_pages: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct OperationsOverview {
    pub environment_variables: usize,
    pub script_commands: usize,
    pub deployment_resources: usize,
    pub database_migrations: usize,
    pub packages: usize,
    pub dependencies: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EntityOverview {
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
    pub degree: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiagnosticOverview {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub source: Option<String>,
}

pub async fn overview_project(options: OverviewOptions) -> Result<RepositoryOverview> {
    overview_project_inner(options, None).await
}

/// Builds an overview with explicitly supplied runtime dependencies.
pub async fn overview_project_with_composition(
    options: OverviewOptions,
    composition: &RuntimeComposition,
) -> Result<RepositoryOverview> {
    overview_project_inner(options, Some(composition)).await
}

async fn overview_project_inner(
    options: OverviewOptions,
    composition: Option<&RuntimeComposition>,
) -> Result<RepositoryOverview> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = match composition {
        Some(composition) => composition.init_store(&root, &config).await?,
        None => init_store(&root, &config).await?,
    };
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;

    Ok(build_repository_overview(&snapshot, options.top.max(1)))
}

pub fn build_repository_overview(snapshot: &CanonicalSnapshot, top: usize) -> RepositoryOverview {
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    let source_files = snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::File)
        .count();
    let open_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .count();

    RepositoryOverview {
        schema: OVERVIEW_SCHEMA.to_string(),
        snapshot: snapshot_id,
        totals: OverviewTotals {
            entities: snapshot.entities.len(),
            facts: snapshot.facts.len(),
            relations: snapshot.relations.len(),
            diagnostics: snapshot.diagnostics.len(),
            open_diagnostics,
            source_files,
        },
        entity_kinds: top_counts(
            snapshot
                .entities
                .iter()
                .map(|entity| serialized_name(&entity.kind)),
            top,
        ),
        relation_kinds: top_counts(
            snapshot
                .relations
                .iter()
                .map(|relation| serialized_name(&relation.kind)),
            top,
        ),
        source_roots: top_counts(snapshot.entities.iter().filter_map(entity_source_root), top),
        api: api_overview(snapshot),
        docs: docs_overview(snapshot),
        operations: operations_overview(snapshot),
        module_structure: module_structure(snapshot, top),
        integration_boundaries: integration_boundaries(snapshot, top),
        graph_hubs: graph_hubs(snapshot, top),
        open_diagnostics: diagnostic_overviews(snapshot, top),
    }
}

fn module_structure(snapshot: &CanonicalSnapshot, top: usize) -> Vec<ModuleOverview> {
    let mut members = HashMap::<String, Vec<String>>::new();
    for relation in &snapshot.relations {
        if matches!(
            relation.kind,
            RelationKind::Defines | RelationKind::Contains
        ) {
            members
                .entry(relation.from.0.clone())
                .or_default()
                .push(relation.id.0.clone());
        }
    }

    let mut modules = snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Module)
        .map(|entity| {
            let mut relation_ids = members.remove(&entity.id.0).unwrap_or_default();
            relation_ids.sort();
            let direct_members = relation_ids.len();
            relation_ids.truncate(top);
            ModuleOverview {
                stable_key: entity.stable_key.0.clone(),
                name: entity.name.clone(),
                source: entity_source_anchor(entity),
                direct_members,
                omitted_relation_ids: direct_members.saturating_sub(relation_ids.len()),
                relation_ids,
            }
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| {
        right
            .direct_members
            .cmp(&left.direct_members)
            .then_with(|| left.stable_key.cmp(&right.stable_key))
    });
    modules.truncate(top);
    modules
}

fn integration_boundaries(
    snapshot: &CanonicalSnapshot,
    top: usize,
) -> Vec<IntegrationBoundaryOverview> {
    let root_by_id = snapshot
        .entities
        .iter()
        .filter_map(|entity| entity_source_root(entity).map(|root| (entity.id.0.clone(), root)))
        .collect::<HashMap<_, _>>();
    let mut grouped = BTreeMap::<(String, String), Vec<&athanor_domain::Relation>>::new();

    for relation in &snapshot.relations {
        let Some(from_root) = root_by_id.get(&relation.from.0) else {
            continue;
        };
        let Some(to_root) = root_by_id.get(&relation.to.0) else {
            continue;
        };
        if from_root != to_root {
            grouped
                .entry((from_root.clone(), to_root.clone()))
                .or_default()
                .push(relation);
        }
    }

    let mut boundaries = grouped
        .into_iter()
        .map(|((from_root, to_root), relations)| {
            let mut relation_ids = relations
                .iter()
                .map(|relation| relation.id.0.clone())
                .collect::<Vec<_>>();
            relation_ids.sort();
            let relation_count = relation_ids.len();
            relation_ids.truncate(top);
            IntegrationBoundaryOverview {
                from_root,
                to_root,
                relations: relation_count,
                relation_kinds: top_counts(
                    relations
                        .iter()
                        .map(|relation| serialized_name(&relation.kind)),
                    top,
                ),
                omitted_relation_ids: relation_count.saturating_sub(relation_ids.len()),
                relation_ids,
            }
        })
        .collect::<Vec<_>>();
    boundaries.sort_by(|left, right| {
        right
            .relations
            .cmp(&left.relations)
            .then_with(|| left.from_root.cmp(&right.from_root))
            .then_with(|| left.to_root.cmp(&right.to_root))
    });
    boundaries.truncate(top);
    boundaries
}

fn api_overview(snapshot: &CanonicalSnapshot) -> ApiOverview {
    let documented = relation_targets(
        snapshot,
        &[
            RelationKind::Documents,
            RelationKind::DocumentsApi,
            RelationKind::DocumentsOperation,
        ],
    );
    let implemented = relation_sources(snapshot, &[RelationKind::ImplementedBy]);

    ApiOverview {
        endpoints: count_kind(snapshot, EntityKind::ApiEndpoint),
        schemas: count_kind(snapshot, EntityKind::ApiSchema),
        examples: count_kind(snapshot, EntityKind::ApiExample),
        documented_endpoints: snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == EntityKind::ApiEndpoint && documented.contains(&entity.id.0)
            })
            .count(),
        implemented_endpoints: snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == EntityKind::ApiEndpoint && implemented.contains(&entity.id.0)
            })
            .count(),
    }
}

fn docs_overview(snapshot: &CanonicalSnapshot) -> DocsOverview {
    DocsOverview {
        pages: count_kind(snapshot, EntityKind::DocumentationPage),
        sections: count_kind(snapshot, EntityKind::DocumentationSection),
        runbooks: count_kind(snapshot, EntityKind::Runbook),
        operation_steps: count_kind(snapshot, EntityKind::OperationStep),
        operations_pages: snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == EntityKind::DocumentationPage
                    && entity
                        .source
                        .as_ref()
                        .is_some_and(|source| source.path.starts_with("docs/operations/"))
            })
            .count(),
    }
}

fn operations_overview(snapshot: &CanonicalSnapshot) -> OperationsOverview {
    OperationsOverview {
        environment_variables: count_kind(snapshot, EntityKind::EnvVar),
        script_commands: count_kind(snapshot, EntityKind::ScriptCommand),
        deployment_resources: count_kind(snapshot, EntityKind::DockerService),
        database_migrations: count_kind(snapshot, EntityKind::DbMigration),
        packages: count_kind(snapshot, EntityKind::Package),
        dependencies: count_kind(snapshot, EntityKind::Dependency),
    }
}

fn graph_hubs(snapshot: &CanonicalSnapshot, top: usize) -> Vec<EntityOverview> {
    let mut degree_by_id = HashMap::<String, usize>::new();
    for relation in &snapshot.relations {
        *degree_by_id.entry(relation.from.0.clone()).or_default() += 1;
        *degree_by_id.entry(relation.to.0.clone()).or_default() += 1;
    }

    let mut hubs = snapshot
        .entities
        .iter()
        .filter_map(|entity| {
            let degree = *degree_by_id.get(&entity.id.0)?;
            (degree > 0).then(|| entity_overview(entity, degree))
        })
        .collect::<Vec<_>>();
    hubs.sort_by(|left, right| {
        right
            .degree
            .cmp(&left.degree)
            .then_with(|| left.stable_key.cmp(&right.stable_key))
    });
    hubs.truncate(top);
    hubs
}

fn diagnostic_overviews(snapshot: &CanonicalSnapshot, top: usize) -> Vec<DiagnosticOverview> {
    let mut diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
        .map(|diagnostic| DiagnosticOverview {
            id: diagnostic.id.0.clone(),
            kind: serialized_name(&diagnostic.kind),
            severity: severity_rank_name(diagnostic.severity).to_string(),
            title: diagnostic.title.clone(),
            source: diagnostic.evidence.iter().find_map(|evidence| {
                evidence.source_file.as_ref().map(|path| match evidence.line_start {
                    Some(line) => format!("{path}:{line}"),
                    None => path.clone(),
                })
            }),
        })
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        severity_rank(&right.severity)
            .cmp(&severity_rank(&left.severity))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
    diagnostics.truncate(top);
    diagnostics
}

fn relation_targets(snapshot: &CanonicalSnapshot, kinds: &[RelationKind]) -> BTreeSet<String> {
    snapshot
        .relations
        .iter()
        .filter(|relation| kinds.contains(&relation.kind))
        .map(|relation| relation.to.0.clone())
        .collect()
}

fn relation_sources(snapshot: &CanonicalSnapshot, kinds: &[RelationKind]) -> BTreeSet<String> {
    snapshot
        .relations
        .iter()
        .filter(|relation| kinds.contains(&relation.kind))
        .map(|relation| relation.from.0.clone())
        .collect()
}

fn count_kind(snapshot: &CanonicalSnapshot, kind: EntityKind) -> usize {
    snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .count()
}

fn entity_overview(entity: &Entity, degree: usize) -> EntityOverview {
    EntityOverview {
        stable_key: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity_source_anchor(entity),
        degree,
    }
}

fn entity_source_anchor(entity: &Entity) -> Option<String> {
    entity.source.as_ref().map(|source| match source.line_start {
        Some(line) => format!("{}:{line}", source.path),
        None => source.path.clone(),
    })
}

fn top_counts(values: impl IntoIterator<Item = String>, top: usize) -> Vec<NamedCount> {
    let mut counts = HashMap::<String, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    let mut counts = counts
        .into_iter()
        .map(|(name, count)| NamedCount { name, count })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    counts.truncate(top);
    counts
}

fn entity_source_root(entity: &Entity) -> Option<String> {
    let path = entity
        .source
        .as_ref()
        .map(|source| source.path.as_str())
        .or_else(|| {
            entity
                .ownership
                .first()
                .map(|owner| owner.source_file.as_str())
        })?;
    let mut parts = path.split('/');
    let first = parts.next()?;
    match parts.next() {
        Some(second) if first == "crates" || first == "apps" || first == "docs" => {
            Some(format!("{first}/{second}"))
        }
        Some(_) => Some(first.to_string()),
        None => Some(path.to_string()),
    }
}

fn serialized_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn severity_rank_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
    }
}

fn severity_rank(severity: &str) -> usize {
    match severity {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::CanonicalSnapshot;
    use athanor_domain::{
        Diagnostic, DiagnosticId, DiagnosticKind, EntityId, Relation, RelationId, RelationStatus,
        SnapshotId, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn builds_bounded_overview_with_hubs_and_counts() {
        let endpoint = entity(
            "ent_endpoint",
            EntityKind::ApiEndpoint,
            "api://GET:/health",
            "health",
            "openapi.yaml",
        );
        let handler = entity(
            "ent_handler",
            EntityKind::Function,
            "rust://src/lib.rs#health",
            "health",
            "src/lib.rs",
        );
        let doc = entity(
            "ent_doc",
            EntityKind::DocumentationPage,
            "doc://docs/api/health.md",
            "Health API",
            "docs/api/health.md",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![endpoint.clone(), handler.clone(), doc.clone()],
            relations: vec![
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
            ],
            diagnostics: vec![diagnostic("diag_docs", "docs/api/health.md")],
            ..CanonicalSnapshot::default()
        };

        let overview = build_repository_overview(&snapshot, 5);

        assert_eq!(overview.snapshot, "snap_test");
        assert_eq!(overview.totals.entities, 3);
        assert_eq!(overview.totals.open_diagnostics, 1);
        assert_eq!(overview.api.endpoints, 1);
        assert_eq!(overview.api.documented_endpoints, 1);
        assert_eq!(overview.api.implemented_endpoints, 1);
        assert_eq!(overview.graph_hubs[0].stable_key, "api://GET:/health");
        assert_eq!(overview.graph_hubs[0].degree, 2);
        assert_eq!(
            overview.open_diagnostics[0].source.as_deref(),
            Some("docs/api/health.md:1")
        );
        assert_eq!(overview.integration_boundaries.len(), 2);
        assert_eq!(overview.integration_boundaries[0].relations, 1);
        assert_eq!(overview.integration_boundaries[0].relation_ids.len(), 1);
    }

    #[test]
    fn summarizes_modules_and_bounds_boundary_relation_ids() {
        let module = entity(
            "ent_module",
            EntityKind::Module,
            "rust://crates/example/src/lib.rs",
            "example",
            "crates/example/src/lib.rs",
        );
        let first = entity(
            "ent_first",
            EntityKind::Function,
            "rust://crates/example/src/lib.rs#first",
            "first",
            "crates/example/src/lib.rs",
        );
        let second = entity(
            "ent_second",
            EntityKind::Function,
            "rust://crates/example/src/lib.rs#second",
            "second",
            "crates/example/src/lib.rs",
        );
        let doc = entity(
            "ent_doc",
            EntityKind::DocumentationPage,
            "doc://docs/example.md",
            "Example",
            "docs/example.md",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![module.clone(), first.clone(), second.clone(), doc.clone()],
            relations: vec![
                relation("rel_define_first", RelationKind::Defines, &module, &first),
                relation("rel_define_second", RelationKind::Defines, &module, &second),
                relation("rel_doc_first", RelationKind::Documents, &doc, &first),
                relation("rel_doc_second", RelationKind::Documents, &doc, &second),
            ],
            ..CanonicalSnapshot::default()
        };

        let overview = build_repository_overview(&snapshot, 1);

        assert_eq!(overview.module_structure.len(), 1);
        assert_eq!(overview.module_structure[0].direct_members, 2);
        assert_eq!(
            overview.module_structure[0].relation_ids,
            vec!["rel_define_first"]
        );
        assert_eq!(overview.module_structure[0].omitted_relation_ids, 1);
        assert_eq!(overview.integration_boundaries.len(), 1);
        assert_eq!(
            overview.integration_boundaries[0].from_root,
            "docs/example.md"
        );
        assert_eq!(overview.integration_boundaries[0].to_root, "crates/example");
        assert_eq!(overview.integration_boundaries[0].relations, 2);
        assert_eq!(
            overview.integration_boundaries[0].relation_ids,
            vec!["rel_doc_first"]
        );
        assert_eq!(overview.integration_boundaries[0].omitted_relation_ids, 1);
    }

    fn entity(id: &str, kind: EntityKind, stable_key: &str, name: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }

    fn diagnostic(id: &str, path: &str) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind: DiagnosticKind::MissingDocumentation,
            severity: Severity::High,
            status: DiagnosticStatus::Open,
            title: "Missing docs".to_string(),
            message: "Missing docs".to_string(),
            entities: Vec::new(),
            evidence: vec![athanor_domain::Evidence {
                source_file: Some(path.to_string()),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: athanor_domain::EvidenceStatus::Missing,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({}),
        }
    }
}
