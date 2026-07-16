use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::config::load_config;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Diagnostic, Entity, EntityId, Relation, RelationKind};
use serde::{Deserialize, Serialize};

use crate::RuntimeComposition;
use crate::index_state::IndexStateStore;
use crate::json_contract::IMPACT_ANALYSIS_SCHEMA_V1;
use crate::local_source::discover_source_files;
use crate::project_path::normalize_canonical_path;

#[derive(Debug, Clone, Serialize)]
pub struct RelationFlow {
    pub relation: Relation,
    pub direction: FlowDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactedEntity {
    pub entity: Entity,
    pub depth: usize,
    pub path: Vec<RelationFlow>,
    pub path_steps: Vec<ImpactPathStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactPathStep {
    pub relation_id: String,
    pub relation_kind: String,
    pub direction: FlowDirection,
    pub from: ImpactPathEndpoint,
    pub to: ImpactPathEndpoint,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactPathEndpoint {
    pub entity_id: String,
    pub stable_key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactAnalysis {
    pub schema: String,
    pub snapshot: String,
    pub starting_entities: Vec<Entity>,
    pub impacted_entities: Vec<ImpactedEntity>,
    pub impacted_files: Vec<String>,
    pub impacted_diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct ImpactOptions {
    pub root: PathBuf,
    pub target: Option<String>,
    pub diff: bool,
    pub max_depth: usize,
}

pub async fn impact_project(options: ImpactOptions) -> Result<ImpactAnalysis> {
    impact_project_inner(options, None).await
}

/// Analyses impact with explicitly supplied runtime dependencies.
pub async fn impact_project_with_composition(
    options: ImpactOptions,
    composition: &RuntimeComposition,
) -> Result<ImpactAnalysis> {
    impact_project_inner(options, Some(composition)).await
}

async fn impact_project_inner(
    options: ImpactOptions,
    composition: Option<&RuntimeComposition>,
) -> Result<ImpactAnalysis> {
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

    // 1. Resolve starting entities
    let starting_entities = if options.diff {
        let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
        let previous_state = state_store.load().context("failed to load index state")?;

        let current_files = discover_source_files(&root)
            .context("failed to discover source files for diff comparison")?;

        let affected_files = previous_state.affected_files(&current_files);

        let mut diff_files = HashSet::new();
        diff_files.extend(affected_files.changed);
        diff_files.extend(affected_files.removed);

        if diff_files.is_empty() {
            println!(
                "No changed files detected in the working tree compared to the last index run."
            );
            return Ok(ImpactAnalysis {
                schema: IMPACT_ANALYSIS_SCHEMA_V1.to_string(),
                snapshot: snapshot
                    .snapshot
                    .as_ref()
                    .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
                starting_entities: Vec::new(),
                impacted_entities: Vec::new(),
                impacted_files: Vec::new(),
                impacted_diagnostics: Vec::new(),
            });
        }

        snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity
                    .ownership
                    .iter()
                    .any(|o| diff_files.contains(&o.source_file))
                    || entity
                        .source
                        .as_ref()
                        .is_some_and(|s| diff_files.contains(&s.path))
            })
            .cloned()
            .collect::<Vec<_>>()
    } else if let Some(target) = &options.target {
        // Try stable key match first
        let mut resolved = snapshot
            .entities
            .iter()
            .filter(|entity| entity.stable_key.0 == *target)
            .cloned()
            .collect::<Vec<_>>();

        if resolved.is_empty() {
            // Try path matching
            if let Some(rel_path) = resolve_project_relative_path(&root, target) {
                resolved = snapshot
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity.ownership.iter().any(|o| o.source_file == rel_path)
                            || entity.source.as_ref().is_some_and(|s| s.path == rel_path)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
            }
        }

        if resolved.is_empty() {
            bail!(
                "could not resolve target \"{}\" to any canonical entities in the latest snapshot",
                target
            );
        }
        resolved
    } else {
        bail!("either a target stable-key/path or the --diff flag must be provided");
    };

    // 2. Perform graph traversal (BFS)
    let analysis = impact_snapshot(&snapshot, starting_entities, options.max_depth);
    Ok(analysis)
}

pub fn impact_snapshot(
    snapshot: &CanonicalSnapshot,
    starting_entities: Vec<Entity>,
    max_depth: usize,
) -> ImpactAnalysis {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let entity_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), entity))
        .collect::<HashMap<_, _>>();

    for entity in &starting_entities {
        queue.push_back((entity.id.clone(), 0, Vec::new()));
        visited.insert(entity.id.clone());
    }

    let mut impacted_entities = Vec::new();

    while let Some((current_id, depth, path)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        for relation in &snapshot.relations {
            let (from_to, to_from) = propagates_impact(&relation.kind);

            if from_to && relation.from == current_id {
                let next_id = relation.to.clone();
                if !visited.contains(&next_id) {
                    visited.insert(next_id.clone());
                    let mut next_path = path.clone();
                    next_path.push(RelationFlow {
                        relation: relation.clone(),
                        direction: FlowDirection::Forward,
                    });
                    queue.push_back((next_id.clone(), depth + 1, next_path.clone()));
                    if let Some(next_entity) = snapshot.entities.iter().find(|e| e.id == next_id) {
                        impacted_entities.push(ImpactedEntity {
                            entity: next_entity.clone(),
                            depth: depth + 1,
                            path_steps: impact_path_steps(&next_path, &entity_by_id),
                            path: next_path,
                        });
                    }
                }
            }

            if to_from && relation.to == current_id {
                let next_id = relation.from.clone();
                if !visited.contains(&next_id) {
                    visited.insert(next_id.clone());
                    let mut next_path = path.clone();
                    next_path.push(RelationFlow {
                        relation: relation.clone(),
                        direction: FlowDirection::Backward,
                    });
                    queue.push_back((next_id.clone(), depth + 1, next_path.clone()));
                    if let Some(next_entity) = snapshot.entities.iter().find(|e| e.id == next_id) {
                        impacted_entities.push(ImpactedEntity {
                            entity: next_entity.clone(),
                            depth: depth + 1,
                            path_steps: impact_path_steps(&next_path, &entity_by_id),
                            path: next_path,
                        });
                    }
                }
            }
        }
    }

    // Sort impacted entities by depth, then by kind, then by stable key for determinism
    impacted_entities.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| format!("{:?}", a.entity.kind).cmp(&format!("{:?}", b.entity.kind)))
            .then_with(|| a.entity.stable_key.0.cmp(&b.entity.stable_key.0))
    });

    // 3. Gather impacted diagnostics
    let all_affected_ids: HashSet<&EntityId> = starting_entities
        .iter()
        .map(|e| &e.id)
        .chain(impacted_entities.iter().map(|i| &i.entity.id))
        .collect();

    let mut impacted_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diag| diag.entities.iter().any(|id| all_affected_ids.contains(id)))
        .cloned()
        .collect::<Vec<_>>();
    impacted_diagnostics.sort_by(|a, b| a.id.0.cmp(&b.id.0));

    // 4. Gather unique files
    let mut impacted_files_set = HashSet::new();
    for entity in starting_entities
        .iter()
        .chain(impacted_entities.iter().map(|i| &i.entity))
    {
        for ownership in &entity.ownership {
            impacted_files_set.insert(ownership.source_file.clone());
        }
        if let Some(source) = &entity.source {
            impacted_files_set.insert(source.path.clone());
        }
    }
    let mut impacted_files: Vec<String> = impacted_files_set.into_iter().collect();
    impacted_files.sort();

    ImpactAnalysis {
        schema: IMPACT_ANALYSIS_SCHEMA_V1.to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        starting_entities,
        impacted_entities,
        impacted_files,
        impacted_diagnostics,
    }
}

fn propagates_impact(kind: &RelationKind) -> (bool, bool) {
    match kind {
        RelationKind::Calls
        | RelationKind::Imports
        | RelationKind::ImplementedBy
        | RelationKind::Documents
        | RelationKind::DocumentsApi
        | RelationKind::DocumentsOperation
        | RelationKind::SchemaForRequest
        | RelationKind::SchemaForResponse
        | RelationKind::Contains => (false, true), // to -> from propagation

        RelationKind::TestedBy | RelationKind::CoveredByTest | RelationKind::Defines => {
            (true, false)
        } // from -> to propagation

        _ => (false, false),
    }
}

fn impact_path_steps(
    path: &[RelationFlow],
    entity_by_id: &HashMap<EntityId, &Entity>,
) -> Vec<ImpactPathStep> {
    path.iter()
        .map(|flow| {
            let (from_id, to_id) = match flow.direction {
                FlowDirection::Forward => (&flow.relation.from, &flow.relation.to),
                FlowDirection::Backward => (&flow.relation.to, &flow.relation.from),
            };

            ImpactPathStep {
                relation_id: flow.relation.id.0.clone(),
                relation_kind: serialized_relation_kind(&flow.relation.kind),
                direction: flow.direction,
                from: impact_path_endpoint(from_id, entity_by_id),
                to: impact_path_endpoint(to_id, entity_by_id),
            }
        })
        .collect()
}

fn impact_path_endpoint(
    id: &EntityId,
    entity_by_id: &HashMap<EntityId, &Entity>,
) -> ImpactPathEndpoint {
    entity_by_id.get(id).map_or_else(
        || ImpactPathEndpoint {
            entity_id: id.0.clone(),
            stable_key: id.0.clone(),
            name: id.0.clone(),
        },
        |entity| ImpactPathEndpoint {
            entity_id: entity.id.0.clone(),
            stable_key: entity.stable_key.0.clone(),
            name: entity.name.clone(),
        },
    )
}

fn serialized_relation_kind(kind: &RelationKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn resolve_project_relative_path(root: &Path, target: &str) -> Option<String> {
    let target_path = Path::new(target);
    let absolute_target = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        root.join(target_path)
    };

    if let Some(relative) = absolute_target
        .canonicalize()
        .ok()
        .and_then(|canonical| canonical.strip_prefix(root).ok().map(|p| p.to_path_buf()))
    {
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }

    // Fallback normalization
    let normalized = target.replace('\\', "/");
    let trimmed = normalized.strip_prefix("./").unwrap_or(&normalized);
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityId, EntityKind, Evidence,
        EvidenceStatus, RelationId, RelationStatus, Severity, SnapshotId, StableKey,
    };
    use serde_json::json;

    use super::*;

    fn entity(id: &str, stable_key: &str, kind: EntityKind) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: stable_key.to_string(),
            title: None,
            source: None,
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
            evidence: vec![Evidence {
                source_file: None,
                line_start: None,
                line_end: None,
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }

    #[test]
    fn test_traverses_impact_correctly() {
        let callee = entity("callee_id", "symbol://rust:callee", EntityKind::Function);
        let caller = entity("caller_id", "symbol://rust:caller", EntityKind::Function);
        let endpoint = entity("endpoint_id", "api://POST:/login", EntityKind::ApiEndpoint);
        let doc = entity(
            "doc_id",
            "doc://docs/login.md",
            EntityKind::DocumentationPage,
        );
        let test = entity("test_id", "symbol://rust:test_login", EntityKind::TestCase);

        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![
                callee.clone(),
                caller.clone(),
                endpoint.clone(),
                doc.clone(),
                test.clone(),
            ],
            facts: Vec::new(),
            relations: vec![
                relation("rel_call", RelationKind::Calls, &caller, &callee),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &caller),
                relation("rel_doc", RelationKind::Documents, &doc, &endpoint),
                relation("rel_test", RelationKind::TestedBy, &caller, &test),
            ],
            diagnostics: vec![Diagnostic {
                id: DiagnosticId("diag_1".to_string()),
                kind: DiagnosticKind::UncoveredSymbol,
                severity: Severity::Low,
                status: DiagnosticStatus::Open,
                title: "Uncovered symbol".to_string(),
                message: "Callee not covered by tests".to_string(),
                entities: vec![callee.id.clone()],
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_test".to_string()),
                suggested_fix: None,
                payload: json!({}),
            }],
        };

        // If callee changes:
        // -> caller changes (Calls: callee -> caller)
        // -> endpoint changes (ImplementedBy: caller -> endpoint)
        // -> doc changes (Documents: endpoint -> doc)
        // -> test changes (TestedBy: caller -> test)
        let analysis = impact_snapshot(&snapshot, vec![callee.clone()], 10);

        assert_eq!(analysis.starting_entities.len(), 1);
        assert_eq!(analysis.starting_entities[0].id, callee.id);

        let impacted_keys: Vec<String> = analysis
            .impacted_entities
            .iter()
            .map(|i| i.entity.stable_key.0.clone())
            .collect();

        assert!(impacted_keys.contains(&caller.stable_key.0));
        assert!(impacted_keys.contains(&endpoint.stable_key.0));
        assert!(impacted_keys.contains(&doc.stable_key.0));
        assert!(impacted_keys.contains(&test.stable_key.0));

        assert_eq!(analysis.impacted_diagnostics.len(), 1);
        assert_eq!(analysis.impacted_diagnostics[0].id.0, "diag_1");

        let doc_impact = analysis
            .impacted_entities
            .iter()
            .find(|impact| impact.entity.id == doc.id)
            .expect("documentation page should be impacted through endpoint");
        assert_eq!(doc_impact.path_steps.len(), 3);
        assert_eq!(doc_impact.path_steps[0].relation_id, "rel_call");
        assert_eq!(doc_impact.path_steps[0].relation_kind, "calls");
        assert_eq!(
            doc_impact.path_steps[0].from.stable_key,
            callee.stable_key.0
        );
        assert_eq!(doc_impact.path_steps[0].to.stable_key, caller.stable_key.0);
        assert_eq!(doc_impact.path_steps[2].relation_id, "rel_doc");
        assert_eq!(
            doc_impact.path_steps[2].from.stable_key,
            endpoint.stable_key.0
        );
        assert_eq!(doc_impact.path_steps[2].to.stable_key, doc.stable_key.0);
    }
}
