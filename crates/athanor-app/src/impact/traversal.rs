use std::collections::{HashMap, HashSet, VecDeque};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityId, RelationKind};

use crate::json_contract::IMPACT_ANALYSIS_SCHEMA_V1;

use super::model::{
    FlowDirection, ImpactAnalysis, ImpactPathEndpoint, ImpactPathStep, ImpactedEntity, RelationFlow,
};

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
                    if let Some(next_entity) = snapshot.entities.iter().find(|entity| entity.id == next_id)
                    {
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
                    if let Some(next_entity) = snapshot.entities.iter().find(|entity| entity.id == next_id)
                    {
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

    impacted_entities.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| {
                format!("{:?}", left.entity.kind).cmp(&format!("{:?}", right.entity.kind))
            })
            .then_with(|| left.entity.stable_key.0.cmp(&right.entity.stable_key.0))
    });

    let all_affected_ids = starting_entities
        .iter()
        .map(|entity| &entity.id)
        .chain(impacted_entities.iter().map(|impact| &impact.entity.id))
        .collect::<HashSet<&EntityId>>();

    let mut impacted_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .entities
                .iter()
                .any(|id| all_affected_ids.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    impacted_diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    let mut impacted_files_set = HashSet::new();
    for entity in starting_entities
        .iter()
        .chain(impacted_entities.iter().map(|impact| &impact.entity))
    {
        for ownership in &entity.ownership {
            impacted_files_set.insert(ownership.source_file.clone());
        }
        if let Some(source) = &entity.source {
            impacted_files_set.insert(source.path.clone());
        }
    }
    let mut impacted_files = impacted_files_set.into_iter().collect::<Vec<_>>();
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
        | RelationKind::Contains => (false, true),
        RelationKind::TestedBy | RelationKind::CoveredByTest | RelationKind::Defines => {
            (true, false)
        }
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
