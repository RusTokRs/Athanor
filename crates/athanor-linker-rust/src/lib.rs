#![allow(clippy::collapsible_if)]

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, InvalidationPolicy, InvalidationScope, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityId, EntityKind, Evidence, EvidenceStatus, Fact, Ownership, Relation, RelationId,
    RelationKind, RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::json;

/// Confidence of a trait-implementation bridge resolved by unique symbol name.
const TRAIT_IMPL_CONFIDENCE: f32 = 0.7;

#[derive(Debug, Clone, Default)]
pub struct RustLinker;

#[async_trait]
impl Linker for RustLinker {
    fn name(&self) -> &'static str {
        "rust-linker"
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

        // Keep a set of all qualified names/stable keys in the snapshot for path resolution
        let mut rust_entities = Vec::new();
        let mut rust_by_qualified_name = HashMap::new();
        let mut rust_by_id = HashMap::new();
        let mut qualified_names = HashSet::new();

        for entity in input.entities.iter() {
            if let Some(qn) = qualified_name(entity) {
                rust_entities.push(entity);
                rust_by_qualified_name.insert(qn, entity);
                rust_by_id.insert(&entity.id, entity);
                qualified_names.insert(qn);
            }
        }

        let mut relations = Vec::new();
        let mut relation_ids = HashSet::new();

        // 1. Containment and Imports Resolution
        for entity in &rust_entities {
            let qn = qualified_name(entity).unwrap();

            // Module Containment (Contains): parent module contains child
            if qn.contains("::") {
                let parts = qn.split("::").collect::<Vec<_>>();
                let parent_qn = parts[..parts.len() - 1].join("::");
                if let Some(parent) = rust_by_qualified_name
                    .get(parent_qn.as_str())
                    .filter(|parent| either_affected(entity, parent, &affected_ids))
                {
                    push_unique(
                        &mut relations,
                        &mut relation_ids,
                        relation(
                            &input.snapshot,
                            parent,
                            entity,
                            RelationKind::Contains,
                            self.name(),
                            "rust_module_containment",
                            &parent_qn,
                            1.0,
                            RelationStatus::Verified,
                        ),
                    );
                }
            }

            // Imports: resolve imports to target symbols
            if entity.kind != EntityKind::Module {
                continue;
            }
            if let Some(imports) = entity.payload.get("imports").and_then(|v| v.as_array()) {
                for import_val in imports {
                    if let Some(import_path) = import_val.as_str() {
                        if let Some(resolved) = resolve_path(qn, import_path, &[], &qualified_names)
                        {
                            if let Some(target) = rust_by_qualified_name
                                .get(resolved.as_str())
                                .filter(|target| either_affected(entity, target, &affected_ids))
                            {
                                push_unique(
                                    &mut relations,
                                    &mut relation_ids,
                                    relation(
                                        &input.snapshot,
                                        entity,
                                        target,
                                        RelationKind::Imports,
                                        self.name(),
                                        "rust_use_import",
                                        import_path,
                                        1.0,
                                        RelationStatus::Verified,
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }

        // 2. Call Graph (Calls) and Tested By (TestedBy)
        let mut calls_relations = Vec::new();

        for entity in &rust_entities {
            if entity.kind == EntityKind::Function || entity.kind == EntityKind::TestCase {
                let qn = qualified_name(entity).unwrap();
                // Find containing module imports to resolve local paths
                let parts = qn.split("::").collect::<Vec<_>>();
                let parent_module_qn = parts[..parts.len() - 1].join("::");
                let imports = rust_by_qualified_name
                    .get(parent_module_qn.as_str())
                    .and_then(|e| e.payload.get("imports").and_then(|v| v.as_array()))
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                if let Some(calls) = entity.payload.get("calls").and_then(|v| v.as_array()) {
                    for call_val in calls {
                        if let Some(call_path) = call_val.as_str() {
                            if let Some(resolved) = resolve_path(
                                &parent_module_qn,
                                call_path,
                                &imports,
                                &qualified_names,
                            ) {
                                if let Some(target) = rust_by_qualified_name
                                    .get(resolved.as_str())
                                    .filter(|target| either_affected(entity, target, &affected_ids))
                                {
                                    let rel = relation(
                                        &input.snapshot,
                                        entity,
                                        target,
                                        RelationKind::Calls,
                                        self.name(),
                                        "rust_static_call",
                                        call_path,
                                        0.8,
                                        RelationStatus::Inferred,
                                    );
                                    calls_relations.push(rel);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Emit tested_by relations based on TestCase -> Function calls
        for rel in &calls_relations {
            if rel.kind != RelationKind::Calls {
                continue;
            }
            let Some(caller) = rust_by_id
                .get(&rel.from)
                .filter(|entity| entity.kind == EntityKind::TestCase)
            else {
                continue;
            };
            let Some(callee) = rust_by_id
                .get(&rel.to)
                .filter(|entity| entity.kind == EntityKind::Function)
            else {
                continue;
            };
            if either_affected(caller, callee, &affected_ids) {
                push_unique(
                    &mut relations,
                    &mut relation_ids,
                    relation(
                        &input.snapshot,
                        callee,
                        caller,
                        RelationKind::TestedBy,
                        self.name(),
                        "test_case_calls_function",
                        &caller.name,
                        0.9,
                        RelationStatus::Inferred,
                    ),
                );
            }
        }

        // Trait implementations recorded as facts become `Implements` bridges from the
        // implementing type to the trait; both endpoints resolve by unique symbol name,
        // so ambiguous names stay unresolved rather than guessed.
        let mut types_by_name: HashMap<&str, Vec<&Entity>> = HashMap::new();
        let mut traits_by_name: HashMap<&str, Vec<&Entity>> = HashMap::new();
        for entity in rust_entities.iter().copied() {
            match entity
                .payload
                .get("symbol_kind")
                .and_then(|value| value.as_str())
            {
                Some(kind) if matches!(kind, "struct" | "enum" | "union" | "type_alias") => {
                    types_by_name
                        .entry(entity.name.as_str())
                        .or_default()
                        .push(entity);
                }
                Some("trait") => {
                    traits_by_name
                        .entry(entity.name.as_str())
                        .or_default()
                        .push(entity);
                }
                _ => {}
            }
        }

        for fact in input.facts.iter() {
            let impl_kind = fact
                .value
                .get("symbol_kind")
                .and_then(|value| value.as_str());
            if impl_kind != Some("trait_impl") {
                continue;
            }
            let Some(self_type) = fact.value.get("self_type").and_then(|value| value.as_str())
            else {
                continue;
            };
            let Some(trait_name) = fact
                .value
                .get("trait_name")
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            let Some(type_entity) = unique_by_name(&types_by_name, self_type) else {
                continue;
            };
            let Some(trait_entity) = unique_by_name(&traits_by_name, trait_name) else {
                continue;
            };
            if !either_affected(type_entity, trait_entity, &affected_ids) {
                continue;
            }
            push_unique(
                &mut relations,
                &mut relation_ids,
                impl_relation(
                    &input.snapshot,
                    type_entity,
                    trait_entity,
                    fact,
                    self.name(),
                ),
            );
        }

        relations.extend(calls_relations);
        Ok(relations)
    }
}

fn qualified_name(entity: &Entity) -> Option<&str> {
    entity
        .payload
        .get("qualified_name")
        .and_then(|v| v.as_str())
}

fn either_affected(left: &Entity, right: &Entity, affected: &HashSet<EntityId>) -> bool {
    affected.contains(&left.id) || affected.contains(&right.id)
}

fn qualify(parent: &str, name: &str) -> String {
    format!("{parent}::{name}")
}

fn resolve_path(
    current_module: &str,
    path: &str,
    imports: &[String],
    all_keys: &HashSet<&str>,
) -> Option<String> {
    // 1. Direct relative resolution in current module
    let local_resolved = qualify(current_module, path);
    if all_keys.contains(local_resolved.as_str()) {
        return Some(local_resolved);
    }

    // 2. Resolve via imports
    let first_segment = path.split("::").next().unwrap_or(path);
    for import in imports {
        let import_last_seg = import.split("::").last().unwrap_or(import);
        if import_last_seg == first_segment {
            let resolved = if path.contains("::") {
                let suffix = path.strip_prefix(first_segment).unwrap_or("");
                format!("{import}{suffix}")
            } else {
                import.clone()
            };
            if all_keys.contains(resolved.as_str()) {
                return Some(resolved);
            }
        } else if import_last_seg == "*" {
            if let Some(resolved) = import
                .strip_suffix("::*")
                .map(|base| qualify(base, path))
                .filter(|resolved| all_keys.contains(resolved.as_str()))
            {
                return Some(resolved);
            }
        }
    }

    // 3. Absolute / relative prefix path resolution (crate::, self::, super::)
    if path.starts_with("crate::") {
        let crate_prefix = current_module.split("::").next().unwrap_or("crate");
        let resolved = path.replace("crate", crate_prefix);
        if all_keys.contains(resolved.as_str()) {
            return Some(resolved);
        }
    } else if path.starts_with("self::") {
        if let Some(suffix) = path.strip_prefix("self::") {
            let resolved = qualify(current_module, suffix);
            if all_keys.contains(resolved.as_str()) {
                return Some(resolved);
            }
        }
    } else if path.starts_with("super::") {
        let mut parts = current_module.split("::").collect::<Vec<_>>();
        let mut path_part = path;
        while path_part.starts_with("super::") {
            parts.pop();
            path_part = &path_part[7..];
        }
        if !parts.is_empty() {
            let resolved = qualify(&parts.join("::"), path_part);
            if all_keys.contains(resolved.as_str()) {
                return Some(resolved);
            }
        }
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn relation(
    snapshot: &SnapshotId,
    from: &Entity,
    to: &Entity,
    kind: RelationKind,
    linker: &str,
    reason: &str,
    matched_value: &str,
    confidence: f32,
    status: RelationStatus,
) -> Relation {
    let id_material = format!(
        "rust_rel\0{:?}\0{}\0{}",
        kind, from.stable_key.0, to.stable_key.0
    );
    Relation {
        id: RelationId(format!(
            "rel_rust_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind,
        from: from.id.clone(),
        to: to.id.clone(),
        status,
        confidence,
        evidence: evidence_for_entities(from, to, linker, confidence, status),
        ownership: ownership_for_entities(from, to),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": from.stable_key.0,
            "to": to.stable_key.0,
            "reason": reason,
            "matched_value": matched_value,
        }),
    }
}

fn evidence_for_entities(
    left: &Entity,
    right: &Entity,
    linker: &str,
    confidence: f32,
    status: RelationStatus,
) -> Vec<Evidence> {
    let evidence_status = match status {
        RelationStatus::Verified => EvidenceStatus::Verified,
        _ => EvidenceStatus::Inferred,
    };
    [left, right]
        .into_iter()
        .filter_map(|entity| entity.source.as_ref())
        .map(|source| Evidence {
            source_file: Some(source.path.clone()),
            line_start: source.line_start,
            line_end: source.line_end,
            extractor: Some(linker.to_string()),
            commit_hash: None,
            confidence,
            status: evidence_status,
        })
        .collect()
}

fn ownership_for_entities(left: &Entity, right: &Entity) -> Vec<Ownership> {
    let mut ownership = left.ownership.clone();
    for owner in &right.ownership {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
}

fn unique_by_name<'a>(index: &HashMap<&str, Vec<&'a Entity>>, name: &str) -> Option<&'a Entity> {
    match index.get(name).map(Vec::as_slice) {
        Some([entity]) => Some(*entity),
        _ => None,
    }
}

fn impl_relation(
    snapshot: &SnapshotId,
    type_entity: &Entity,
    trait_entity: &Entity,
    fact: &Fact,
    linker: &str,
) -> Relation {
    let id_material = format!(
        "rust_rel_implements\0{}\0{}",
        type_entity.stable_key.0, trait_entity.stable_key.0
    );
    let trait_name = fact
        .value
        .get("trait_name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    Relation {
        id: RelationId(format!(
            "rel_rust_implements_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Implements,
        from: type_entity.id.clone(),
        to: trait_entity.id.clone(),
        status: RelationStatus::Inferred,
        confidence: TRAIT_IMPL_CONFIDENCE,
        evidence: impl_evidence(fact, type_entity, trait_entity, linker),
        ownership: ownership_for_entities(type_entity, trait_entity),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": type_entity.stable_key.0,
            "to": trait_entity.stable_key.0,
            "reason": "rust_trait_impl",
            "matched_value": trait_name.clone(),
            "self_type": type_entity.name.clone(),
            "trait_name": trait_name,
        }),
    }
}

/// The impl-block evidence recorded by the extractor plus one anchor per endpoint.
fn impl_evidence(
    fact: &Fact,
    type_entity: &Entity,
    trait_entity: &Entity,
    linker: &str,
) -> Vec<Evidence> {
    let mut evidence = fact.evidence.clone();
    for entity in [type_entity, trait_entity] {
        let Some(source) = entity.source.as_ref() else {
            continue;
        };
        if evidence
            .iter()
            .any(|existing| existing.source_file.as_deref() == Some(source.path.as_str()))
        {
            continue;
        }
        evidence.push(Evidence {
            source_file: Some(source.path.clone()),
            line_start: source.line_start,
            line_end: source.line_end,
            extractor: Some(linker.to_string()),
            commit_hash: None,
            confidence: TRAIT_IMPL_CONFIDENCE,
            status: EvidenceStatus::Inferred,
        });
    }
    evidence
}

fn push_unique(
    relations: &mut Vec<Relation>,
    relation_ids: &mut HashSet<RelationId>,
    relation: Relation,
) {
    if relation_ids.insert(relation.id.clone()) {
        relations.push(relation);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use athanor_core::AffectedSubset;
    use athanor_domain::{FactId, FactKind, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn links_rust_containment_imports_calls_and_testing() {
        let root = entity(
            "ent_mod_crate",
            "symbol://rust:crate",
            EntityKind::Module,
            "crate",
            json!({ "qualified_name": "crate", "imports": ["crate::auth::Session"] }),
        );
        let child = entity(
            "ent_mod_auth",
            "symbol://rust:crate::auth",
            EntityKind::Module,
            "auth",
            json!({ "qualified_name": "crate::auth", "imports": [] }),
        );
        let session = entity(
            "ent_struct_session",
            "symbol://rust:crate::auth::Session",
            EntityKind::Symbol,
            "Session",
            json!({ "qualified_name": "crate::auth::Session" }),
        );
        let function = entity(
            "ent_fn_login",
            "symbol://rust:crate::auth::login",
            EntityKind::Function,
            "login",
            json!({ "qualified_name": "crate::auth::login", "calls": ["Session::refresh"] }),
        );
        let method = entity(
            "ent_method_refresh",
            "symbol://rust:crate::auth::Session::refresh",
            EntityKind::Function,
            "refresh",
            json!({ "qualified_name": "crate::auth::Session::refresh" }),
        );
        let test_case = entity(
            "ent_test_login",
            "symbol://rust:crate::auth::test_login",
            EntityKind::TestCase,
            "test_login",
            json!({ "qualified_name": "crate::auth::test_login", "calls": ["login"] }),
        );

        let entities = vec![
            root.clone(),
            child.clone(),
            session.clone(),
            function.clone(),
            method.clone(),
            test_case.clone(),
        ];

        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone().into(),
                facts: Vec::new().into(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        // 1. Module containment
        assert!(
            relations
                .iter()
                .any(|r| r.kind == RelationKind::Contains && r.from == root.id && r.to == child.id)
        );
        assert!(
            relations.iter().any(|r| r.kind == RelationKind::Contains
                && r.from == child.id
                && r.to == session.id)
        );

        // 2. Imports
        assert!(
            relations.iter().any(|r| r.kind == RelationKind::Imports
                && r.from == root.id
                && r.to == session.id)
        );

        // 3. Calls
        assert!(
            relations.iter().any(|r| r.kind == RelationKind::Calls
                && r.from == function.id
                && r.to == method.id)
        );
        assert!(relations.iter().any(|r| r.kind == RelationKind::Calls
            && r.from == test_case.id
            && r.to == function.id));

        // 4. Tested by
        assert!(relations.iter().any(|r| r.kind == RelationKind::TestedBy
            && r.from == function.id
            && r.to == test_case.id));
    }

    #[tokio::test]
    async fn links_trait_impl_facts_to_implements_bridges() {
        let trait_entity = entity(
            "ent_trait_login",
            "symbol://rust:crate::auth::Login",
            EntityKind::Symbol,
            "Login",
            json!({ "symbol_kind": "trait", "qualified_name": "crate::auth::Login" }),
        );
        let type_entity = entity(
            "ent_struct_session",
            "symbol://rust:crate::auth::Session",
            EntityKind::Symbol,
            "Session",
            json!({ "symbol_kind": "struct", "qualified_name": "crate::auth::Session" }),
        );
        // Two structs share one name, so the ambiguous name must stay unresolved.
        let ambiguous = entity(
            "ent_struct_session_other",
            "symbol://rust:crate::other::Session",
            EntityKind::Symbol,
            "Session",
            json!({ "symbol_kind": "struct", "qualified_name": "crate::other::Session" }),
        );
        let fact = Fact {
            id: FactId("fact_trait_impl_test".to_string()),
            kind: FactKind::SymbolDefined,
            subject: type_entity.id.clone(),
            object: None,
            value: json!({
                "symbol_kind": "trait_impl",
                "self_type": "Session",
                "trait_name": "Login",
                "trait_path": "crate::auth::Login",
            }),
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            extractor: "rust".to_string(),
            confidence: 1.0,
        };
        let entities = vec![trait_entity.clone(), type_entity.clone()];
        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(vec![fact.clone()]),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Implements
                && relation.from == type_entity.id
                && relation.to == trait_entity.id
                && relation.status == RelationStatus::Inferred
                && relation.payload["reason"] == "rust_trait_impl"
                && relation.payload["trait_name"] == "Login"
        }));

        let ambiguous_entities = vec![trait_entity.clone(), type_entity.clone(), ambiguous.clone()];
        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(ambiguous_entities.clone()),
                facts: Arc::new(vec![fact]),
                affected: AffectedSubset::from_extracted(ambiguous_entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Implements)
        );
    }

    fn entity(
        id: &str,
        stable_key: &str,
        kind: EntityKind,
        name: &str,
        payload: serde_json::Value,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "src/lib.rs".to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("rust".to_string())),
            aliases: Vec::new(),
            ownership: athanor_extractor_basic::ownership_for_file("src/lib.rs"),
            payload,
        }
    }
}
