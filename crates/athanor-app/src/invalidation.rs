//! Conservative adapter invalidation planning for incremental indexing.

use std::collections::{BTreeSet, HashMap, VecDeque};

use athanor_core::{AffectedSubset, InvalidationPolicy, InvalidationScope};
use athanor_domain::{Entity, Fact, Relation};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterInvalidationDeclaration {
    pub adapter: String,
    pub policy: InvalidationPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedInvalidationScope {
    FileLocal,
    DependencyClosure,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvalidationPlan {
    pub scope: PlannedInvalidationScope,
    pub added_files: bool,
    pub removed_files: bool,
    pub global_adapters: Vec<String>,
}

pub fn plan_invalidation(
    declarations: impl IntoIterator<Item = AdapterInvalidationDeclaration>,
    added_files: bool,
    removed_files: bool,
) -> InvalidationPlan {
    let mut scope = PlannedInvalidationScope::FileLocal;
    let mut global_adapters = Vec::new();

    for declaration in declarations {
        let adapter_scope = scope_for(&declaration.policy, added_files, removed_files);
        match adapter_scope {
            InvalidationScope::AlwaysGlobal
            | InvalidationScope::GlobalOnAdd
            | InvalidationScope::GlobalOnRemove => {
                scope = PlannedInvalidationScope::Global;
                global_adapters.push(declaration.adapter);
            }
            InvalidationScope::DependencyClosure if scope != PlannedInvalidationScope::Global => {
                scope = PlannedInvalidationScope::DependencyClosure;
            }
            InvalidationScope::FileLocal => {}
            _ => {}
        }
    }

    global_adapters.sort();
    global_adapters.dedup();
    InvalidationPlan {
        scope,
        added_files,
        removed_files,
        global_adapters,
    }
}

/// Expands affected entities through the current canonical relation graph.
///
/// Linkers and checkers still receive the complete canonical context separately. This subset
/// identifies the graph-connected portion that must be reconsidered by adapters declaring
/// [`InvalidationScope::DependencyClosure`]. Relations are treated as undirected for invalidation:
/// a changed importer can affect its import target just as a changed target can affect importers.
pub fn dependency_closure(
    affected: &AffectedSubset,
    entities: &[Entity],
    facts: &[Fact],
    relations: &[Relation],
) -> AffectedSubset {
    let mut adjacent = HashMap::<String, Vec<String>>::new();
    for relation in relations {
        adjacent
            .entry(relation.from.0.clone())
            .or_default()
            .push(relation.to.0.clone());
        adjacent
            .entry(relation.to.0.clone())
            .or_default()
            .push(relation.from.0.clone());
    }

    let mut ids = affected
        .entities
        .iter()
        .map(|entity| entity.id.0.clone())
        .collect::<BTreeSet<_>>();
    let mut queue = ids.iter().cloned().collect::<VecDeque<_>>();
    while let Some(id) = queue.pop_front() {
        for adjacent_id in adjacent.get(&id).into_iter().flatten() {
            if ids.insert(adjacent_id.clone()) {
                queue.push_back(adjacent_id.clone());
            }
        }
    }

    AffectedSubset {
        entities: entities
            .iter()
            .filter(|entity| ids.contains(&entity.id.0))
            .cloned()
            .collect(),
        facts: facts
            .iter()
            .filter(|fact| {
                ids.contains(&fact.subject.0)
                    || fact
                        .object
                        .as_ref()
                        .is_some_and(|object| ids.contains(&object.0))
            })
            .cloned()
            .collect(),
        relations: relations
            .iter()
            .filter(|relation| ids.contains(&relation.from.0) && ids.contains(&relation.to.0))
            .cloned()
            .collect(),
    }
}

fn scope_for(
    policy: &InvalidationPolicy,
    added_files: bool,
    removed_files: bool,
) -> InvalidationScope {
    if added_files {
        return policy.on_add;
    }
    if removed_files {
        return policy.on_remove;
    }
    policy.on_change
}

#[cfg(test)]
mod tests {
    use super::{
        AdapterInvalidationDeclaration, PlannedInvalidationScope, dependency_closure,
        plan_invalidation,
    };
    use athanor_core::{AffectedSubset, InvalidationPolicy, InvalidationScope};
    use athanor_domain::{
        Entity, EntityId, EntityKind, Relation, RelationId, RelationKind, RelationStatus,
        SnapshotId, StableKey,
    };
    use serde_json::json;

    fn declaration(name: &str, policy: InvalidationPolicy) -> AdapterInvalidationDeclaration {
        AdapterInvalidationDeclaration {
            adapter: name.to_string(),
            policy,
        }
    }

    #[test]
    fn defaults_to_global_for_undeclared_adapter() {
        let plan = plan_invalidation(
            [declaration(
                "checker:unknown",
                InvalidationPolicy::ALWAYS_GLOBAL,
            )],
            false,
            false,
        );
        assert_eq!(plan.scope, PlannedInvalidationScope::Global);
        assert_eq!(plan.global_adapters, vec!["checker:unknown"]);
    }

    #[test]
    fn preserves_file_local_scope_when_all_adapters_are_local() {
        let plan = plan_invalidation(
            [declaration(
                "extractor:file",
                InvalidationPolicy::FILE_LOCAL,
            )],
            true,
            false,
        );
        assert_eq!(plan.scope, PlannedInvalidationScope::FileLocal);
    }

    #[test]
    fn selects_dependency_closure_without_global_adapter() {
        let policy = InvalidationPolicy {
            on_change: InvalidationScope::DependencyClosure,
            on_add: InvalidationScope::DependencyClosure,
            on_remove: InvalidationScope::DependencyClosure,
        };
        let plan = plan_invalidation([declaration("linker:imports", policy)], false, true);
        assert_eq!(plan.scope, PlannedInvalidationScope::DependencyClosure);
    }

    #[test]
    fn reports_global_adapter_for_add_specific_policy() {
        let policy = InvalidationPolicy {
            on_change: InvalidationScope::FileLocal,
            on_add: InvalidationScope::GlobalOnAdd,
            on_remove: InvalidationScope::FileLocal,
        };
        let plan = plan_invalidation([declaration("checker:duplicates", policy)], true, false);
        assert_eq!(plan.scope, PlannedInvalidationScope::Global);
        assert_eq!(plan.global_adapters, vec!["checker:duplicates"]);
    }

    #[test]
    fn dependency_closure_traverses_connected_relations_and_excludes_other_components() {
        let first = entity("first");
        let second = entity("second");
        let third = entity("third");
        let isolated = entity("isolated");
        let closure = dependency_closure(
            &AffectedSubset::from_extracted(vec![first.clone()], Vec::new()),
            &[
                first.clone(),
                second.clone(),
                third.clone(),
                isolated.clone(),
            ],
            &[],
            &[
                relation("first-second", &first, &second),
                relation("second-third", &second, &third),
            ],
        );

        assert_eq!(closure.entities.len(), 3);
        assert!(
            closure
                .entities
                .iter()
                .all(|entity| entity.id != isolated.id)
        );
        assert_eq!(closure.relations.len(), 2);
    }

    fn entity(id: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("test://{id}")),
            kind: EntityKind::File,
            name: id.to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn relation(id: &str, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind: RelationKind::Contains,
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
}
