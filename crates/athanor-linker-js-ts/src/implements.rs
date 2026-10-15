//! Class and interface heritage bridges for JavaScript and TypeScript.
//!
//! The JS/TS extractor records class `extends`/`implements` and interface `extends` clauses as
//! bare type names. This linker resolves those names to canonical symbols and preserves the
//! distinction between `Extends` and `Implements` in the canonical relation vocabulary.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, InvalidationPolicy, InvalidationScope, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityId, Evidence, EvidenceStatus, Relation, RelationId, RelationKind, RelationStatus,
    SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

use crate::combined_ownership;
use crate::paths::normalize_path;

/// Bounded number of heritage bridges emitted for one snapshot.
const MAX_HERITAGE_RELATIONS: usize = 10_000;
/// Confidence of a heritage bridge resolved by class or interface name.
const HERITAGE_CONFIDENCE: f32 = 0.7;

#[derive(Debug, Default, Clone, Copy)]
pub struct JsTsImplementsLinker;

#[async_trait]
impl Linker for JsTsImplementsLinker {
    fn name(&self) -> &str {
        "js-ts-implements"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy {
            on_change: InvalidationScope::DependencyClosure,
            on_add: InvalidationScope::DependencyClosure,
            on_remove: InvalidationScope::DependencyClosure,
        }
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let affected_ids = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<_>>();

        let mut classes_by_name: HashMap<&str, Vec<&Entity>> = HashMap::new();
        let mut interfaces_by_name: HashMap<&str, Vec<&Entity>> = HashMap::new();
        for entity in input.entities.iter() {
            match js_ts_symbol_kind(entity) {
                Some("class") => classes_by_name
                    .entry(entity.name.as_str())
                    .or_default()
                    .push(entity),
                Some("interface") => interfaces_by_name
                    .entry(entity.name.as_str())
                    .or_default()
                    .push(entity),
                _ => {}
            }
        }

        let mut relations = Vec::new();
        for entity in input.entities.iter() {
            if relations.len() >= MAX_HERITAGE_RELATIONS {
                break;
            }
            let Some(source_kind) = js_ts_symbol_kind(entity) else {
                continue;
            };
            if !matches!(source_kind, "class" | "interface") {
                continue;
            }
            let owner_path = entity_path(entity);
            let mut declared = payload_names(&entity.payload, "extends")
                .into_iter()
                .map(|name| ("extends", name))
                .collect::<Vec<_>>();
            if source_kind == "class" {
                declared.extend(
                    payload_names(&entity.payload, "implements")
                        .into_iter()
                        .map(|name| ("implements", name)),
                );
            }

            for (bridge, name) in declared {
                if relations.len() >= MAX_HERITAGE_RELATIONS {
                    break;
                }
                let (index, relation_kind) = match (source_kind, bridge) {
                    ("class", "extends") => (&classes_by_name, RelationKind::Extends),
                    ("interface", "extends") => (&interfaces_by_name, RelationKind::Extends),
                    ("class", "implements") => (&interfaces_by_name, RelationKind::Implements),
                    _ => continue,
                };
                let Some(target) = resolve(index, &name, owner_path.as_deref(), &entity.id) else {
                    continue;
                };
                if !affected_ids.contains(&entity.id) && !affected_ids.contains(&target.id) {
                    continue;
                }
                relations.push(heritage_relation(
                    &input.snapshot,
                    entity,
                    target,
                    bridge,
                    relation_kind,
                    &name,
                ));
            }
        }

        relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        relations.dedup_by(|left, right| left.id == right.id);
        Ok(relations)
    }
}

fn js_ts_symbol_kind(entity: &Entity) -> Option<&str> {
    if !entity.stable_key.0.starts_with("symbol://js-ts:") {
        return None;
    }
    entity
        .payload
        .get("symbol_kind")
        .and_then(|value| value.as_str())
}

fn entity_path(entity: &Entity) -> Option<String> {
    entity
        .source
        .as_ref()
        .map(|source| normalize_path(&source.path))
}

fn payload_names(payload: &serde_json::Value, field: &str) -> Vec<String> {
    match payload.get(field) {
        Some(value) => value
            .as_str()
            .map(|name| vec![name.to_string()])
            .or_else(|| {
                value.as_array().map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
            })
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Resolve a heritage name to exactly one symbol.
///
/// A candidate declared in the same module wins; otherwise the name must be unique across the
/// snapshot. Ambiguous names stay unresolved instead of being guessed, matching the Rust linker's
/// trait-implementation bridges, and a class never resolves to itself.
fn resolve<'a>(
    index: &HashMap<&str, Vec<&'a Entity>>,
    name: &str,
    owner_path: Option<&str>,
    owner_id: &EntityId,
) -> Option<&'a Entity> {
    let candidates = index
        .get(name)?
        .iter()
        .copied()
        .filter(|candidate| candidate.id != *owner_id)
        .collect::<Vec<_>>();
    if let Some(path) = owner_path {
        let same_module = candidates
            .iter()
            .copied()
            .filter(|candidate| entity_path(candidate).as_deref() == Some(path))
            .collect::<Vec<_>>();
        match same_module.as_slice() {
            [only] => return Some(*only),
            [] => {}
            _ => return None,
        }
    }
    match candidates.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

fn heritage_relation(
    snapshot: &SnapshotId,
    source: &Entity,
    target: &Entity,
    bridge: &str,
    relation_kind: RelationKind,
    matched_value: &str,
) -> Relation {
    let relation_family = if relation_kind == RelationKind::Extends {
        "extends"
    } else {
        "implements"
    };
    let id_material = format!(
        "js_ts_{relation_family}\0{}\0{}\0{}",
        source.stable_key.0, target.stable_key.0, bridge
    );
    let relation_id = format!(
        "rel_js_ts_{relation_family}_{:016x}",
        stable_hash(id_material.as_bytes())
    );
    let reason = match js_ts_symbol_kind(source) {
        Some("interface") => "js_ts_interface_heritage",
        _ => "js_ts_class_heritage",
    };
    Relation {
        id: RelationId(relation_id),
        kind: relation_kind,
        from: source.id.clone(),
        to: target.id.clone(),
        status: RelationStatus::Inferred,
        confidence: HERITAGE_CONFIDENCE,
        evidence: heritage_evidence(source, target),
        ownership: combined_ownership(source, target),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": source.stable_key.0,
            "to": target.stable_key.0,
            "reason": reason,
            "bridge": bridge,
            "matched_value": matched_value,
        }),
    }
}

fn heritage_evidence(class_entity: &Entity, target: &Entity) -> Vec<Evidence> {
    let mut evidence = Vec::new();
    for entity in [class_entity, target] {
        let Some(location) = entity.source.as_ref() else {
            continue;
        };
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: location.line_start,
            line_end: location.line_end,
            extractor: Some("js-ts-implements".to_string()),
            commit_hash: None,
            confidence: HERITAGE_CONFIDENCE,
            status: EvidenceStatus::Inferred,
        });
    }
    evidence
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use athanor_core::AffectedSubset;
    use athanor_domain::{EntityKind, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn links_extends_and_implements_heritage() {
        let base = class_entity("ent_base", "src/base.ts", "Base", None, &[]);
        let store = interface_entity("ent_store", "src/store.ts", "Store");
        let service = class_entity(
            "ent_service",
            "src/service.ts",
            "Service",
            Some("Base"),
            &["Store"],
        );
        let entities = vec![base.clone(), store.clone(), service.clone()];

        let relations = JsTsImplementsLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 2);
        assert!(relations.iter().all(|relation| {
            relation.from == service.id
                && relation.status == RelationStatus::Inferred
                && relation.payload["reason"] == "js_ts_class_heritage"
                && !relation.evidence.is_empty()
        }));
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Extends
                && relation.to == base.id
                && relation.payload["bridge"] == "extends"
        }));
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Implements
                && relation.to == store.id
                && relation.payload["bridge"] == "implements"
                && relation.payload["matched_value"] == "Store"
        }));
    }

    #[tokio::test]
    async fn links_interface_extends_as_extends() {
        let base = interface_entity("ent_base_interface", "src/types.ts", "BaseInterface");
        let child = interface_entity_extending(
            "ent_child_interface",
            "src/types.ts",
            "ChildInterface",
            &["BaseInterface"],
        );
        let entities = vec![base.clone(), child.clone()];

        let relations = JsTsImplementsLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].kind, RelationKind::Extends);
        assert_eq!(relations[0].from, child.id);
        assert_eq!(relations[0].to, base.id);
        assert_eq!(relations[0].payload["reason"], "js_ts_interface_heritage");
    }

    #[tokio::test]
    async fn prefers_the_same_module_target_over_a_remote_one() {
        let local_store = interface_entity("ent_store_local", "src/service.ts", "Store");
        let remote_store = interface_entity("ent_store_remote", "src/other/store.ts", "Store");
        let service = class_entity("ent_service", "src/service.ts", "Service", None, &["Store"]);
        let entities = vec![local_store.clone(), remote_store, service.clone()];

        let relations = JsTsImplementsLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].to, local_store.id);
    }

    #[tokio::test]
    async fn leaves_ambiguous_heritage_names_unresolved() {
        let first = interface_entity("ent_store_a", "src/a.ts", "Store");
        let second = interface_entity("ent_store_b", "src/b.ts", "Store");
        let service = class_entity(
            "ent_service",
            "src/service.ts",
            "Service",
            Some("Base"),
            &["Store"],
        );
        let entities = vec![first, second, service];

        let relations = JsTsImplementsLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(relations.is_empty());
    }

    #[tokio::test]
    async fn leaves_ambiguous_class_and_interface_extends_unresolved() {
        let first_class = class_entity("ent_base_class_a", "src/a.ts", "Base", None, &[]);
        let second_class = class_entity("ent_base_class_b", "src/b.ts", "Base", None, &[]);
        let derived = class_entity(
            "ent_derived_class",
            "src/child.ts",
            "Derived",
            Some("Base"),
            &[],
        );
        let first_interface = interface_entity("ent_parent_interface_a", "src/a.ts", "Parent");
        let second_interface = interface_entity("ent_parent_interface_b", "src/b.ts", "Parent");
        let child_interface = interface_entity_extending(
            "ent_child_interface_ambiguous",
            "src/child.ts",
            "Child",
            &["Parent"],
        );
        let entities = vec![
            first_class,
            second_class,
            derived,
            first_interface,
            second_interface,
            child_interface,
        ];

        let relations = JsTsImplementsLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(relations.is_empty());
    }

    fn class_entity(
        id: &str,
        path: &str,
        name: &str,
        extends: Option<&str>,
        implements: &[&str],
    ) -> Entity {
        let mut payload = json!({
            "symbol_kind": "class",
            "qualified_name": name,
            "exported": true,
            "async": false,
            "source_node": "class_declaration",
            "calls": [],
        });
        payload["extends"] = extends.map_or_else(|| json!([]), |name| json!([name]));
        payload["implements"] = json!(implements);
        js_ts_entity(id, path, name, EntityKind::Class, payload)
    }

    fn interface_entity(id: &str, path: &str, name: &str) -> Entity {
        interface_entity_extending(id, path, name, &[])
    }

    fn interface_entity_extending(id: &str, path: &str, name: &str, extends: &[&str]) -> Entity {
        js_ts_entity(
            id,
            path,
            name,
            EntityKind::Symbol,
            json!({
                "symbol_kind": "interface",
                "qualified_name": name,
                "exported": true,
                "async": false,
                "source_node": "interface_declaration",
                "extends": extends,
                "implements": [],
                "calls": [],
            }),
        )
    }

    fn js_ts_entity(
        id: &str,
        path: &str,
        name: &str,
        kind: EntityKind,
        payload: serde_json::Value,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("symbol://js-ts:{path}#{name}")),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(3),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file(path),
            payload,
        }
    }
}
