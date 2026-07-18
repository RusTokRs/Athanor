use std::collections::{BTreeMap, BTreeSet, HashMap};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{DiagnosticStatus, Entity, EntityKind, RelationKind, Severity};

use super::model::{
    ApiOverview, DiagnosticOverview, DocsOverview, EntityOverview, IntegrationBoundaryOverview,
    ModuleOverview, NamedCount, OperationsOverview, OverviewTotals, RepositoryOverview,
    OVERVIEW_SCHEMA,
};

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
