//! Conservative adapter invalidation planning for incremental indexing.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use athanor_core::{AffectedSubset, CanonicalSnapshot, InvalidationPolicy, InvalidationScope};
use athanor_domain::{Entity, EntityId, Fact, Relation};
use serde::Serialize;

use crate::hash::stable_hash;
use crate::pipeline_ownership::entity_owned_by_any_path;

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

/// Names the declarations a changed file still states identically.
///
/// A file is re-extracted as a whole, but the declarations inside it usually did
/// not move: a fingerprint of everything a linker or checker can read — identity,
/// source span, aliases, ownership, and payload — says which extracted objects are
/// still the ones the previous snapshot already resolved. Only the rest need to be
/// reconsidered, so a file that gained a blank line no longer invalidates every
/// declaration in it.
///
/// A declaration is reused only when the previous snapshot states it from one of
/// the affected paths: an object that was never extracted from a changed file has
/// no previous state to compare against.
pub(crate) fn unchanged_declarations(
    previous: Option<&CanonicalSnapshot>,
    affected_paths: &BTreeSet<String>,
    extracted: &[Entity],
) -> HashSet<EntityId> {
    let Some(previous) = previous else {
        return HashSet::new();
    };
    let mut previous_fingerprints = BTreeMap::<&str, u64>::new();
    for entity in &previous.entities {
        if !entity_owned_by_any_path(entity, affected_paths) {
            continue;
        }
        previous_fingerprints.insert(
            entity.stable_key.0.as_str(),
            declaration_fingerprint(entity),
        );
    }
    if previous_fingerprints.is_empty() {
        return HashSet::new();
    }
    extracted
        .iter()
        .filter(|entity| {
            previous_fingerprints.get(entity.stable_key.0.as_str())
                == Some(&declaration_fingerprint(entity))
        })
        .map(|entity| entity.id.clone())
        .collect()
}

/// Hashes everything a linker or checker can read from one declaration.
///
/// `id` is left out because it is derived from the stable key, and `snapshot` is
/// left out because it names the run rather than the object.
fn declaration_fingerprint(entity: &Entity) -> u64 {
    let material = serde_json::to_vec(&(
        &entity.stable_key,
        &entity.kind,
        &entity.name,
        &entity.title,
        &entity.source,
        &entity.language,
        &entity.aliases,
        &entity.ownership,
        &entity.payload,
    ));
    match material {
        Ok(bytes) => stable_hash(&bytes),
        Err(_) => stable_hash(entity.stable_key.0.as_bytes()),
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
    use std::collections::{BTreeSet, HashSet};

    use athanor_core::{AffectedSubset, CanonicalSnapshot, InvalidationPolicy, InvalidationScope};

    use super::unchanged_declarations;
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

    fn symbol(
        id: &str,
        key: &str,
        path: &str,
        line: Option<u32>,
        payload: serde_json::Value,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(key.to_string()),
            kind: EntityKind::Function,
            name: id.to_string(),
            title: None,
            source: Some(athanor_domain::SourceLocation {
                path: path.to_string(),
                line_start: line,
                line_end: line,
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![athanor_domain::Ownership {
                source_file: path.to_string(),
            }],
            payload,
        }
    }

    fn snapshot_with(entities: Vec<Entity>) -> CanonicalSnapshot {
        CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_previous".to_string())),
            entities,
            relations: Vec::new(),
            ..CanonicalSnapshot::default()
        }
    }

    #[test]
    fn reuses_a_declaration_a_changed_file_states_identically() {
        let previous = symbol(
            "ent_same",
            "symbol://rust:crate::same",
            "src/lib.rs",
            Some(4),
            json!({ "qualified_name": "crate::same" }),
        );
        let extracted = previous.clone();
        let reused = unchanged_declarations(
            Some(&snapshot_with(vec![previous])),
            &BTreeSet::from(["src/lib.rs".to_string()]),
            std::slice::from_ref(&extracted),
        );
        assert_eq!(reused, HashSet::from([extracted.id.clone()]));
    }

    #[test]
    fn reconsiders_a_declaration_whose_payload_moved() {
        let previous = symbol(
            "ent_moved",
            "symbol://rust:crate::moved",
            "src/lib.rs",
            Some(4),
            json!({ "qualified_name": "crate::moved" }),
        );
        let mut extracted = previous.clone();
        extracted.payload = json!({ "qualified_name": "crate::moved", "calls": [] });
        let reused = unchanged_declarations(
            Some(&snapshot_with(vec![previous])),
            &BTreeSet::from(["src/lib.rs".to_string()]),
            &[extracted],
        );
        assert!(reused.is_empty());
    }

    #[test]
    fn reconsiders_a_declaration_that_moved_to_another_line() {
        let previous = symbol(
            "ent_shifted",
            "symbol://rust:crate::shifted",
            "src/lib.rs",
            Some(4),
            json!({}),
        );
        let mut extracted = previous.clone();
        extracted.source = Some(athanor_domain::SourceLocation {
            path: "src/lib.rs".to_string(),
            line_start: Some(9),
            line_end: Some(9),
        });
        let reused = unchanged_declarations(
            Some(&snapshot_with(vec![previous])),
            &BTreeSet::from(["src/lib.rs".to_string()]),
            &[extracted],
        );
        assert!(reused.is_empty());
    }

    #[test]
    fn an_absent_or_untouched_previous_state_reuses_nothing() {
        let extracted = symbol(
            "ent_new",
            "symbol://rust:crate::new",
            "src/lib.rs",
            Some(4),
            json!({}),
        );
        assert!(
            unchanged_declarations(
                None,
                &BTreeSet::from(["src/lib.rs".to_string()]),
                std::slice::from_ref(&extracted)
            )
            .is_empty()
        );
        // A declaration the previous snapshot stated from another file has no
        // changed-file state to compare against.
        let previous = symbol(
            "ent_new",
            "symbol://rust:crate::new",
            "src/other.rs",
            Some(4),
            json!({}),
        );
        assert!(
            unchanged_declarations(
                Some(&snapshot_with(vec![previous])),
                &BTreeSet::from(["src/lib.rs".to_string()]),
                &[extracted],
            )
            .is_empty()
        );
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
