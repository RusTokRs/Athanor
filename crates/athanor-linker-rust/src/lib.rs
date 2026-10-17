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
/// Confidence of a trait supertrait bridge resolved by canonical path/name.
const TRAIT_SUPERTYPE_CONFIDENCE: f32 = 0.7;
/// Confidence of a statically named call resolved by canonical path or import.
const STATIC_CALL_CONFIDENCE: f32 = 0.8;
/// Confidence of a method call on `self`, whose type is the enclosing impl.
const SELF_METHOD_CONFIDENCE: f32 = 0.8;
/// Confidence of a method call on a receiver whose concrete type is declared.
const TYPED_RECEIVER_CONFIDENCE: f32 = 0.8;
/// Confidence of a method call dispatched through a `dyn`/`impl` trait: the
/// target is the trait's declaration, not the code that runs.
const DYN_DISPATCH_CONFIDENCE: f32 = 0.7;
/// Confidence of one implementation a `dyn`/`impl` call can dispatch into: the
/// trait proves the interface and the impl proves the body exists, not that this
/// call reaches it.
const DYN_CANDIDATE_CONFIDENCE: f32 = 0.5;
/// The most implementations one dynamic call site records as candidates.
const MAX_DYN_CANDIDATES: usize = 8;

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

        // A receiver's declared type is a name; only a unique type of that name
        // in the snapshot can own the method a call dispatches to.
        let type_index = type_qualified_names(&rust_entities);

        // Trait implementations recorded as facts become `Implements` bridges from the
        // implementing type to the trait; both endpoints resolve by unique symbol name,
        // so ambiguous names stay unresolved rather than guessed.
        let mut types_by_name: HashMap<&str, Vec<&Entity>> = HashMap::new();
        let mut traits_by_name: HashMap<&str, Vec<&Entity>> = HashMap::new();
        let mut trait_qualified_names = HashSet::new();
        let mut trait_entities = Vec::new();
        for entity in rust_entities.iter().copied() {
            match entity
                .payload
                .get("symbol_kind")
                .and_then(|value| value.as_str())
            {
                Some("struct" | "enum" | "union" | "type_alias") => {
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
                    if let Some(qn) = qualified_name(entity) {
                        trait_qualified_names.insert(qn);
                    }
                    trait_entities.push(entity);
                }
                _ => {}
            }
        }

        // The types each trait is implemented for, proven by the same facts that
        // produce the `Implements` bridges. A dynamic call names the trait; the
        // implementations are what the trait says can run.
        let implementors_by_trait = trait_implementors(
            input.facts.iter(),
            &types_by_name,
            &traits_by_name,
        );

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
                        let Some(call) = call_val.as_object() else {
                            continue;
                        };
                        let callee = call.get("callee").and_then(|v| v.as_str()).unwrap_or("");
                        let name = call.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let kind = call.get("kind").and_then(|v| v.as_str()).unwrap_or("call");
                        let resolved = if kind == "method" {
                            call.get("receiver")
                                .and_then(|v| v.as_str())
                                .and_then(|receiver| {
                                    resolve_typed_method(
                                        entity,
                                        receiver,
                                        name,
                                        &type_index,
                                        &rust_by_qualified_name,
                                    )
                                })
                        } else {
                            resolve_path(&parent_module_qn, callee, &imports, &qualified_names)
                                .and_then(|resolved| {
                                    let target = rust_by_qualified_name.get(resolved.as_str())?;
                                    Some((*target, "rust_static_call", STATIC_CALL_CONFIDENCE))
                                })
                        };
                        if let Some((target, reason, confidence)) = resolved {
                            if target.id != entity.id
                                && either_affected(entity, target, &affected_ids)
                            {
                                calls_relations.push(relation(
                                    &input.snapshot,
                                    entity,
                                    target,
                                    RelationKind::Calls,
                                    self.name(),
                                    reason,
                                    callee,
                                    confidence,
                                    RelationStatus::Inferred,
                                ));
                                if reason == "rust_dyn_dispatch_method" {
                                    dyn_candidates(
                                        &input.snapshot,
                                        entity,
                                        target,
                                        name,
                                        &implementors_by_trait,
                                        &rust_by_qualified_name,
                                        &affected_ids,
                                        self.name(),
                                        &mut relations,
                                        &mut relation_ids,
                                    );
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

        // Rust supertraits are inheritance edges from the child trait to each resolved parent.
        // Resolve only against trait declarations, so same-named structs never become false bases.
        for trait_entity in trait_entities {
            let Some(trait_qn) = qualified_name(trait_entity) else {
                continue;
            };
            let Some((current_module, _)) = trait_qn.rsplit_once("::") else {
                continue;
            };
            let imports = trait_entity
                .payload
                .get("imports")
                .and_then(|value| value.as_array())
                .or_else(|| {
                    rust_by_qualified_name
                        .get(current_module)
                        .and_then(|module| {
                            module
                                .payload
                                .get("imports")
                                .and_then(|value| value.as_array())
                        })
                })
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let Some(supertraits) = trait_entity
                .payload
                .get("supertraits")
                .and_then(|value| value.as_array())
            else {
                continue;
            };
            for supertrait in supertraits.iter().filter_map(|value| value.as_str()) {
                let Some(resolved) = resolve_supertrait_path(
                    current_module,
                    supertrait,
                    &imports,
                    &qualified_names,
                    &trait_qualified_names,
                ) else {
                    continue;
                };
                let Some(supertrait_entity) =
                    rust_by_qualified_name.get(resolved.as_str()).copied()
                else {
                    continue;
                };
                if trait_entity.id == supertrait_entity.id
                    || !either_affected(trait_entity, supertrait_entity, &affected_ids)
                {
                    continue;
                }
                push_unique(
                    &mut relations,
                    &mut relation_ids,
                    relation(
                        &input.snapshot,
                        trait_entity,
                        supertrait_entity,
                        RelationKind::Extends,
                        self.name(),
                        "rust_trait_supertrait",
                        supertrait,
                        TRAIT_SUPERTYPE_CONFIDENCE,
                        RelationStatus::Inferred,
                    ),
                );
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

/// Maps a type name to the unique qualified name that declares it.
///
/// Only a struct, enum, union, type alias, or trait can own a method, and a name
/// declared twice names neither, so a receiver of that type stays unresolved
/// instead of dispatching to whichever declaration the map kept.
fn type_qualified_names<'a>(entities: &[&'a Entity]) -> HashMap<String, Option<&'a str>> {
    let mut owners: HashMap<String, Option<&'a str>> = HashMap::new();
    for entity in entities {
        let kind = entity
            .payload
            .get("symbol_kind")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if !matches!(kind, "struct" | "enum" | "union" | "type_alias" | "trait") {
            continue;
        }
        let Some(qualified) = qualified_name(entity) else {
            continue;
        };
        let prior = owners.get(&entity.name).copied();
        match prior {
            None => {
                owners.insert(entity.name.clone(), Some(qualified));
            }
            Some(Some(existing)) => {
                if existing != qualified {
                    owners.insert(entity.name.clone(), None);
                }
            }
            Some(None) => {}
        }
    }
    owners
}

/// The type one receiver binding declares.
#[derive(Clone)]
struct ReceiverType {
    type_name: String,
    kind: &'static str,
}

/// Reads the receiver types one function states.
///
/// A name bound to two types is ambiguous and names nothing, so it is recorded
/// as unresolved instead of letting one declaration win by accident.
fn receiver_types(entity: &Entity) -> HashMap<String, Option<ReceiverType>> {
    let mut receivers: HashMap<String, Option<ReceiverType>> = HashMap::new();
    let Some(bindings) = entity
        .payload
        .get("typed_bindings")
        .and_then(|v| v.as_array())
    else {
        return receivers;
    };
    for binding in bindings {
        let (Some(name), Some(type_name)) = (
            binding.get("name").and_then(|v| v.as_str()),
            binding.get("type").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        let kind = match binding.get("kind").and_then(|v| v.as_str()) {
            Some("trait_object") => "trait_object",
            Some("impl_trait") => "impl_trait",
            _ => "annotation",
        };
        let recorded = ReceiverType {
            type_name: type_name.to_string(),
            kind,
        };
        let prior = receivers.get(name).cloned();
        match prior {
            None => {
                receivers.insert(name.to_string(), Some(recorded));
            }
            Some(Some(existing)) => {
                if existing.type_name != type_name {
                    receivers.insert(name.to_string(), None);
                }
            }
            Some(None) => {}
        }
    }
    receivers
}

/// Resolves a method call whose receiver the source typed.
///
/// `self` dispatches inside its own impl, and any other receiver dispatches to
/// the method of its declared type. A `dyn`/`impl` trait receiver reaches the
/// trait's declaration, which is what dynamic dispatch calls, while a receiver
/// the function does not type stays unresolved rather than matching by name.
fn resolve_typed_method<'a>(
    caller: &Entity,
    receiver: &str,
    name: &str,
    type_index: &HashMap<String, Option<&'a str>>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
) -> Option<(&'a Entity, &'static str, f32)> {
    let (qualified, reason, confidence) = if receiver == "self" {
        let owner = qualified_name(caller)?.rsplit_once("::")?.0;
        (
            format!("{owner}::{name}"),
            "rust_self_method",
            SELF_METHOD_CONFIDENCE,
        )
    } else {
        let receivers = receiver_types(caller);
        let binding = receivers.get(receiver)?.as_ref()?;
        let type_qualified = type_index.get(&binding.type_name)?.as_ref()?;
        if matches!(binding.kind, "trait_object" | "impl_trait") {
            (
                format!("{type_qualified}::{name}"),
                "rust_dyn_dispatch_method",
                DYN_DISPATCH_CONFIDENCE,
            )
        } else {
            (
                format!("{type_qualified}::{name}"),
                "rust_typed_receiver_method",
                TYPED_RECEIVER_CONFIDENCE,
            )
        }
    };
    let target = by_qualified_name.get(qualified.as_str()).copied()?;
    if !matches!(target.kind, EntityKind::Function) {
        return None;
    }
    Some((target, reason, confidence))
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

/// Resolve a supertrait path only when its import bindings identify one in-snapshot trait.
///
/// Unlike ordinary call resolution, a glob import can make a bare trait name ambiguous. Do not
/// let the first matching `use path::*` silently choose one of several possible supertraits.
fn resolve_supertrait_path(
    current_module: &str,
    path: &str,
    imports: &[String],
    all_qualified_names: &HashSet<&str>,
    trait_qualified_names: &HashSet<&str>,
) -> Option<String> {
    let local = qualify(current_module, path);
    if trait_qualified_names.contains(local.as_str()) {
        return Some(local);
    }

    // Explicit crate/self/super paths have one canonical interpretation, independent of imports.
    if path.starts_with("crate::") || path.starts_with("self::") || path.starts_with("super::") {
        return resolve_path(current_module, path, imports, trait_qualified_names);
    }

    let first_segment = path.split("::").next().unwrap_or(path);
    let mut candidates = HashSet::new();
    for import in imports {
        let import_last_segment = import.split("::").last().unwrap_or(import);
        if import_last_segment == first_segment {
            let imported_path = resolve_path(current_module, import, &[], all_qualified_names)
                .unwrap_or_else(|| import.to_string());
            let suffix = if path.contains("::") {
                path.strip_prefix(first_segment).unwrap_or("")
            } else {
                ""
            };
            let resolved = format!("{imported_path}{suffix}");
            if all_qualified_names.contains(resolved.as_str()) {
                candidates.insert(resolved);
            }
        } else if import_last_segment == "*" {
            let Some(imported_module) = import.strip_suffix("::*") else {
                continue;
            };
            let imported_module = match imported_module {
                "self" => Some(current_module.to_string()),
                "super" => current_module
                    .rsplit_once("::")
                    .map(|(parent, _)| parent.to_string()),
                "crate" => current_module.split("::").next().map(str::to_string),
                _ => Some(
                    resolve_path(current_module, imported_module, &[], all_qualified_names)
                        .unwrap_or_else(|| imported_module.to_string()),
                ),
            };
            let Some(imported_module) = imported_module else {
                continue;
            };
            let resolved = qualify(&imported_module, path);
            if all_qualified_names.contains(resolved.as_str()) {
                candidates.insert(resolved);
            }
        }
    }

    match candidates.len() {
        1 => candidates
            .into_iter()
            .next()
            .filter(|candidate| trait_qualified_names.contains(candidate.as_str())),
        0 => resolve_path(current_module, path, imports, trait_qualified_names),
        _ => None,
    }
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

/// Maps each trait declaration to the types that implement it, in a stable order.
///
/// Both endpoints resolve by unique symbol name, and a trait implemented twice by
/// the same type counts once, so the same evidence that produces an `Implements`
/// bridge produces exactly one candidate per type.
fn trait_implementors<'a>(
    facts: impl Iterator<Item = &'a Fact>,
    types_by_name: &HashMap<&str, Vec<&'a Entity>>,
    traits_by_name: &HashMap<&str, Vec<&'a Entity>>,
) -> HashMap<&'a str, Vec<&'a Entity>> {
    let mut implementors: HashMap<&str, Vec<&Entity>> = HashMap::new();
    for fact in facts {
        if fact.value.get("symbol_kind").and_then(|value| value.as_str())
            != Some("trait_impl")
        {
            continue;
        }
        let Some(self_type) = fact.value.get("self_type").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(trait_name) = fact.value.get("trait_name").and_then(|value| value.as_str()) else {
            continue;
        };
        let (Some(type_entity), Some(trait_entity)) = (
            unique_by_name(types_by_name, self_type),
            unique_by_name(traits_by_name, trait_name),
        ) else {
            continue;
        };
        let Some(trait_qn) = qualified_name(trait_entity) else {
            continue;
        };
        let types = implementors.entry(trait_qn).or_default();
        if !types.iter().any(|existing| existing.id == type_entity.id) {
            types.push(type_entity);
        }
    }
    for types in implementors.values_mut() {
        types.sort_by(|left, right| {
            qualified_name(left)
                .unwrap_or_default()
                .cmp(qualified_name(right).unwrap_or_default())
        });
    }
    implementors
}

/// Records the implementations a `dyn`/`impl` method call can dispatch into.
///
/// The trait declaration the call resolves to is what dynamic dispatch invokes;
/// which body runs is decided by the value behind the receiver. Every declared
/// implementation of that method is therefore a candidate, recorded below the
/// confidence of the declaration edge and never as test coverage. The list is
/// bounded and the payload discloses how many implementations were seen.
fn dyn_candidates(
    snapshot: &SnapshotId,
    caller: &Entity,
    trait_method: &Entity,
    method: &str,
    implementors_by_trait: &HashMap<&str, Vec<&Entity>>,
    by_qualified_name: &HashMap<&str, &Entity>,
    affected_ids: &HashSet<EntityId>,
    linker: &str,
    relations: &mut Vec<Relation>,
    relation_ids: &mut HashSet<RelationId>,
) {
    let Some((trait_qn, _)) = qualified_name(trait_method).and_then(|qn| qn.rsplit_once("::"))
    else {
        return;
    };
    let Some(implementors) = implementors_by_trait.get(trait_qn) else {
        return;
    };
    let mut candidates = Vec::new();
    for implementor in implementors {
        let Some(type_qn) = qualified_name(implementor) else {
            continue;
        };
        let method_qn = format!("{type_qn}::{method}");
        let Some(target) = by_qualified_name.get(method_qn.as_str()) else {
            continue;
        };
        if target.kind != EntityKind::Function || target.id == caller.id {
            continue;
        }
        if !either_affected(caller, target, affected_ids) {
            continue;
        }
        candidates.push(*target);
    }
    let truncated = candidates.len() > MAX_DYN_CANDIDATES;
    candidates.truncate(MAX_DYN_CANDIDATES);
    let candidate_count = candidates.len();
    for target in candidates {
        let mut candidate = relation(
            snapshot,
            caller,
            target,
            RelationKind::Calls,
            linker,
            "rust_dyn_dispatch_candidate",
            &format!("{trait_qn}::{method}"),
            DYN_CANDIDATE_CONFIDENCE,
            RelationStatus::Inferred,
        );
        if let Some(payload) = candidate.payload.as_object_mut() {
            payload.insert("candidate_count".to_string(), json!(candidate_count));
            payload.insert("truncated".to_string(), json!(truncated));
        }
        push_unique(relations, relation_ids, candidate);
    }
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

    fn call_site(kind: &str, callee: &str, name: &str) -> serde_json::Value {
        json!({
            "callee": callee,
            "name": name,
            "receiver": if kind == "method" {
                Some(callee.rsplit_once('.').map(|(receiver, _)| receiver).unwrap_or("self"))
            } else {
                None
            },
            "kind": kind,
            "line": 4,
        })
    }

    fn typed_caller(
        id: &str,
        qualified_name: &str,
        bindings: serde_json::Value,
        calls: serde_json::Value,
    ) -> Entity {
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Function,
            qualified_name
                .rsplit_once("::")
                .map(|(_, name)| name)
                .unwrap_or(qualified_name),
            json!({ "qualified_name": qualified_name, "typed_bindings": bindings, "calls": calls }),
        )
    }

    fn trait_method(id: &str, qualified_name: &str) -> Entity {
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Function,
            qualified_name
                .rsplit_once("::")
                .map(|(_, name)| name)
                .unwrap_or(qualified_name),
            json!({ "qualified_name": qualified_name, "symbol_kind": "trait_method" }),
        )
    }

    fn type_entity(id: &str, qualified_name: &str, symbol_kind: &str) -> Entity {
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Symbol,
            qualified_name
                .rsplit_once("::")
                .map(|(_, name)| name)
                .unwrap_or(qualified_name),
            json!({ "qualified_name": qualified_name, "symbol_kind": symbol_kind }),
        )
    }

    fn method(id: &str, qualified_name: &str) -> Entity {
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Function,
            qualified_name.rsplit_once("::").map(|(_, name)| name).unwrap_or(qualified_name),
            json!({ "qualified_name": qualified_name, "symbol_kind": "method" }),
        )
    }

    fn test_caller(
        id: &str,
        qualified_name: &str,
        bindings: serde_json::Value,
        calls: serde_json::Value,
    ) -> Entity {
        let mut test_case = typed_caller(id, qualified_name, bindings, calls);
        test_case.kind = EntityKind::TestCase;
        test_case
    }

    fn impl_fact(
        id: &str,
        subject: &str,
        self_type: &str,
        trait_name: &str,
        trait_path: &str,
    ) -> Fact {
        Fact {
            id: FactId(id.to_string()),
            kind: FactKind::SymbolDefined,
            subject: EntityId(subject.to_string()),
            object: None,
            value: json!({
                "symbol_kind": "trait_impl",
                "self_type": self_type,
                "trait_name": trait_name,
                "trait_path": trait_path,
            }),
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            extractor: "rust".to_string(),
            confidence: 1.0,
        }
    }

    async fn link_all(entities: Vec<Entity>, facts: Vec<Fact>) -> Vec<Relation> {
        let snapshot = SnapshotId("snap_test".to_string());
        let affected = AffectedSubset::from_extracted(entities.clone(), facts.clone());
        RustLinker
            .link(LinkInput {
                snapshot,
                entities: Arc::new(entities),
                facts: Arc::new(facts),
                affected,
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn records_the_implementations_a_dynamic_call_can_reach() {
        let reader_trait = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let file_type = type_entity("ent_type_file", "crate::app::FileReader", "struct");
        let net_type = type_entity("ent_type_net", "crate::app::NetReader", "struct");
        let trait_read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let file_read = method("ent_file_read", "crate::app::FileReader::read");
        let net_read = method("ent_net_read", "crate::app::NetReader::read");
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "reader", "type": "Reader", "kind": "trait_object", "line": 3}]),
            json!([call_site("method", "reader.read", "read")]),
        );

        let entities = vec![
            reader_trait,
            file_type,
            net_type,
            trait_read.clone(),
            file_read.clone(),
            net_read.clone(),
            caller.clone(),
        ];
        let facts = vec![
            impl_fact("fact_file", "ent_type_file", "FileReader", "Reader", "crate::app::Reader"),
            impl_fact("fact_net", "ent_type_net", "NetReader", "Reader", "crate::app::Reader"),
        ];
        let relations = link_all(entities, facts).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);

        let declaration = calls
            .iter()
            .find(|relation| relation.to == trait_read.id)
            .expect("the trait declaration edge");
        assert_eq!(declaration.payload["reason"], "rust_dyn_dispatch_method");
        assert_eq!(declaration.confidence, 0.7);

        let candidates = calls
            .iter()
            .filter(|relation| relation.payload["reason"] == "rust_dyn_dispatch_candidate")
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|relation| relation.to == file_read.id));
        assert!(candidates.iter().any(|relation| relation.to == net_read.id));
        assert!(
            candidates
                .iter()
                .all(|relation| relation.confidence == 0.5
                    && relation.payload["candidate_count"] == 2
                    && relation.payload["truncated"] == false)
        );
        // Candidate edges are also `Implements` evidence, never a resolved call.
        assert!(relations.iter().any(|relation| relation.kind == RelationKind::Implements));
    }

    #[tokio::test]
    async fn a_concrete_receiver_claims_no_candidates() {
        let reader_trait = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let file_type = type_entity("ent_type_file", "crate::app::FileReader", "struct");
        let trait_read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let file_read = method("ent_file_read", "crate::app::FileReader::read");
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "file", "type": "FileReader", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "file.read", "read")]),
        );

        let entities = vec![
            reader_trait,
            file_type,
            trait_read,
            file_read.clone(),
            caller.clone(),
        ];
        let facts = vec![impl_fact(
            "fact_file",
            "ent_type_file",
            "FileReader",
            "Reader",
            "crate::app::Reader",
        )];
        let relations = link_all(entities, facts).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, file_read.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn the_candidate_list_is_bounded_and_disclosed() {
        let reader_trait = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let trait_read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "reader", "type": "Reader", "kind": "impl_trait", "line": 3}]),
            json!([call_site("method", "reader.read", "read")]),
        );

        let mut entities = vec![reader_trait, trait_read, caller.clone()];
        let mut facts = Vec::new();
        for index in 0..10 {
            let name = format!("Reader{index}");
            entities.push(method(
                &format!("ent_method_{index}"),
                &format!("crate::app::{name}::read"),
            ));
            entities.push(type_entity(
                &format!("ent_type_{index}"),
                &format!("crate::app::{name}"),
                "struct",
            ));
            facts.push(impl_fact(
                &format!("fact_{index}"),
                &format!("ent_type_{index}"),
                &name,
                "Reader",
                "crate::app::Reader",
            ));
        }
        let relations = link_all(entities, facts).await;

        let candidates = relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Calls
                    && relation.from == caller.id
                    && relation.payload["reason"] == "rust_dyn_dispatch_candidate"
            })
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 8);
        assert!(
            candidates
                .iter()
                .all(|relation| relation.payload["candidate_count"] == 8
                    && relation.payload["truncated"] == true)
        );
    }

    #[tokio::test]
    async fn candidates_do_not_become_test_coverage() {
        let reader_trait = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let file_type = type_entity("ent_type_file", "crate::app::FileReader", "struct");
        let trait_read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let file_read = method("ent_file_read", "crate::app::FileReader::read");
        let test_case = test_caller(
            "ent_test",
            "crate::app::reads",
            json!([{"name": "reader", "type": "Reader", "kind": "trait_object", "line": 3}]),
            json!([call_site("method", "reader.read", "read")]),
        );

        let entities = vec![
            reader_trait,
            file_type,
            trait_read.clone(),
            file_read.clone(),
            test_case.clone(),
        ];
        let facts = vec![impl_fact(
            "fact_file",
            "ent_type_file",
            "FileReader",
            "Reader",
            "crate::app::Reader",
        )];
        let relations = link_all(entities, facts).await;

        let tested = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::TestedBy)
            .collect::<Vec<_>>();
        assert_eq!(tested.len(), 1);
        assert_eq!(tested[0].to, test_case.id);
        assert_eq!(tested[0].from, trait_read.id);
    }

    #[tokio::test]
    async fn resolves_method_calls_through_declared_receiver_types() {
        let trait_type = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let struct_type = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let write = trait_method("ent_store_write", "crate::app::Store::write");
        let dyn_caller = typed_caller(
            "ent_run_dyn",
            "crate::app::run_dyn",
            json!([{"name": "reader", "type": "Reader", "kind": "trait_object", "line": 3}]),
            json!([call_site("method", "reader.read", "read")]),
        );
        let typed_caller_entity = typed_caller(
            "ent_run_typed",
            "crate::app::run_typed",
            json!([{"name": "store", "type": "Store", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "store.write", "write")]),
        );
        let self_caller = typed_caller(
            "ent_run_self",
            "crate::app::Store::flush",
            json!([]),
            json!([call_site("method", "self.write", "write")]),
        );

        let entities = vec![
            trait_type,
            struct_type,
            read.clone(),
            write.clone(),
            dyn_caller.clone(),
            typed_caller_entity.clone(),
            self_caller.clone(),
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

        let calls = |from: &Entity| {
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls && relation.from == from.id)
                .collect::<Vec<_>>()
        };

        let dyn_calls = calls(&dyn_caller);
        assert_eq!(dyn_calls.len(), 1);
        assert_eq!(dyn_calls[0].to, read.id);
        assert_eq!(dyn_calls[0].payload["reason"], "rust_dyn_dispatch_method");
        assert_eq!(dyn_calls[0].confidence, 0.7);

        let typed_calls = calls(&typed_caller_entity);
        assert_eq!(typed_calls.len(), 1);
        assert_eq!(typed_calls[0].to, write.id);
        assert_eq!(
            typed_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
        assert_eq!(typed_calls[0].confidence, 0.8);

        let self_calls = calls(&self_caller);
        assert_eq!(self_calls.len(), 1);
        assert_eq!(self_calls[0].to, write.id);
        assert_eq!(self_calls[0].payload["reason"], "rust_self_method");
        assert_eq!(self_calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn unproven_receivers_stay_unresolved() {
        let trait_type = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let untyped = typed_caller(
            "ent_run_untyped",
            "crate::app::run_untyped",
            json!([]),
            json!([call_site("method", "reader.read", "read")]),
        );
        let ambiguous = typed_caller(
            "ent_run_ambiguous",
            "crate::app::run_ambiguous",
            json!([
                {"name": "reader", "type": "Reader", "kind": "trait_object", "line": 3},
                {"name": "reader", "type": "Other", "kind": "annotation", "line": 4}
            ]),
            json!([call_site("method", "reader.read", "read")]),
        );
        let nested = typed_caller(
            "ent_run_nested",
            "crate::app::run_nested",
            json!([{"name": "reader", "type": "Reader", "kind": "trait_object", "line": 3}]),
            json!([call_site("method", "self.reader.read", "read")]),
        );

        let entities = vec![trait_type, read, untyped, ambiguous, nested];
        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone().into(),
                facts: Vec::new().into(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped, ambiguous, or nested receiver must not resolve"
        );
    }

    #[tokio::test]
    async fn an_ambiguous_type_name_owns_no_method() {
        let first = type_entity("ent_reader_one", "crate::one::Reader", "trait");
        let second = type_entity("ent_reader_two", "crate::two::Reader", "trait");
        let read = trait_method("ent_reader_read", "crate::one::Reader::read");
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "reader", "type": "Reader", "kind": "trait_object", "line": 3}]),
            json!([call_site("method", "reader.read", "read")]),
        );

        let entities = vec![first, second, read, caller];
        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone().into(),
                facts: Vec::new().into(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a type name declared twice owns neither declaration"
        );
    }

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
            json!({
                "qualified_name": "crate::auth::login",
                "calls": [call_site("call", "Session::refresh", "refresh")]
            }),
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
            json!({
                "qualified_name": "crate::auth::test_login",
                "calls": [call_site("call", "login", "login")]
            }),
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
    async fn links_trait_supertraits_as_extends_bridges() {
        let base_trait = entity(
            "ent_trait_base",
            "symbol://rust:crate::auth::Base",
            EntityKind::Symbol,
            "Base",
            json!({
                "symbol_kind": "trait",
                "qualified_name": "crate::auth::Base",
                "supertraits": [],
            }),
        );
        let child_trait = entity(
            "ent_trait_child",
            "symbol://rust:crate::auth::Child",
            EntityKind::Symbol,
            "Child",
            json!({
                "symbol_kind": "trait",
                "qualified_name": "crate::auth::Child",
                "supertraits": ["Base"],
            }),
        );
        let module = entity(
            "ent_module_auth",
            "symbol://rust:crate::auth",
            EntityKind::Module,
            "auth",
            json!({
                "symbol_kind": "module",
                "qualified_name": "crate::auth",
                "imports": [],
            }),
        );
        let entities = vec![module, base_trait.clone(), child_trait.clone()];

        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Extends
                && relation.from == child_trait.id
                && relation.to == base_trait.id
                && relation.payload["reason"] == "rust_trait_supertrait"
                && relation.payload["matched_value"] == "Base"
        }));
        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Implements)
        );
    }

    #[tokio::test]
    async fn resolves_imported_supertraits_and_rejects_ambiguous_globs() {
        let base = rust_trait_symbol(
            "ent_trait_imported_base",
            "crate::one::Base",
            "Base",
            &[],
            &[],
        );
        let child = rust_trait_symbol(
            "ent_trait_imported_child",
            "crate::auth::Child",
            "Child",
            &["Base"],
            &["crate::one::Base"],
        );
        let module = rust_module_symbol(
            "ent_module_auth_imports",
            "crate::auth",
            &["crate::one::Base"],
        );
        let entities = vec![module, base.clone(), child.clone()];

        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(relations.iter().any(|relation| {
            relation.kind == RelationKind::Extends
                && relation.from == child.id
                && relation.to == base.id
        }));

        let other_base =
            rust_trait_symbol("ent_trait_other_base", "crate::two::Base", "Base", &[], &[]);
        let ambiguous_child = rust_trait_symbol(
            "ent_trait_ambiguous_child",
            "crate::auth::Child",
            "Child",
            &["Base"],
            &["crate::one::*", "crate::two::*"],
        );
        let ambiguous_module = rust_module_symbol(
            "ent_module_auth_ambiguous_imports",
            "crate::auth",
            &["crate::one::*", "crate::two::*"],
        );
        let ambiguous_entities = vec![ambiguous_module, base, other_base, ambiguous_child.clone()];
        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(ambiguous_entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(ambiguous_entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(!relations.iter().any(|relation| {
            relation.kind == RelationKind::Extends && relation.from == ambiguous_child.id
        }));
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

    fn rust_trait_symbol(
        id: &str,
        qualified_name: &str,
        name: &str,
        supertraits: &[&str],
        imports: &[&str],
    ) -> Entity {
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Symbol,
            name,
            json!({
                "symbol_kind": "trait",
                "qualified_name": qualified_name,
                "supertraits": supertraits,
                "imports": imports,
            }),
        )
    }

    fn rust_module_symbol(id: &str, qualified_name: &str, imports: &[&str]) -> Entity {
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Module,
            qualified_name.rsplit("::").next().unwrap_or(qualified_name),
            json!({
                "symbol_kind": "module",
                "qualified_name": qualified_name,
                "imports": imports,
            }),
        )
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
