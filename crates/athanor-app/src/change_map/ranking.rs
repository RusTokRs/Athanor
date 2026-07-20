use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use athanor_domain::{
    Diagnostic, DiagnosticStatus, Entity, EntityId, EntityKind, Relation, RelationKind,
};

use crate::json_contract::CHANGE_MAP_SCHEMA_V1;

use super::evidence::{annotations_from_payload, dedupe_evidence, entity_evidence, entity_files};
use super::model::{
    Candidate, ChangeMapCompleteness, ChangeMapCounts, ChangeMapEndpoint, ChangeMapFile,
    ChangeMapItem, ChangeMapLimits, ChangeMapPathStep, ChangeMapQuery, ChangeMapReport,
    ChangeMapTestCoverage, ChangeMapTestStatus, PathLink, Seed,
};

const DEFAULT_MAX_CANDIDATES: usize = 5_000;

pub(super) fn build_change_map(
    snapshot: &athanor_core::CanonicalSnapshot,
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
                (relation.to.clone(), crate::impact::FlowDirection::Forward)
            } else {
                (
                    relation.from.clone(),
                    crate::impact::FlowDirection::Backward,
                )
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
        schema: CHANGE_MAP_SCHEMA_V1.to_string(),
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

fn diagnostics_by_entity(
    snapshot: &athanor_core::CanonicalSnapshot,
) -> HashMap<EntityId, Vec<&Diagnostic>> {
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
    snapshot: &athanor_core::CanonicalSnapshot,
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
