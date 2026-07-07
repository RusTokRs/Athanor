use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticStatus, Entity, EntityId, EntityKind, Evidence,
    EvidenceStatus, Relation, RelationKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::load_config;
use crate::impact::FlowDirection;
use crate::index_state::IndexStateStore;
use crate::local_source::discover_source_files;
use crate::project_path::normalize_canonical_path;
use crate::search::search_snapshot;
use crate::store::init_store;

const DEFAULT_MAX_CANDIDATES: usize = 5_000;

#[derive(Debug, Clone)]
pub struct ChangeMapOptions {
    pub root: PathBuf,
    pub task: Option<String>,
    pub target: Option<String>,
    pub diff: bool,
    pub max_entities: usize,
    pub max_files: usize,
    pub max_diagnostics: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeMapReport {
    pub schema: String,
    pub snapshot: String,
    pub query: ChangeMapQuery,
    pub limits: ChangeMapLimits,
    pub returned: ChangeMapCounts,
    pub omitted: ChangeMapCounts,
    pub items: Vec<ChangeMapItem>,
    pub files: Vec<ChangeMapFile>,
    pub diagnostics: Vec<Diagnostic>,
    pub completeness: ChangeMapCompleteness,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapQuery {
    pub task: Option<String>,
    pub target: Option<String>,
    pub diff: bool,
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapLimits {
    pub max_entities: usize,
    pub max_files: usize,
    pub max_diagnostics: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapCounts {
    pub entities: usize,
    pub files: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeMapItem {
    pub rank: usize,
    pub score: i64,
    pub depth: usize,
    pub entity: Entity,
    pub reasons: Vec<String>,
    pub path: Vec<ChangeMapPathStep>,
    pub files: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub diagnostics: Vec<DiagnosticId>,
    pub test_coverage: ChangeMapTestCoverage,
    pub annotations: Vec<ChangeMapAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeMapPathStep {
    pub relation_id: String,
    pub relation_kind: String,
    pub direction: FlowDirection,
    pub from: ChangeMapEndpoint,
    pub to: ChangeMapEndpoint,
    pub confidence: f32,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapEndpoint {
    pub entity_id: String,
    pub stable_key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeMapTestStatus {
    Linked,
    NotLinked,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapTestCoverage {
    pub status: ChangeMapTestStatus,
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeMapAnnotation {
    pub source: String,
    pub schema: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapFile {
    pub rank: usize,
    pub path: String,
    pub score: i64,
    pub entity_kinds: Vec<String>,
    pub stable_keys: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeMapCompleteness {
    pub candidate_limit_reached: bool,
    pub candidate_limit: usize,
    pub note: String,
    pub suggested_command: String,
}

#[derive(Debug, Clone)]
struct Seed {
    id: EntityId,
    score: i64,
    reason: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    id: EntityId,
    seed_score: i64,
    depth: usize,
    reasons: BTreeSet<String>,
    path: Vec<PathLink>,
}

#[derive(Debug, Clone)]
struct PathLink {
    relation: Relation,
    direction: FlowDirection,
    from: EntityId,
    to: EntityId,
}

pub async fn change_map_project(options: ChangeMapOptions) -> Result<ChangeMapReport> {
    validate_options(&options)?;
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;

    let mut seeds = Vec::new();
    let mut changed_files = BTreeSet::new();

    if let Some(target) = options.target.as_deref() {
        let resolved = resolve_target_entities(&root, &snapshot, target);
        if resolved.is_empty() {
            bail!("could not resolve target `{target}` to a canonical stable key or source path");
        }
        seeds.extend(resolved.into_iter().map(|entity| Seed {
            id: entity.id.clone(),
            score: 1_200,
            reason: format!("explicit target `{target}`"),
        }));
    }

    if options.diff {
        let state = IndexStateStore::new(root.join(".athanor/state/index-state.json"))
            .load()
            .context("failed to load index state")?;
        let current = discover_source_files(&root)
            .context("failed to discover source files for change-map diff")?;
        let affected = state.affected_files(&current);
        changed_files.extend(affected.changed);
        changed_files.extend(affected.removed);
        for entity in snapshot.entities.iter().filter(|entity| {
            entity_files(entity)
                .iter()
                .any(|path| changed_files.contains(path))
        }) {
            seeds.push(Seed {
                id: entity.id.clone(),
                score: 1_100,
                reason: "owned by a changed or removed file".to_string(),
            });
        }
    }

    if let Some(task) = options
        .task
        .as_deref()
        .filter(|task| !task.trim().is_empty())
    {
        let search_limit = options.max_entities.saturating_mul(4).clamp(10, 200);
        let search = search_snapshot(&root, &snapshot, task.to_string(), search_limit).await?;
        for (rank, result) in search.results.into_iter().enumerate() {
            seeds.push(Seed {
                id: result.entity_id,
                score: 1_000_i64.saturating_sub((rank as i64) * 10),
                reason: format!("task search matched `{task}`"),
            });
        }
    }

    let query = ChangeMapQuery {
        task: options.task.filter(|task| !task.trim().is_empty()),
        target: options.target,
        diff: options.diff,
        changed_files: changed_files.into_iter().collect(),
    };
    Ok(build_change_map(
        &snapshot,
        query,
        seeds,
        ChangeMapLimits {
            max_entities: options.max_entities,
            max_files: options.max_files,
            max_diagnostics: options.max_diagnostics,
            max_depth: options.max_depth,
        },
    ))
}

fn validate_options(options: &ChangeMapOptions) -> Result<()> {
    let has_task = options
        .task
        .as_ref()
        .is_some_and(|task| !task.trim().is_empty());
    if !has_task && options.target.is_none() && !options.diff {
        bail!("change-map requires a task, --target, or --diff");
    }
    if options.max_entities == 0 || options.max_files == 0 || options.max_diagnostics == 0 {
        bail!("change-map entity, file, and diagnostic limits must be greater than zero");
    }
    Ok(())
}

fn resolve_target_entities<'a>(
    root: &Path,
    snapshot: &'a CanonicalSnapshot,
    target: &str,
) -> Vec<&'a Entity> {
    let exact = snapshot
        .entities
        .iter()
        .filter(|entity| entity.stable_key.0 == target)
        .collect::<Vec<_>>();
    if !exact.is_empty() {
        return exact;
    }
    let path = normalize_target_path(root, target);
    snapshot
        .entities
        .iter()
        .filter(|entity| entity_files(entity).iter().any(|file| file == &path))
        .collect()
}

fn normalize_target_path(root: &Path, target: &str) -> String {
    let candidate = Path::new(target);
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    absolute
        .canonicalize()
        .ok()
        .and_then(|path| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            target
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_string()
        })
}

fn build_change_map(
    snapshot: &CanonicalSnapshot,
    query: ChangeMapQuery,
    seeds: Vec<Seed>,
    limits: ChangeMapLimits,
) -> ChangeMapReport {
    let entities = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), entity))
        .collect::<HashMap<_, _>>();
    let mut adjacency = HashMap::<EntityId, Vec<&Relation>>::new();
    let mut sorted_relations = snapshot.relations.iter().collect::<Vec<_>>();
    sorted_relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    for relation in sorted_relations {
        adjacency
            .entry(relation.from.clone())
            .or_default()
            .push(relation);
        adjacency
            .entry(relation.to.clone())
            .or_default()
            .push(relation);
    }

    let mut seed_map = BTreeMap::<String, Seed>::new();
    for seed in seeds {
        seed_map
            .entry(seed.id.0.clone())
            .and_modify(|current| {
                if seed.score > current.score {
                    current.score = seed.score;
                }
                if !current.reason.contains(&seed.reason) {
                    current.reason = format!("{}; {}", current.reason, seed.reason);
                }
            })
            .or_insert(seed);
    }

    let candidate_limit = DEFAULT_MAX_CANDIDATES.max(limits.max_entities.saturating_mul(50));
    let mut candidates = HashMap::<EntityId, Candidate>::new();
    let mut queue = VecDeque::new();
    for seed in seed_map.into_values() {
        if !entities.contains_key(&seed.id) {
            continue;
        }
        let mut reasons = BTreeSet::new();
        reasons.insert(seed.reason);
        candidates.insert(
            seed.id.clone(),
            Candidate {
                id: seed.id.clone(),
                seed_score: seed.score,
                depth: 0,
                reasons,
                path: Vec::new(),
            },
        );
        queue.push_back(seed.id);
    }

    let mut candidate_limit_reached = false;
    while let Some(current_id) = queue.pop_front() {
        let current = candidates
            .get(&current_id)
            .cloned()
            .expect("queued change-map candidate must exist");
        if current.depth >= limits.max_depth {
            continue;
        }
        for relation in adjacency.get(&current_id).into_iter().flatten() {
            let (next, direction) = if relation.from == current_id {
                (relation.to.clone(), FlowDirection::Forward)
            } else {
                (relation.from.clone(), FlowDirection::Backward)
            };
            if candidates.contains_key(&next) || !entities.contains_key(&next) {
                continue;
            }
            if candidates.len() >= candidate_limit {
                candidate_limit_reached = true;
                break;
            }
            let mut path = current.path.clone();
            path.push(PathLink {
                relation: (*relation).clone(),
                direction,
                from: current_id.clone(),
                to: next.clone(),
            });
            let mut reasons = BTreeSet::new();
            reasons.insert(format!(
                "connected through `{}`",
                serialized_relation_kind(&relation.kind)
            ));
            candidates.insert(
                next.clone(),
                Candidate {
                    id: next.clone(),
                    seed_score: current.seed_score,
                    depth: current.depth + 1,
                    reasons,
                    path,
                },
            );
            queue.push_back(next);
        }
        if candidate_limit_reached {
            break;
        }
    }

    let diagnostics_by_entity = diagnostics_by_entity(snapshot);
    let tests_by_entity = tests_by_entity(snapshot, &entities);
    let mut ranked = candidates
        .into_values()
        .filter_map(|candidate| {
            let entity = entities.get(&candidate.id)?;
            let diagnostic_count = diagnostics_by_entity.get(&candidate.id).map_or(0, Vec::len);
            let score = candidate.seed_score - (candidate.depth as i64 * 100)
                + candidate
                    .path
                    .iter()
                    .map(|link| {
                        relation_weight(&link.relation.kind)
                            + (link.relation.confidence.clamp(0.0, 1.0) * 10.0).round() as i64
                    })
                    .sum::<i64>()
                + (diagnostic_count as i64 * 20);
            Some((score, candidate, *entity))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, candidate, entity)| {
        (
            Reverse(*score),
            candidate.depth,
            entity.stable_key.0.clone(),
        )
    });
    let ranked_count = ranked.len();
    let ranked = diversify_ranked_entities(ranked, limits.max_entities);
    let entity_omitted = ranked_count.saturating_sub(ranked.len());

    let mut all_diagnostic_ids = BTreeSet::new();
    let mut items = ranked
        .into_iter()
        .enumerate()
        .map(|(index, (score, candidate, entity))| {
            let diagnostics = diagnostics_by_entity
                .get(&entity.id)
                .cloned()
                .unwrap_or_default();
            all_diagnostic_ids.extend(diagnostics.iter().map(|diagnostic| diagnostic.id.0.clone()));
            build_item(
                index + 1,
                score,
                candidate,
                entity,
                &entities,
                diagnostics,
                tests_by_entity.get(&entity.id).cloned().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    let mut diagnostic_candidates = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| all_diagnostic_ids.contains(&diagnostic.id.0))
        .cloned()
        .collect::<Vec<_>>();
    diagnostic_candidates.sort_by_key(|diagnostic| {
        (
            Reverse(severity_weight(diagnostic)),
            diagnostic.id.0.clone(),
        )
    });
    let diagnostic_omitted = diagnostic_candidates
        .len()
        .saturating_sub(limits.max_diagnostics);
    let diagnostics = diagnostic_candidates
        .into_iter()
        .take(limits.max_diagnostics)
        .collect::<Vec<_>>();
    let returned_diagnostic_ids = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id.clone())
        .collect::<HashSet<_>>();
    for item in &mut items {
        item.diagnostics
            .retain(|id| returned_diagnostic_ids.contains(id));
    }

    let (files, file_omitted) = build_files(&items, limits.max_files);
    ChangeMapReport {
        schema: "athanor.change_map.v1".to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        query,
        limits,
        returned: ChangeMapCounts {
            entities: items.len(),
            files: files.len(),
            diagnostics: diagnostics.len(),
        },
        omitted: ChangeMapCounts {
            entities: entity_omitted,
            files: file_omitted,
            diagnostics: diagnostic_omitted,
        },
        items,
        files,
        diagnostics,
        completeness: ChangeMapCompleteness {
            candidate_limit_reached,
            candidate_limit,
            note: "Results reflect relations emitted by enabled adapters; absent links are not proof that no dependency exists.".to_string(),
            suggested_command: "ath coverage --json".to_string(),
        },
    }
}

fn diversify_ranked_entities(
    ranked: Vec<(i64, Candidate, &Entity)>,
    limit: usize,
) -> Vec<(i64, Candidate, &Entity)> {
    if ranked.len() <= limit {
        return ranked;
    }
    let mut selected = vec![false; ranked.len()];
    let mut selected_count = 0;
    let mut covered_files = HashSet::new();
    for (index, (_, _, entity)) in ranked.iter().enumerate() {
        let files = entity_files(entity);
        if files.is_empty() || files.iter().all(|path| covered_files.contains(path)) {
            continue;
        }
        selected[index] = true;
        selected_count += 1;
        covered_files.extend(files);
        if selected_count == limit {
            break;
        }
    }
    if selected_count < limit {
        for is_selected in &mut selected {
            if !*is_selected {
                *is_selected = true;
                selected_count += 1;
                if selected_count == limit {
                    break;
                }
            }
        }
    }
    ranked
        .into_iter()
        .zip(selected)
        .filter_map(|(item, selected)| selected.then_some(item))
        .collect()
}

fn build_item(
    rank: usize,
    score: i64,
    candidate: Candidate,
    entity: &Entity,
    entities: &HashMap<EntityId, &Entity>,
    diagnostics: Vec<&Diagnostic>,
    tests: Vec<String>,
) -> ChangeMapItem {
    let mut reasons = candidate.reasons.into_iter().collect::<Vec<_>>();
    let test_coverage = if test_coverage_applies(&entity.kind) {
        if tests.is_empty() {
            reasons.push("no linked test relation was found".to_string());
            ChangeMapTestCoverage {
                status: ChangeMapTestStatus::NotLinked,
                tests,
            }
        } else {
            ChangeMapTestCoverage {
                status: ChangeMapTestStatus::Linked,
                tests,
            }
        }
    } else {
        ChangeMapTestCoverage {
            status: ChangeMapTestStatus::NotApplicable,
            tests,
        }
    };
    let path = candidate
        .path
        .iter()
        .map(|link| ChangeMapPathStep {
            relation_id: link.relation.id.0.clone(),
            relation_kind: serialized_relation_kind(&link.relation.kind),
            direction: link.direction,
            from: endpoint(&link.from, entities),
            to: endpoint(&link.to, entities),
            confidence: link.relation.confidence,
            evidence: link.relation.evidence.clone(),
        })
        .collect::<Vec<_>>();
    let mut evidence = entity_evidence(entity);
    for link in &candidate.path {
        evidence.extend(link.relation.evidence.iter().cloned());
    }
    for diagnostic in &diagnostics {
        evidence.extend(diagnostic.evidence.iter().cloned());
    }
    dedupe_evidence(&mut evidence);

    let mut annotations = annotations_from_payload(&entity.payload, "entity");
    for link in &candidate.path {
        annotations.extend(annotations_from_payload(
            &link.relation.payload,
            &serialized_relation_kind(&link.relation.kind),
        ));
    }
    annotations.sort();
    annotations.dedup();

    ChangeMapItem {
        rank,
        score,
        depth: candidate.depth,
        entity: entity.clone(),
        reasons,
        path,
        files: entity_files(entity),
        evidence,
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.id.clone())
            .collect(),
        test_coverage,
        annotations,
    }
}

fn build_files(items: &[ChangeMapItem], limit: usize) -> (Vec<ChangeMapFile>, usize) {
    let mut rows =
        BTreeMap::<String, (i64, BTreeSet<String>, BTreeSet<String>, BTreeSet<String>)>::new();
    for item in items {
        for path in &item.files {
            let row = rows.entry(path.clone()).or_insert_with(|| {
                (
                    item.score,
                    BTreeSet::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                )
            });
            row.0 = row.0.max(item.score);
            row.1.insert(serialized_entity_kind(&item.entity.kind));
            row.2.insert(item.entity.stable_key.0.clone());
            row.3.extend(item.reasons.iter().cloned());
        }
    }
    let mut files = rows.into_iter().collect::<Vec<_>>();
    files.sort_by_key(|(path, (score, _, _, _))| (Reverse(*score), path.clone()));
    let omitted = files.len().saturating_sub(limit);
    (
        files
            .into_iter()
            .take(limit)
            .enumerate()
            .map(
                |(rank, (path, (score, kinds, keys, reasons)))| ChangeMapFile {
                    rank: rank + 1,
                    path,
                    score,
                    entity_kinds: kinds.into_iter().collect(),
                    stable_keys: keys.into_iter().collect(),
                    reasons: reasons.into_iter().collect(),
                },
            )
            .collect(),
        omitted,
    )
}

fn diagnostics_by_entity(snapshot: &CanonicalSnapshot) -> HashMap<EntityId, Vec<&Diagnostic>> {
    let mut result = HashMap::<EntityId, Vec<&Diagnostic>>::new();
    for diagnostic in snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.status == DiagnosticStatus::Open)
    {
        for entity in &diagnostic.entities {
            result.entry(entity.clone()).or_default().push(diagnostic);
        }
    }
    for diagnostics in result.values_mut() {
        diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    }
    result
}

fn tests_by_entity(
    snapshot: &CanonicalSnapshot,
    entities: &HashMap<EntityId, &Entity>,
) -> HashMap<EntityId, Vec<String>> {
    let mut result = HashMap::<EntityId, BTreeSet<String>>::new();
    for relation in &snapshot.relations {
        if !matches!(
            relation.kind,
            RelationKind::TestedBy | RelationKind::CoveredByTest
        ) {
            continue;
        }
        let (subject, test) = if entities
            .get(&relation.to)
            .is_some_and(|entity| entity.kind == EntityKind::TestCase)
        {
            (&relation.from, &relation.to)
        } else if entities
            .get(&relation.from)
            .is_some_and(|entity| entity.kind == EntityKind::TestCase)
        {
            (&relation.to, &relation.from)
        } else {
            (&relation.from, &relation.to)
        };
        if let Some(test) = entities.get(test) {
            result
                .entry(subject.clone())
                .or_default()
                .insert(test.stable_key.0.clone());
        }
    }
    result
        .into_iter()
        .map(|(id, tests)| (id, tests.into_iter().collect()))
        .collect()
}

fn entity_files(entity: &Entity) -> Vec<String> {
    let mut files = entity
        .ownership
        .iter()
        .map(|ownership| ownership.source_file.clone())
        .collect::<BTreeSet<_>>();
    if let Some(source) = &entity.source {
        files.insert(source.path.clone());
    }
    files.into_iter().collect()
}

fn entity_evidence(entity: &Entity) -> Vec<Evidence> {
    if let Some(source) = &entity.source {
        return vec![Evidence {
            source_file: Some(source.path.clone()),
            line_start: source.line_start,
            line_end: source.line_end,
            extractor: Some("canonical_entity".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        }];
    }
    entity
        .ownership
        .iter()
        .map(|ownership| Evidence {
            source_file: Some(ownership.source_file.clone()),
            line_start: None,
            line_end: None,
            extractor: Some("canonical_ownership".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        })
        .collect()
}

fn dedupe_evidence(evidence: &mut Vec<Evidence>) {
    evidence.sort_by_key(|item| {
        (
            item.source_file.clone(),
            item.line_start,
            item.line_end,
            item.extractor.clone(),
        )
    });
    evidence.dedup_by(|left, right| {
        left.source_file == right.source_file
            && left.line_start == right.line_start
            && left.line_end == right.line_end
            && left.extractor == right.extractor
    });
}

fn annotations_from_payload(payload: &Value, message: &str) -> Vec<ChangeMapAnnotation> {
    let Some(schema) = payload.get("schema").and_then(Value::as_str) else {
        return Vec::new();
    };
    let source = schema.split('.').next().unwrap_or("adapter");
    if matches!(source, "athanor" | "") {
        return Vec::new();
    }
    vec![ChangeMapAnnotation {
        source: source.to_string(),
        schema: schema.to_string(),
        message: format!("{source} adapter context from {message}"),
    }]
}

fn endpoint(id: &EntityId, entities: &HashMap<EntityId, &Entity>) -> ChangeMapEndpoint {
    entities.get(id).map_or_else(
        || ChangeMapEndpoint {
            entity_id: id.0.clone(),
            stable_key: id.0.clone(),
            name: id.0.clone(),
        },
        |entity| ChangeMapEndpoint {
            entity_id: entity.id.0.clone(),
            stable_key: entity.stable_key.0.clone(),
            name: entity.name.clone(),
        },
    )
}

fn test_coverage_applies(kind: &EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Symbol
            | EntityKind::Function
            | EntityKind::Class
            | EntityKind::Module
            | EntityKind::ApiEndpoint
            | EntityKind::ApiSchema
            | EntityKind::DbTable
            | EntityKind::DbMigration
            | EntityKind::Feature
    ) || matches!(kind, EntityKind::Other(_))
}

fn relation_weight(kind: &RelationKind) -> i64 {
    match kind {
        RelationKind::Calls
        | RelationKind::ImplementedBy
        | RelationKind::Implements
        | RelationKind::SchemaForRequest
        | RelationKind::SchemaForResponse
        | RelationKind::RequiresAuth
        | RelationKind::RequiresPermission
        | RelationKind::QueriesTable => 90,
        RelationKind::Imports
        | RelationKind::TestedBy
        | RelationKind::CoveredByTest
        | RelationKind::DeclaredInOpenapi
        | RelationKind::ChangedWith => 80,
        RelationKind::Documents
        | RelationKind::DocumentsApi
        | RelationKind::DocumentsOperation
        | RelationKind::UsesEnv
        | RelationKind::ExampleFor => 60,
        RelationKind::Contains | RelationKind::Defines => 30,
        RelationKind::Other(_) => 75,
        _ => 50,
    }
}

fn severity_weight(diagnostic: &Diagnostic) -> usize {
    match diagnostic.severity {
        athanor_domain::Severity::Critical => 4,
        athanor_domain::Severity::High => 3,
        athanor_domain::Severity::Medium => 2,
        athanor_domain::Severity::Low => 1,
    }
}

fn serialized_relation_kind(kind: &RelationKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn serialized_entity_kind(kind: &EntityKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        DiagnosticKind, DiagnosticStatus, EntityId, EvidenceStatus, Ownership, RelationId,
        RelationStatus, Severity, SnapshotId, StableKey,
    };
    use serde_json::json;

    use super::*;

    fn entity(id: &str, key: &str, kind: EntityKind, path: &str, schema: Option<&str>) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(key.to_string()),
            kind,
            name: key.to_string(),
            title: None,
            source: Some(athanor_domain::SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(2),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: schema.map_or_else(|| json!({}), |schema| json!({"schema": schema})),
        }
    }

    fn relation(
        id: &str,
        kind: RelationKind,
        from: &Entity,
        to: &Entity,
        schema: Option<&str>,
    ) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Verified,
            confidence: 0.9,
            evidence: vec![Evidence {
                source_file: Some("src/link.rs".to_string()),
                line_start: Some(3),
                line_end: Some(3),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 0.9,
                status: EvidenceStatus::Verified,
            }],
            ownership: vec![Ownership {
                source_file: "src/link.rs".to_string(),
            }],
            snapshot: SnapshotId("snap-1".to_string()),
            payload: schema.map_or_else(|| json!({}), |schema| json!({"schema": schema})),
        }
    }

    fn fixture() -> (CanonicalSnapshot, Entity) {
        let endpoint = entity(
            "endpoint",
            "api://GET:/users",
            EntityKind::ApiEndpoint,
            "openapi.yaml",
            None,
        );
        let handler = entity(
            "handler",
            "rust://users/list",
            EntityKind::Function,
            "src/users.rs",
            None,
        );
        let test = entity(
            "test",
            "rust-test://users/list",
            EntityKind::TestCase,
            "tests/users.rs",
            None,
        );
        let platform = entity(
            "fba",
            "rustok-fba://users",
            EntityKind::Other("rustok_fba_module".to_string()),
            "contracts/users-fba-registry.json",
            Some("rustok.fba.entity.v1"),
        );
        let relations = vec![
            relation(
                "implemented",
                RelationKind::ImplementedBy,
                &endpoint,
                &handler,
                None,
            ),
            relation("tested", RelationKind::TestedBy, &handler, &test, None),
            relation(
                "platform",
                RelationKind::Other("rustok_fba_owns".to_string()),
                &platform,
                &handler,
                Some("rustok.fba.relation.v1"),
            ),
        ];
        (
            CanonicalSnapshot {
                snapshot: Some(SnapshotId("snap-1".to_string())),
                entities: vec![endpoint.clone(), handler, test, platform],
                facts: Vec::new(),
                relations,
                diagnostics: Vec::new(),
            },
            endpoint,
        )
    }

    #[test]
    fn builds_deterministic_relation_chains_and_test_coverage() {
        let (snapshot, endpoint) = fixture();
        let report = build_change_map(
            &snapshot,
            ChangeMapQuery {
                task: Some("change users API".to_string()),
                target: None,
                diff: false,
                changed_files: Vec::new(),
            },
            vec![Seed {
                id: endpoint.id,
                score: 1_000,
                reason: "task match".to_string(),
            }],
            ChangeMapLimits {
                max_entities: 10,
                max_files: 10,
                max_diagnostics: 10,
                max_depth: 3,
            },
        );

        assert_eq!(report.schema, "athanor.change_map.v1");
        let handler = report
            .items
            .iter()
            .find(|item| item.entity.id.0 == "handler")
            .unwrap();
        assert_eq!(handler.path[0].relation_id, "implemented");
        assert_eq!(handler.test_coverage.status, ChangeMapTestStatus::Linked);
        assert_eq!(handler.test_coverage.tests, ["rust-test://users/list"]);
        let platform = report
            .items
            .iter()
            .find(|item| item.entity.id.0 == "fba")
            .unwrap();
        assert!(
            platform
                .annotations
                .iter()
                .any(|item| item.source == "rustok")
        );
    }

    #[test]
    fn reports_limits_missing_tests_and_open_diagnostics() {
        let (mut snapshot, endpoint) = fixture();
        snapshot.diagnostics.push(Diagnostic {
            id: DiagnosticId("diag-1".to_string()),
            kind: DiagnosticKind::UncoveredSymbol,
            severity: Severity::High,
            status: DiagnosticStatus::Open,
            title: "Missing test".to_string(),
            message: "Add a test".to_string(),
            entities: vec![endpoint.id.clone()],
            evidence: entity_evidence(&endpoint),
            ownership: endpoint.ownership.clone(),
            snapshot: SnapshotId("snap-1".to_string()),
            suggested_fix: None,
            payload: json!({}),
        });
        let report = build_change_map(
            &snapshot,
            ChangeMapQuery {
                task: None,
                target: Some(endpoint.stable_key.0.clone()),
                diff: false,
                changed_files: Vec::new(),
            },
            vec![Seed {
                id: endpoint.id,
                score: 1_200,
                reason: "target".to_string(),
            }],
            ChangeMapLimits {
                max_entities: 1,
                max_files: 1,
                max_diagnostics: 1,
                max_depth: 3,
            },
        );

        assert!(report.omitted.entities > 0);
        assert_eq!(report.returned.diagnostics, 1);
        assert_eq!(
            report.items[0].test_coverage.status,
            ChangeMapTestStatus::NotLinked
        );
        assert!(
            report.items[0]
                .reasons
                .iter()
                .any(|reason| reason.contains("no linked test"))
        );
    }
}
