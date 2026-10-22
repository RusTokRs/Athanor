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
/// Call hops a factory-result receiver may take: `create_repo().get()` is two.
const MAX_FACTORY_HOPS: usize = 2;

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
        let implementors_by_trait =
            trait_implementors(input.facts.iter(), &types_by_name, &traits_by_name);
        let candidate_context = DynCandidateContext {
            snapshot: &input.snapshot,
            implementors: &implementors_by_trait,
            by_qualified_name: &rust_by_qualified_name,
            affected: &affected_ids,
            linker: self.name(),
        };

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
                                        &imports,
                                        &qualified_names,
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
                                        &candidate_context,
                                        entity,
                                        target,
                                        name,
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
/// `self` dispatches inside its own impl. `self.field` walks the named field
/// the owner struct declares, one hop, after wrapping parentheses and a
/// leading `&`/`mut` are peeled. `create_repo().write()` takes the type the
/// factory declares it returns. `svc.create().write()` and `self.create().write()`
/// take the type that method declares it returns. `create_repo().get().write()`
/// and `svc.create().get().write()` take one extra hop through that inner
/// method's return. `Store::new().write()` takes the type that associated
/// function declares it returns. `Store::new().get().write()` takes one extra
/// hop through that inner method's return. `let store = create_repo(); store.write()`,
/// `let store = Store::new(); store.write()`,
/// `let store = svc.create(); store.write()`, and
/// `let store = create_repo().get(); store.write()`,
/// `let store = crate::create_repo().get(); store.write()`, and
/// `let store = create_repo()?; store.write()`,
/// `let store = create_repo()?.get(); store.write()`, and
/// `let store = create_repo().unwrap(); store.write()` take the same factory return
/// through the binding. A trailing `?`, `.unwrap()`, or `.expect(...)` takes the
/// Ok/Some type a `Result` / `Option` factory declares. A `?` or unwrap/expect
/// on the inner hop (`create_repo()?.get().write()`, `create_repo().unwrap().get().write()`,
/// `svc.create()?.get().write()`, `Store::new()?.get().write()`,
/// `crate::create_repo()?.get().write()`) takes that yield for that hop only.
/// `crate::create_repo().write()` and
/// `crate::app::Store::new().write()` take a `crate::` / `self::` / `super::`
/// path to that factory. `crate::create_repo().get().write()` takes one extra
/// hop through that inner method's return. Any other receiver dispatches to the
/// method of its declared type. A type alias unfolds one hop, so
/// `type Client = Store` types `item.write()` as `Store::write`. A `dyn`/`impl`
/// trait receiver reaches the trait's declaration, which is what dynamic
/// dispatch calls, while a receiver the function does not type stays unresolved
/// rather than matching by name.
#[allow(clippy::too_many_arguments)]
fn resolve_typed_method<'a>(
    caller: &Entity,
    receiver: &str,
    name: &str,
    type_index: &HashMap<String, Option<&'a str>>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
    imports: &[String],
    qualified_names: &HashSet<&str>,
) -> Option<(&'a Entity, &'static str, f32)> {
    let (receiver, allow_try) = peel_receiver(receiver);
    let (qualified, reason, confidence) = if receiver == "self" {
        let owner = qualified_name(caller)?.rsplit_once("::")?.0;
        (
            format!("{owner}::{name}"),
            "rust_self_method",
            SELF_METHOD_CONFIDENCE,
        )
    } else if let Some(field) = receiver.strip_prefix("self.")
        && !field.contains('.')
        && is_field_ident(field)
    {
        // `self.reader.write()` names the field the owner struct declares. One
        // hop only: `self.a.b.write()` is not a field the source typed on self.
        // `self.create().write()` is a member factory, not a field.
        let owner_qn = qualified_name(caller)?.rsplit_once("::")?.0;
        let owner = by_qualified_name.get(owner_qn)?;
        let receivers = receiver_types(owner);
        let binding = receivers.get(field)?.as_ref()?;
        typed_dispatch(binding, type_index, by_qualified_name, name)?
    } else if let Some(factory) = factory_call_name(&receiver) {
        let binding = factory_return_type(
            caller,
            factory,
            imports,
            qualified_names,
            by_qualified_name,
            allow_try,
        )?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some((owner, method)) = member_factory_call(&receiver) {
        let binding = member_factory_return(
            caller,
            owner,
            method,
            type_index,
            by_qualified_name,
            allow_try,
        )?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some((factory, method, factory_tried)) = nested_factory_call(&receiver) {
        let inner = factory_return_type(
            caller,
            factory,
            imports,
            qualified_names,
            by_qualified_name,
            factory_tried,
        )?;
        let binding = type_method_return(&inner, method, type_index, by_qualified_name, allow_try)?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some((owner, factory_method, method, factory_tried)) =
        nested_member_factory_call(&receiver)
    {
        let inner = member_factory_return(
            caller,
            owner,
            factory_method,
            type_index,
            by_qualified_name,
            factory_tried,
        )?;
        let binding = type_method_return(&inner, method, type_index, by_qualified_name, allow_try)?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some((type_name, method)) = associated_factory_call(&receiver) {
        let binding = ReceiverType {
            type_name: type_name.to_string(),
            kind: "annotation",
        };
        let binding =
            type_method_return(&binding, method, type_index, by_qualified_name, allow_try)?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some((type_name, factory_method, method, factory_tried)) =
        nested_associated_factory_call(&receiver)
    {
        let inner = ReceiverType {
            type_name: type_name.to_string(),
            kind: "annotation",
        };
        let inner = type_method_return(
            &inner,
            factory_method,
            type_index,
            by_qualified_name,
            factory_tried,
        )?;
        let binding = type_method_return(&inner, method, type_index, by_qualified_name, allow_try)?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some(factory) = crate_path_factory_call(&receiver) {
        let binding = factory_return_type(
            caller,
            factory,
            imports,
            qualified_names,
            by_qualified_name,
            allow_try,
        )?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else if let Some((factory, method, factory_tried)) = nested_crate_path_factory_call(&receiver)
    {
        let inner = factory_return_type(
            caller,
            factory,
            imports,
            qualified_names,
            by_qualified_name,
            factory_tried,
        )?;
        let binding = type_method_return(&inner, method, type_index, by_qualified_name, allow_try)?;
        typed_dispatch(&binding, type_index, by_qualified_name, name)?
    } else {
        let receivers = receiver_types(caller);
        if let Some(entry) = receivers.get(receiver.as_str()) {
            let binding = entry.as_ref()?;
            typed_dispatch(binding, type_index, by_qualified_name, name)?
        } else {
            let (callee, tried) = call_receivers(caller).get(receiver.as_str())?.clone()?;
            let binding = factory_callee_return(
                caller,
                &callee,
                type_index,
                imports,
                qualified_names,
                by_qualified_name,
                tried,
            )?;
            typed_dispatch(&binding, type_index, by_qualified_name, name)?
        }
    };
    let target = by_qualified_name.get(qualified.as_str()).copied()?;
    if !matches!(target.kind, EntityKind::Function) {
        return None;
    }
    Some((target, reason, confidence))
}

fn typed_dispatch<'a>(
    binding: &ReceiverType,
    type_index: &HashMap<String, Option<&'a str>>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
    name: &str,
) -> Option<(String, &'static str, f32)> {
    let type_name = unfold_alias_type(&binding.type_name, type_index, by_qualified_name);
    let type_qualified = type_index.get(&type_name)?.as_ref()?;
    if matches!(binding.kind, "trait_object" | "impl_trait") {
        Some((
            format!("{type_qualified}::{name}"),
            "rust_dyn_dispatch_method",
            DYN_DISPATCH_CONFIDENCE,
        ))
    } else {
        Some((
            format!("{type_qualified}::{name}"),
            "rust_typed_receiver_method",
            TYPED_RECEIVER_CONFIDENCE,
        ))
    }
}

/// Strips wrapping parentheses, a leading `&`/`mut`, a trailing `?`, a trailing
/// `.unwrap()` / `.expect(...)` on a factory call, and pretty-printed spaces
/// around `.`, `::`, and `()` so `(&mut self.reader)`, `self . reader`,
/// `Store :: new ()`, `svc . create ()`, `create_repo()?`, and
/// `create_repo().unwrap()` walk as `self.reader` / `Store::new()` /
/// `svc.create()` / `create_repo()`. The bool is whether a trailing try was
/// peeled. A `?` on an inner hop stays so `create_repo()?.get()` still names
/// that hop. An inner unwrap (`create_repo().unwrap().get()`) folds to `?` on
/// that hop.
fn peel_receiver(receiver: &str) -> (String, bool) {
    let mut text = compact_receiver_dots(receiver);
    let mut tried = false;
    loop {
        let trimmed = compact_receiver_dots(&text);
        let trimmed = trimmed.trim();
        if let Some(folded) = fold_one_inner_unwrap_or_expect(trimmed) {
            text = folded;
            continue;
        }
        if let Some(inner) = trimmed
            .strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
        {
            text = inner.to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('&') {
            text = rest.to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("mut ") {
            text = rest.to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_suffix('?') {
            tried = true;
            text = rest.to_string();
            continue;
        }
        if let Some(rest) = peel_trailing_unwrap_or_expect(trimmed) {
            if rest.contains("()") {
                tried = true;
                text = rest.to_string();
                continue;
            }
        }
        return (trimmed.to_string(), tried);
    }
}

/// Folds one inner `.unwrap().` / `.expect(...).` into `?.` so
/// `create_repo().unwrap().get()` is `create_repo()?.get()`. Trailing unwrap
/// stays for `peel_trailing_unwrap_or_expect`.
fn fold_one_inner_unwrap_or_expect(text: &str) -> Option<String> {
    if let Some(idx) = text.find(".unwrap().") {
        let mut folded = String::with_capacity(text.len() - ".unwrap()".len() + 1);
        folded.push_str(&text[..idx]);
        folded.push_str("?.");
        folded.push_str(&text[idx + ".unwrap().".len()..]);
        return Some(folded);
    }
    let mut search = 0;
    while let Some(rel) = text[search..].find(".expect") {
        let idx = search + rel;
        let rest = &text[idx + ".expect".len()..];
        let after = rest.trim_start();
        if !after.starts_with('(') {
            search = idx + 1;
            continue;
        }
        let open = text.len() - after.len();
        let Some(close) = matching_close_paren(text, open) else {
            search = idx + 1;
            continue;
        };
        let Some(following_raw) = text.get(close + 1..) else {
            search = idx + 1;
            continue;
        };
        let following = following_raw.trim_start();
        if !following.starts_with('.') {
            search = idx + 1;
            continue;
        }
        let dot_at = text.len() - following.len();
        let mut folded = String::with_capacity(text.len());
        folded.push_str(&text[..idx]);
        folded.push('?');
        folded.push_str(&text[dot_at..]);
        return Some(folded);
    }
    None
}

/// Peels a trailing `.unwrap()` or `.expect(...)` so `create_repo().unwrap()`
/// is `create_repo()`. `store.unwrap()` and `unwrap_or` are not peeled here.
fn peel_trailing_unwrap_or_expect(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_suffix(".unwrap()") {
        return Some(rest.trim_end());
    }
    let idx = trimmed.rfind(".expect")?;
    let after = trimmed[idx + ".expect".len()..].trim_start();
    if !after.starts_with('(') {
        return None;
    }
    let open = trimmed.len() - after.len();
    if matching_close_paren(trimmed, open) != Some(trimmed.len() - 1) {
        return None;
    }
    Some(trimmed[..idx].trim_end())
}

fn matching_close_paren(text: &str, open: usize) -> Option<usize> {
    if text.as_bytes().get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in text[open..].char_indices() {
        let at = open + idx;
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

/// Peels a trailing `?` from one hop: `create_repo()?` is `create_repo()` plus tried.
fn peel_hop_try(hop: &str) -> (&str, bool) {
    let trimmed = hop.trim();
    match trimmed.strip_suffix('?') {
        Some(rest) => (rest.trim_end(), true),
        None => (trimmed, false),
    }
}

/// The factory an identifier call names: `create_repo()` is `create_repo`.
///
/// A second hop (`create_repo()()`) and a path that is not one identifier name
/// nothing. A method after the call is `nested_factory_call`.
fn factory_call_name(receiver: &str) -> Option<&str> {
    let name = receiver.trim().strip_suffix("()")?.trim_end();
    if name.contains(['(', ')', '.', ':']) || !is_field_ident(name) {
        return None;
    }
    Some(name)
}

/// The method a member factory names: `svc.create()` is `svc` + `create`.
///
/// One hop only. `foo.bar.create()` names nothing. A method after the call is
/// `nested_member_factory_call`.
fn member_factory_call(receiver: &str) -> Option<(&str, &str)> {
    member_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// `svc.create` without the call: one owner identifier or `self`, then one method.
fn member_factory_name(callee: &str) -> Option<(&str, &str)> {
    if callee.contains(['(', ')', ':']) {
        return None;
    }
    let (owner, method) = callee.rsplit_once('.')?;
    if owner.contains('.') || !is_field_ident(method) {
        return None;
    }
    if owner != "self" && !is_field_ident(owner) {
        return None;
    }
    Some((owner, method))
}

/// Identifier factory then one method: `create_repo().get()` is `create_repo` + `get`.
///
/// Exactly two call hops. A `?` on the inner hop (`create_repo()?.get()`) is
/// peeled and returned as tried. `create_repo()()` and `create_repo().a().b()`
/// name nothing.
fn nested_factory_call(receiver: &str) -> Option<(&str, &str, bool)> {
    if receiver.matches("()").count() != MAX_FACTORY_HOPS {
        return None;
    }
    nested_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// `create_repo().get` without the trailing call: one identifier factory, then one method.
/// A `?` on the inner hop (`create_repo()?.get`) is peeled and returned as tried.
fn nested_factory_name(callee: &str) -> Option<(&str, &str, bool)> {
    if callee.matches("()").count() != 1 {
        return None;
    }
    let (factory, method) = callee.rsplit_once('.')?;
    if !is_field_ident(method) {
        return None;
    }
    let (factory, factory_tried) = peel_hop_try(factory);
    Some((factory_call_name(factory)?, method, factory_tried))
}

/// Member factory then one method: `svc.create().get()` is `svc` + `create` + `get`.
///
/// Exactly two call hops. A `?` on the inner hop (`svc.create()?.get()`) is
/// peeled and returned as tried. `svc.create().inner` and `foo.bar.create().get()`
/// name nothing.
fn nested_member_factory_call(receiver: &str) -> Option<(&str, &str, &str, bool)> {
    if receiver.matches("()").count() != MAX_FACTORY_HOPS {
        return None;
    }
    nested_member_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// `svc.create().get` without the trailing call.
fn nested_member_factory_name(callee: &str) -> Option<(&str, &str, &str, bool)> {
    if callee.matches("()").count() != 1 {
        return None;
    }
    let (prefix, method) = callee.rsplit_once('.')?;
    if !is_field_ident(method) {
        return None;
    }
    let (prefix, factory_tried) = peel_hop_try(prefix);
    let (owner, factory_method) = member_factory_call(prefix)?;
    Some((owner, factory_method, method, factory_tried))
}

/// Associated function factory: `Store::new()` is `Store` + `new`.
///
/// One hop only. `crate::Store::new()` is `crate_path_factory_call`.
/// A method after the call is `nested_associated_factory_call`.
fn associated_factory_call(receiver: &str) -> Option<(&str, &str)> {
    associated_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// Associated factory then one method: `Store::new().get()` is `Store` + `new` + `get`.
///
/// Exactly two call hops. A `?` on the inner hop (`Store::new()?.get()`) is
/// peeled and returned as tried. `crate::Store::new().get()` is
/// `nested_crate_path_factory_call`. `Store::new().a().b()` names nothing.
fn nested_associated_factory_call(receiver: &str) -> Option<(&str, &str, &str, bool)> {
    if receiver.matches("()").count() != MAX_FACTORY_HOPS {
        return None;
    }
    nested_associated_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// `Store::new().get` without the trailing call.
fn nested_associated_factory_name(callee: &str) -> Option<(&str, &str, &str, bool)> {
    if callee.matches("()").count() != 1 {
        return None;
    }
    let (prefix, method) = callee.rsplit_once('.')?;
    if !is_field_ident(method) {
        return None;
    }
    let (prefix, factory_tried) = peel_hop_try(prefix);
    let (type_name, factory_method) = associated_factory_call(prefix)?;
    Some((type_name, factory_method, method, factory_tried))
}

/// `Store::new` without the call: one type identifier and one method.
///
/// `crate::create_repo` is a crate-path factory, not a type named `crate`.
fn associated_factory_name(callee: &str) -> Option<(&str, &str)> {
    if callee.contains(['(', ')', '.']) {
        return None;
    }
    let (type_name, method) = callee.rsplit_once("::")?;
    if type_name.contains(':')
        || matches!(type_name, "crate" | "self" | "super")
        || !is_field_ident(type_name)
        || !is_field_ident(method)
    {
        return None;
    }
    Some((type_name, method))
}

/// A `crate::` / `self::` / `super::` factory call: `crate::create_repo()` or
/// `crate::app::Store::new()`.
///
/// Relative `app::create_repo()` stays an associated factory or unresolved.
/// A method after the call is `nested_crate_path_factory_call`.
fn crate_path_factory_call(receiver: &str) -> Option<&str> {
    crate_path_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// Crate-path factory then one method: `crate::create_repo().get()` is
/// `crate::create_repo` + `get`.
///
/// Exactly two call hops. A `?` on the inner hop (`crate::create_repo()?.get()`)
/// is peeled and returned as tried. `crate::create_repo().a().b()` names nothing.
fn nested_crate_path_factory_call(receiver: &str) -> Option<(&str, &str, bool)> {
    if receiver.matches("()").count() != MAX_FACTORY_HOPS {
        return None;
    }
    nested_crate_path_factory_name(receiver.trim().strip_suffix("()")?.trim_end())
}

/// `crate::create_repo().get` without the trailing call.
fn nested_crate_path_factory_name(callee: &str) -> Option<(&str, &str, bool)> {
    if callee.matches("()").count() != 1 {
        return None;
    }
    let (factory, method) = callee.rsplit_once('.')?;
    if !is_field_ident(method) {
        return None;
    }
    let (factory, factory_tried) = peel_hop_try(factory);
    Some((crate_path_factory_call(factory)?, method, factory_tried))
}

/// `crate::create_repo` without the call.
fn crate_path_factory_name(callee: &str) -> Option<&str> {
    if callee.contains(['(', ')', '.']) {
        return None;
    }
    let mut segments = callee.split("::");
    let first = segments.next()?;
    if !matches!(first, "crate" | "self" | "super") {
        return None;
    }
    let mut rest = 0usize;
    for segment in segments {
        if !is_field_ident(segment) {
            return None;
        }
        rest += 1;
    }
    if rest == 0 {
        return None;
    }
    Some(callee)
}

/// Reads the bindings one function initializes with a factory call.
///
/// A name bound to two different factories is ambiguous and names nothing.
fn call_receivers(entity: &Entity) -> HashMap<String, Option<(String, bool)>> {
    let mut receivers: HashMap<String, Option<(String, bool)>> = HashMap::new();
    let Some(bindings) = entity
        .payload
        .get("call_bindings")
        .and_then(|value| value.as_array())
    else {
        return receivers;
    };
    for binding in bindings {
        let (Some(name), Some(callee)) = (
            binding.get("name").and_then(|value| value.as_str()),
            binding.get("callee").and_then(|value| value.as_str()),
        ) else {
            continue;
        };
        if name.is_empty() || callee.is_empty() {
            continue;
        }
        let tried = binding
            .get("try")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let entry = (callee.to_string(), tried);
        let prior = receivers.get(name).cloned();
        match prior {
            None => {
                receivers.insert(name.to_string(), Some(entry));
            }
            Some(Some(existing)) => {
                if existing != entry {
                    receivers.insert(name.to_string(), None);
                }
            }
            Some(None) => {}
        }
    }
    receivers
}

/// The type a `let store = create_repo()`, `let store = Store::new()`,
/// `let store = svc.create()`, `let store = crate::create_repo()`,
/// `let store = create_repo().get()`, `let store = crate::create_repo().get()`,
/// `let store = create_repo()?`, `let store = create_repo()?.get()`,
/// `let store = create_repo().unwrap()`, or
/// `let store = create_repo().unwrap().get()` factory
/// declares it returns.
#[allow(clippy::too_many_arguments)]
fn factory_callee_return<'a>(
    caller: &Entity,
    callee: &str,
    type_index: &HashMap<String, Option<&'a str>>,
    imports: &[String],
    qualified_names: &HashSet<&str>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
    allow_try: bool,
) -> Option<ReceiverType> {
    if let Some((type_name, method)) = associated_factory_name(callee) {
        let binding = ReceiverType {
            type_name: type_name.to_string(),
            kind: "annotation",
        };
        return type_method_return(&binding, method, type_index, by_qualified_name, allow_try);
    }
    if let Some((owner, method)) = member_factory_name(callee) {
        return member_factory_return(
            caller,
            owner,
            method,
            type_index,
            by_qualified_name,
            allow_try,
        );
    }
    if let Some((type_name, factory_method, method, factory_tried)) =
        nested_associated_factory_name(callee)
    {
        let inner = ReceiverType {
            type_name: type_name.to_string(),
            kind: "annotation",
        };
        let inner = type_method_return(
            &inner,
            factory_method,
            type_index,
            by_qualified_name,
            factory_tried,
        )?;
        return type_method_return(&inner, method, type_index, by_qualified_name, allow_try);
    }
    if let Some((owner, factory_method, method, factory_tried)) = nested_member_factory_name(callee)
    {
        let inner = member_factory_return(
            caller,
            owner,
            factory_method,
            type_index,
            by_qualified_name,
            factory_tried,
        )?;
        return type_method_return(&inner, method, type_index, by_qualified_name, allow_try);
    }
    if let Some((factory, method, factory_tried)) = nested_factory_name(callee) {
        let inner = factory_return_type(
            caller,
            factory,
            imports,
            qualified_names,
            by_qualified_name,
            factory_tried,
        )?;
        return type_method_return(&inner, method, type_index, by_qualified_name, allow_try);
    }
    if let Some((factory, method, factory_tried)) = nested_crate_path_factory_name(callee) {
        let inner = factory_return_type(
            caller,
            factory,
            imports,
            qualified_names,
            by_qualified_name,
            factory_tried,
        )?;
        return type_method_return(&inner, method, type_index, by_qualified_name, allow_try);
    }
    if crate_path_factory_name(callee).is_some() {
        return factory_return_type(
            caller,
            callee,
            imports,
            qualified_names,
            by_qualified_name,
            allow_try,
        );
    }
    if !is_field_ident(callee) {
        return None;
    }
    factory_return_type(
        caller,
        callee,
        imports,
        qualified_names,
        by_qualified_name,
        allow_try,
    )
}

/// The type a factory declaration states it returns, resolved like a static call.
fn factory_return_type<'a>(
    caller: &Entity,
    factory: &str,
    imports: &[String],
    qualified_names: &HashSet<&str>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
    allow_try: bool,
) -> Option<ReceiverType> {
    let parent = qualified_name(caller)?.rsplit_once("::")?.0;
    let resolved = resolve_path(parent, factory, imports, qualified_names)?;
    let entity = by_qualified_name.get(resolved.as_str()).copied()?;
    entity_return_type(entity, allow_try)
}

/// The type `owner.method()` returns: `svc.create()` reads `App::create`.
fn member_factory_return<'a>(
    caller: &Entity,
    owner: &str,
    method: &str,
    type_index: &HashMap<String, Option<&'a str>>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
    allow_try: bool,
) -> Option<ReceiverType> {
    if owner == "self" {
        let owner_qn = qualified_name(caller)?.rsplit_once("::")?.0;
        let factory = by_qualified_name
            .get(format!("{owner_qn}::{method}").as_str())
            .copied()?;
        return entity_return_type(factory, allow_try);
    }
    let receivers = receiver_types(caller);
    let binding = receivers.get(owner)?.as_ref()?;
    type_method_return(binding, method, type_index, by_qualified_name, allow_try)
}

/// The type `Type::method` declares it returns, from a receiver already typed.
fn type_method_return<'a>(
    binding: &ReceiverType,
    method: &str,
    type_index: &HashMap<String, Option<&'a str>>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
    allow_try: bool,
) -> Option<ReceiverType> {
    let type_name = unfold_alias_type(&binding.type_name, type_index, by_qualified_name);
    let type_qualified = type_index.get(&type_name)?.as_ref()?;
    let factory = by_qualified_name
        .get(format!("{type_qualified}::{method}").as_str())
        .copied()?;
    entity_return_type(factory, allow_try)
}

fn entity_return_type(entity: &Entity, allow_try: bool) -> Option<ReceiverType> {
    if !matches!(entity.kind, EntityKind::Function) {
        return None;
    }
    let try_yield = entity
        .payload
        .get("try_yield")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if try_yield != allow_try {
        return None;
    }
    let type_name = entity
        .payload
        .get("return_type")
        .and_then(|value| value.as_str())
        .filter(|type_name| !type_name.is_empty())?
        .to_string();
    let kind = match entity
        .payload
        .get("return_kind")
        .and_then(|value| value.as_str())
    {
        Some("trait_object") => "trait_object",
        Some("impl_trait") => "impl_trait",
        _ => "annotation",
    };
    Some(ReceiverType { type_name, kind })
}

fn compact_receiver_dots(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" . ", ".")
        .replace(" .", ".")
        .replace(". ", ".")
        .replace(" :: ", "::")
        .replace(" ::", "::")
        .replace(":: ", "::")
        .replace(" ()", "()")
}

fn is_field_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// One hop through a type alias the source writes: `type Client = Store` names
/// `Store`.
///
/// A generic, a tuple, and an alias that names itself stay the original name.
/// A second hop is not taken.
fn unfold_alias_type<'a>(
    type_name: &str,
    type_index: &HashMap<String, Option<&'a str>>,
    by_qualified_name: &HashMap<&'a str, &'a Entity>,
) -> String {
    let Some(Some(qualified)) = type_index.get(type_name) else {
        return type_name.to_string();
    };
    let Some(entity) = by_qualified_name.get(qualified).copied() else {
        return type_name.to_string();
    };
    if !entity
        .payload
        .get("symbol_kind")
        .and_then(|value| value.as_str())
        .is_some_and(|kind| kind == "type_alias")
    {
        return type_name.to_string();
    }
    match entity
        .payload
        .get("aliased_type")
        .and_then(|value| value.as_str())
    {
        Some(aliased) if !aliased.is_empty() && aliased != type_name => aliased.to_string(),
        _ => type_name.to_string(),
    }
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
        if fact
            .value
            .get("symbol_kind")
            .and_then(|value| value.as_str())
            != Some("trait_impl")
        {
            continue;
        }
        let Some(self_type) = fact.value.get("self_type").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(trait_name) = fact
            .value
            .get("trait_name")
            .and_then(|value| value.as_str())
        else {
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

/// The snapshot-wide inputs a dynamic call site needs to name its candidates.
struct DynCandidateContext<'a> {
    snapshot: &'a SnapshotId,
    implementors: &'a HashMap<&'a str, Vec<&'a Entity>>,
    by_qualified_name: &'a HashMap<&'a str, &'a Entity>,
    affected: &'a HashSet<EntityId>,
    linker: &'a str,
}

/// Records the implementations a `dyn`/`impl` method call can dispatch into.
///
/// The trait declaration the call resolves to is what dynamic dispatch invokes;
/// which body runs is decided by the value behind the receiver. Every declared
/// implementation of that method is therefore a candidate, recorded below the
/// confidence of the declaration edge and never as test coverage. The list is
/// bounded and the payload discloses how many implementations were seen.
fn dyn_candidates(
    context: &DynCandidateContext<'_>,
    caller: &Entity,
    trait_method: &Entity,
    method: &str,
    relations: &mut Vec<Relation>,
    relation_ids: &mut HashSet<RelationId>,
) {
    let Some((trait_qn, _)) = qualified_name(trait_method).and_then(|qn| qn.rsplit_once("::"))
    else {
        return;
    };
    let Some(implementors) = context.implementors.get(trait_qn) else {
        return;
    };
    let mut candidates = Vec::new();
    for implementor in implementors {
        let Some(type_qn) = qualified_name(implementor) else {
            continue;
        };
        let method_qn = format!("{type_qn}::{method}");
        let Some(target) = context.by_qualified_name.get(method_qn.as_str()) else {
            continue;
        };
        if target.kind != EntityKind::Function || target.id == caller.id {
            continue;
        }
        if !either_affected(caller, target, context.affected) {
            continue;
        }
        candidates.push(*target);
    }
    let truncated = candidates.len() > MAX_DYN_CANDIDATES;
    candidates.truncate(MAX_DYN_CANDIDATES);
    let candidate_count = candidates.len();
    for target in candidates {
        let mut candidate = relation(
            context.snapshot,
            caller,
            target,
            RelationKind::Calls,
            context.linker,
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

    fn factory_fn(
        id: &str,
        qualified_name: &str,
        return_type: Option<&str>,
        return_kind: Option<&str>,
    ) -> Entity {
        let mut payload = json!({
            "qualified_name": qualified_name,
            "symbol_kind": "function",
            "calls": json!([]),
        });
        if let Some(return_type) = return_type {
            payload["return_type"] = json!(return_type);
        }
        if let Some(return_kind) = return_kind {
            payload["return_kind"] = json!(return_kind);
        }
        entity(
            id,
            &format!("symbol://rust:{qualified_name}"),
            EntityKind::Function,
            qualified_name
                .rsplit_once("::")
                .map(|(_, name)| name)
                .unwrap_or(qualified_name),
            payload,
        )
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
            qualified_name
                .rsplit_once("::")
                .map(|(_, name)| name)
                .unwrap_or(qualified_name),
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
            impl_fact(
                "fact_file",
                "ent_type_file",
                "FileReader",
                "Reader",
                "crate::app::Reader",
            ),
            impl_fact(
                "fact_net",
                "ent_type_net",
                "NetReader",
                "Reader",
                "crate::app::Reader",
            ),
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
        assert!(
            candidates
                .iter()
                .any(|relation| relation.to == file_read.id)
        );
        assert!(candidates.iter().any(|relation| relation.to == net_read.id));
        assert!(candidates.iter().all(|relation| relation.confidence == 0.5
            && relation.payload["candidate_count"] == 2
            && relation.payload["truncated"] == false));
        // Candidate edges are also `Implements` evidence, never a resolved call.
        assert!(
            relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Implements)
        );
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
            "an untyped, ambiguous, or free-function nested receiver must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_through_a_struct_field() {
        let mut app = type_entity("ent_struct_app", "crate::app::App", "struct");
        app.payload["typed_bindings"] = json!([
            {"name": "reader", "type": "Store", "kind": "annotation", "line": 3},
            {"name": "boxed", "type": "Reader", "kind": "trait_object", "line": 4},
            {"name": "client", "type": "Client", "kind": "annotation", "line": 5}
        ]);
        let mut client = type_entity("ent_alias_client", "crate::app::Client", "type_alias");
        client.payload["aliased_type"] = json!("Store");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let reader = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let write = method("ent_store_write", "crate::app::Store::write");
        let read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let field_caller = typed_caller(
            "ent_start",
            "crate::app::App::start",
            json!([]),
            json!([call_site("method", "self.reader.write", "write")]),
        );
        let ref_caller = typed_caller(
            "ent_flush",
            "crate::app::App::flush",
            json!([]),
            json!([call_site("method", "&self.reader.write", "write")]),
        );
        let dyn_caller = typed_caller(
            "ent_read_all",
            "crate::app::App::read_all",
            json!([]),
            json!([call_site("method", "self.boxed.read", "read")]),
        );
        let alias_caller = typed_caller(
            "ent_via_alias",
            "crate::app::App::via_alias",
            json!([]),
            json!([call_site("method", "self.client.write", "write")]),
        );
        let paren_caller = typed_caller(
            "ent_paren",
            "crate::app::App::paren",
            json!([]),
            json!([call_site("method", "(&mut self.reader).write", "write")]),
        );

        let entities = vec![
            app,
            client,
            store,
            reader,
            write.clone(),
            read.clone(),
            field_caller.clone(),
            ref_caller.clone(),
            dyn_caller.clone(),
            alias_caller.clone(),
            paren_caller.clone(),
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

        let field_calls = calls(&field_caller);
        assert_eq!(field_calls.len(), 1);
        assert_eq!(field_calls[0].to, write.id);
        assert_eq!(
            field_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
        assert_eq!(field_calls[0].confidence, 0.8);

        let ref_calls = calls(&ref_caller);
        assert_eq!(ref_calls.len(), 1);
        assert_eq!(ref_calls[0].to, write.id);
        assert_eq!(ref_calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(ref_calls[0].confidence, 0.8);

        let dyn_calls = calls(&dyn_caller);
        assert_eq!(dyn_calls.len(), 1);
        assert_eq!(dyn_calls[0].to, read.id);
        assert_eq!(dyn_calls[0].payload["reason"], "rust_dyn_dispatch_method");
        assert_eq!(dyn_calls[0].confidence, 0.7);

        let alias_calls = calls(&alias_caller);
        assert_eq!(alias_calls.len(), 1);
        assert_eq!(alias_calls[0].to, write.id);
        assert_eq!(
            alias_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
        assert_eq!(alias_calls[0].confidence, 0.8);

        let paren_calls = calls(&paren_caller);
        assert_eq!(paren_calls.len(), 1);
        assert_eq!(paren_calls[0].to, write.id);
        assert_eq!(
            paren_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
        assert_eq!(paren_calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn unproven_struct_field_receivers_stay_unresolved() {
        let mut app = type_entity("ent_struct_app", "crate::app::App", "struct");
        app.payload["typed_bindings"] = json!([
            {"name": "reader", "type": "Store", "kind": "annotation", "line": 3}
        ]);
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let nested = typed_caller(
            "ent_nested",
            "crate::app::App::nested",
            json!([]),
            json!([call_site("method", "self.reader.inner.write", "write")]),
        );
        let missing = typed_caller(
            "ent_missing",
            "crate::app::App::missing",
            json!([]),
            json!([call_site("method", "self.other.write", "write")]),
        );
        let trait_impl_owner = typed_caller(
            "ent_trait_impl",
            "crate::app::App::Writer::flush",
            json!([]),
            json!([call_site("method", "self.reader.write", "write")]),
        );
        let mut conflict = type_entity("ent_struct_conflict", "crate::app::Conflict", "struct");
        conflict.payload["typed_bindings"] = json!([
            {"name": "reader", "type": "Store", "kind": "annotation", "line": 3},
            {"name": "reader", "type": "Other", "kind": "annotation", "line": 4}
        ]);
        let conflict_caller = typed_caller(
            "ent_conflict",
            "crate::app::Conflict::start",
            json!([]),
            json!([call_site("method", "self.reader.write", "write")]),
        );

        let entities = vec![
            app,
            store,
            write,
            nested,
            missing,
            trait_impl_owner,
            conflict,
            conflict_caller,
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

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a two-hop, missing, trait-impl-owned, or ambiguous field must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo().write", "write")]),
        );
        let entities = vec![store, write.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_untyped_factory_result_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn("ent_create_repo", "crate::app::create_repo", None, None);
        let untyped = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo().write", "write")]),
        );
        let nested = typed_caller(
            "ent_nested",
            "crate::app::nested",
            json!([]),
            json!([call_site("method", "create_repo()().write", "write")]),
        );
        let member = typed_caller(
            "ent_member",
            "crate::app::member",
            json!([]),
            json!([call_site("method", "svc.create().write", "write")]),
        );
        let entities = vec![store, write, factory, untyped, nested, member];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped factory, a second hop, or an untyped member factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_constrained_factory_result() {
        let reader_trait = type_entity("ent_trait_reader", "crate::app::Reader", "trait");
        let trait_read = trait_method("ent_reader_read", "crate::app::Reader::read");
        let factory = factory_fn(
            "ent_wrap",
            "crate::app::wrap",
            Some("Reader"),
            Some("impl_trait"),
        );
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "wrap().read", "read")]),
        );
        let entities = vec![reader_trait, trait_read.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, trait_read.id);
        assert_eq!(calls[0].payload["reason"], "rust_dyn_dispatch_method");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_member_factory_result() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.create().write", "write")]),
        );
        let entities = vec![app, store, write.clone(), create, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_self_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let caller = typed_caller(
            "ent_start",
            "crate::app::App::start",
            json!([]),
            json!([call_site("method", "self.create().write", "write")]),
        );
        let entities = vec![store, write.clone(), create, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_untyped_member_factory_stays_unresolved() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let two_hop = typed_caller(
            "ent_nested",
            "crate::app::run",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.create().inner.write", "write")]),
        );
        let missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.other().write", "write")]),
        );
        let untyped_owner = typed_caller(
            "ent_untyped_owner",
            "crate::app::untyped_owner",
            json!([]),
            json!([call_site("method", "svc.create().write", "write")]),
        );
        let untyped_return = factory_fn("ent_app_build", "crate::app::App::build", None, None);
        let untyped_method = typed_caller(
            "ent_untyped_method",
            "crate::app::untyped_method",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.build().write", "write")]),
        );
        let entities = vec![
            app,
            store,
            write,
            create,
            untyped_return,
            two_hop,
            missing,
            untyped_owner,
            untyped_method,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a two-hop, missing, untyped-owner, or untyped-return member factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo().get().write", "write")]),
        );
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_member_factory_result() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let member = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.create().get().write", "write")]),
        );
        let self_caller = typed_caller(
            "ent_start",
            "crate::app::App::start",
            json!([]),
            json!([call_site("method", "self.create().get().write", "write")]),
        );
        let entities = vec![
            app,
            store,
            inner,
            write.clone(),
            create,
            get,
            member.clone(),
            self_caller.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        let member_calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == member.id)
            .collect::<Vec<_>>();
        assert_eq!(member_calls.len(), 1);
        assert_eq!(member_calls[0].to, write.id);
        assert_eq!(
            member_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
        let self_calls = relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Calls && relation.from == self_caller.id
            })
            .collect::<Vec<_>>();
        assert_eq!(self_calls.len(), 1);
        assert_eq!(self_calls[0].to, write.id);
    }

    #[tokio::test]
    async fn a_third_factory_hop_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let untyped_get = factory_fn("ent_store_extra", "crate::app::Store::extra", None, None);
        let three = typed_caller(
            "ent_three",
            "crate::app::three",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().get().extra().write",
                "write"
            )]),
        );
        let field = typed_caller(
            "ent_field",
            "crate::app::field",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().get().inner.write",
                "write"
            )]),
        );
        let missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([]),
            json!([call_site("method", "create_repo().other().write", "write")]),
        );
        let untyped = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site("method", "create_repo().extra().write", "write")]),
        );
        let entities = vec![
            store,
            inner,
            write,
            factory,
            get,
            untyped_get,
            three,
            field,
            missing,
            untyped,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a third hop, field walk, missing method, or untyped inner factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_associated_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "Store::new().write", "write")]),
        );
        let entities = vec![store, write.clone(), new, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_untyped_associated_factory_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let untyped = factory_fn("ent_store_build", "crate::app::Store::build", None, None);
        let missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([]),
            json!([call_site("method", "Store::other().write", "write")]),
        );
        let no_return = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site("method", "Store::build().write", "write")]),
        );
        let entities = vec![store, write, new, untyped, missing, no_return];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a missing or untyped associated factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_associated_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "Store::new().get().write", "write")]),
        );
        let entities = vec![store, inner, write.clone(), new, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn a_missing_nested_associated_factory_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let untyped_get = factory_fn("ent_store_extra", "crate::app::Store::extra", None, None);
        let missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([]),
            json!([call_site("method", "Store::new().other().write", "write")]),
        );
        let untyped = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site("method", "Store::new().extra().write", "write")]),
        );
        let three = typed_caller(
            "ent_three",
            "crate::app::three",
            json!([]),
            json!([call_site(
                "method",
                "Store::new().get().extra().write",
                "write"
            )]),
        );
        let entities = vec![
            store,
            inner,
            write,
            new,
            get,
            untyped_get,
            missing,
            untyped,
            three,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a missing, untyped, or third-hop nested associated factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_crate_path_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn("ent_create_repo", "crate::create_repo", Some("Store"), None);
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let ident = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "crate::create_repo().write", "write")]),
        );
        let associated = typed_caller(
            "ent_start",
            "crate::app::start",
            json!([]),
            json!([call_site(
                "method",
                "crate::app::Store::new().write",
                "write"
            )]),
        );
        let entities = vec![
            store,
            write.clone(),
            factory,
            new,
            ident.clone(),
            associated.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        let ident_calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == ident.id)
            .collect::<Vec<_>>();
        assert_eq!(ident_calls.len(), 1);
        assert_eq!(ident_calls[0].to, write.id);
        let associated_calls = relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Calls && relation.from == associated.id
            })
            .collect::<Vec<_>>();
        assert_eq!(associated_calls.len(), 1);
        assert_eq!(associated_calls[0].to, write.id);
        assert_eq!(
            associated_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_crate_path_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn("ent_create_repo", "crate::create_repo", Some("Store"), None);
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "crate::create_repo", "line": 4}]);
        let entities = vec![store, write.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn a_missing_crate_path_factory_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn("ent_create_repo", "crate::create_repo", Some("Store"), None);
        let untyped = factory_fn("ent_build", "crate::build", None, None);
        let missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([]),
            json!([call_site("method", "crate::other().write", "write")]),
        );
        let no_return = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site("method", "crate::build().write", "write")]),
        );
        let entities = vec![store, write, factory, untyped, missing, no_return];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a missing or untyped crate-path factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_crate_path_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn("ent_create_repo", "crate::create_repo", Some("Store"), None);
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let ident = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo().get().write",
                "write"
            )]),
        );
        let associated = typed_caller(
            "ent_start",
            "crate::app::start",
            json!([]),
            json!([call_site(
                "method",
                "crate::app::Store::new().get().write",
                "write"
            )]),
        );
        let entities = vec![
            store,
            inner,
            write.clone(),
            factory,
            new,
            get,
            ident.clone(),
            associated.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        let ident_calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == ident.id)
            .collect::<Vec<_>>();
        assert_eq!(ident_calls.len(), 1);
        assert_eq!(ident_calls[0].to, write.id);
        let associated_calls = relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Calls && relation.from == associated.id
            })
            .collect::<Vec<_>>();
        assert_eq!(associated_calls.len(), 1);
        assert_eq!(associated_calls[0].to, write.id);
        assert_eq!(
            associated_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_crate_path_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn("ent_create_repo", "crate::create_repo", Some("Store"), None);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "crate::create_repo().get", "line": 4}]);
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn a_missing_nested_crate_path_factory_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn("ent_create_repo", "crate::create_repo", Some("Store"), None);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let untyped_get = factory_fn("ent_store_extra", "crate::app::Store::extra", None, None);
        let missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo().other().write",
                "write"
            )]),
        );
        let untyped = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo().extra().write",
                "write"
            )]),
        );
        let three = typed_caller(
            "ent_three",
            "crate::app::three",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo().get().extra().write",
                "write"
            )]),
        );
        let relative = typed_caller(
            "ent_relative",
            "crate::app::relative",
            json!([]),
            json!([call_site(
                "method",
                "app::create_repo().get().write",
                "write"
            )]),
        );
        let entities = vec![
            store,
            inner,
            write,
            factory,
            get,
            untyped_get,
            missing,
            untyped,
            three,
            relative,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a missing, untyped, third-hop, or relative nested crate-path factory must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo", "line": 4}]);
        let entities = vec![store, write.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_associated_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "Store::new", "line": 4}]);
        let entities = vec![store, write.clone(), new, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_untyped_factory_initialized_binding_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let factory = factory_fn("ent_create_repo", "crate::app::create_repo", None, None);
        let mut untyped = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        untyped.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo", "line": 4}]);
        let mut conflict = typed_caller(
            "ent_conflict",
            "crate::app::conflict",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        conflict.payload["call_bindings"] = json!([
            {"name": "store", "callee": "create_repo", "line": 4},
            {"name": "store", "callee": "Store::new", "line": 5}
        ]);
        let entities = vec![store, write, factory, untyped, conflict];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped or ambiguous factory-initialized binding must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_member_factory_initialized_binding() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let mut member = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "store.write", "write")]),
        );
        member.payload["call_bindings"] =
            json!([{"name": "store", "callee": "svc.create", "line": 4}]);
        let mut self_caller = typed_caller(
            "ent_start",
            "crate::app::App::start",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        self_caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "self.create", "line": 4}]);
        let entities = vec![
            app,
            store,
            write.clone(),
            create,
            member.clone(),
            self_caller.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        let member_calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == member.id)
            .collect::<Vec<_>>();
        assert_eq!(member_calls.len(), 1);
        assert_eq!(member_calls[0].to, write.id);
        assert_eq!(
            member_calls[0].payload["reason"],
            "rust_typed_receiver_method"
        );
        let self_calls = relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Calls && relation.from == self_caller.id
            })
            .collect::<Vec<_>>();
        assert_eq!(self_calls.len(), 1);
        assert_eq!(self_calls[0].to, write.id);
    }

    #[tokio::test]
    async fn an_untyped_member_factory_initialized_binding_stays_unresolved() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let mut untyped_owner = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        untyped_owner.payload["call_bindings"] =
            json!([{"name": "store", "callee": "svc.create", "line": 4}]);
        let mut missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "store.write", "write")]),
        );
        missing.payload["call_bindings"] =
            json!([{"name": "store", "callee": "svc.other", "line": 4}]);
        let entities = vec![app, store, write, create, untyped_owner, missing];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped owner or missing member factory init must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo().get", "line": 4}]);
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_associated_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "Store::new().get", "line": 4}]);
        let entities = vec![store, inner, write.clone(), new, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_nested_member_factory_initialized_binding() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let mut member = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "store.write", "write")]),
        );
        member.payload["call_bindings"] =
            json!([{"name": "store", "callee": "svc.create().get", "line": 4}]);
        let mut self_caller = typed_caller(
            "ent_start",
            "crate::app::App::start",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        self_caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "self.create().get", "line": 4}]);
        let entities = vec![
            app,
            store,
            inner,
            write.clone(),
            create,
            get,
            member.clone(),
            self_caller.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        let member_calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == member.id)
            .collect::<Vec<_>>();
        assert_eq!(member_calls.len(), 1);
        assert_eq!(member_calls[0].to, write.id);
        let self_calls = relations
            .iter()
            .filter(|relation| {
                relation.kind == RelationKind::Calls && relation.from == self_caller.id
            })
            .collect::<Vec<_>>();
        assert_eq!(self_calls.len(), 1);
        assert_eq!(self_calls[0].to, write.id);
    }

    #[tokio::test]
    async fn a_missing_nested_factory_initialized_binding_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let untyped_get = factory_fn("ent_store_extra", "crate::app::Store::extra", None, None);
        let mut missing = typed_caller(
            "ent_missing",
            "crate::app::missing",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        missing.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo().other", "line": 4}]);
        let mut untyped = typed_caller(
            "ent_untyped",
            "crate::app::untyped",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        untyped.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo().extra", "line": 4}]);
        let mut third = typed_caller(
            "ent_third",
            "crate::app::third",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        third.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo().get().extra", "line": 4}]);
        let entities = vec![
            store,
            inner,
            write,
            factory,
            get,
            untyped_get,
            missing,
            untyped,
            third,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a missing, untyped, or third-hop factory init must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_try_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo()?.write", "write")]),
        );
        let entities = vec![store, write.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_try_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo", "line": 4, "try": true}]);
        let entities = vec![store, write.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn resolves_a_method_on_a_try_nested_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let mut get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        get.payload["try_yield"] = json!(true);
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo().get()?.write", "write")]),
        );
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_inner_try_factory_hop() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let mut crate_factory = factory_fn(
            "ent_crate_create_repo",
            "crate::create_repo",
            Some("Store"),
            None,
        );
        crate_factory.payload["try_yield"] = json!(true);
        let mut create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        create.payload["try_yield"] = json!(true);
        let mut new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        new.payload["try_yield"] = json!(true);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let ident = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo()?.get().write", "write")]),
        );
        let member = typed_caller(
            "ent_member",
            "crate::app::member",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.create()?.get().write", "write")]),
        );
        let self_caller = typed_caller(
            "ent_self",
            "crate::app::App::start",
            json!([]),
            json!([call_site("method", "self.create()?.get().write", "write")]),
        );
        let associated = typed_caller(
            "ent_associated",
            "crate::app::associated",
            json!([]),
            json!([call_site("method", "Store::new()?.get().write", "write")]),
        );
        let path = typed_caller(
            "ent_path",
            "crate::app::path",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo()?.get().write",
                "write"
            )]),
        );
        let entities = vec![
            app,
            store,
            inner,
            write.clone(),
            factory,
            crate_factory,
            create,
            new,
            get,
            ident.clone(),
            member.clone(),
            self_caller.clone(),
            associated.clone(),
            path.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        for caller in [&ident, &member, &self_caller, &associated, &path] {
            let calls = relations
                .iter()
                .filter(|relation| {
                    relation.kind == RelationKind::Calls && relation.from == caller.id
                })
                .collect::<Vec<_>>();
            assert_eq!(
                calls.len(),
                1,
                "inner-try hop must resolve for {}",
                caller.name
            );
            assert_eq!(calls[0].to, write.id);
            assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
            assert_eq!(calls[0].confidence, 0.8);
        }
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_inner_try_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo()?.get", "line": 4}]);
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_unwrap_factory_result() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let mut crate_factory = factory_fn(
            "ent_crate_create_repo",
            "crate::create_repo",
            Some("Store"),
            None,
        );
        crate_factory.payload["try_yield"] = json!(true);
        let mut create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        create.payload["try_yield"] = json!(true);
        let mut new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        new.payload["try_yield"] = json!(true);
        let ident = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "create_repo().unwrap().write", "write")]),
        );
        let expect = typed_caller(
            "ent_expect",
            "crate::app::expect_run",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().expect(\"failed\").write",
                "write"
            )]),
        );
        let member = typed_caller(
            "ent_member",
            "crate::app::member",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "svc.create().unwrap().write", "write")]),
        );
        let associated = typed_caller(
            "ent_associated",
            "crate::app::associated",
            json!([]),
            json!([call_site("method", "Store::new().unwrap().write", "write")]),
        );
        let path = typed_caller(
            "ent_path",
            "crate::app::path",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo().unwrap().write",
                "write"
            )]),
        );
        let entities = vec![
            app,
            store,
            write.clone(),
            factory,
            crate_factory,
            create,
            new,
            ident.clone(),
            expect.clone(),
            member.clone(),
            associated.clone(),
            path.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        for caller in [&ident, &expect, &member, &associated, &path] {
            let calls = relations
                .iter()
                .filter(|relation| {
                    relation.kind == RelationKind::Calls && relation.from == caller.id
                })
                .collect::<Vec<_>>();
            assert_eq!(
                calls.len(),
                1,
                "unwrap factory hop must resolve for {}",
                caller.name
            );
            assert_eq!(calls[0].to, write.id);
            assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
            assert_eq!(calls[0].confidence, 0.8);
        }
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_unwrap_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo", "line": 4, "try": true}]);
        let entities = vec![store, write.clone(), factory, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_unwrap_nested_factory_result() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        let mut get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        get.payload["try_yield"] = json!(true);
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().get().unwrap().write",
                "write"
            )]),
        );
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_inner_unwrap_factory_hop() {
        let app = type_entity("ent_struct_app", "crate::app::App", "struct");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let mut crate_factory = factory_fn(
            "ent_crate_create_repo",
            "crate::create_repo",
            Some("Store"),
            None,
        );
        crate_factory.payload["try_yield"] = json!(true);
        let mut create = factory_fn(
            "ent_app_create",
            "crate::app::App::create",
            Some("Store"),
            None,
        );
        create.payload["try_yield"] = json!(true);
        let mut new = factory_fn(
            "ent_store_new",
            "crate::app::Store::new",
            Some("Store"),
            None,
        );
        new.payload["try_yield"] = json!(true);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let ident = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().unwrap().get().write",
                "write"
            )]),
        );
        let expect = typed_caller(
            "ent_expect",
            "crate::app::expect_run",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().expect(\"failed\").get().write",
                "write"
            )]),
        );
        let member = typed_caller(
            "ent_member",
            "crate::app::member",
            json!([{"name": "svc", "type": "App", "kind": "annotation", "line": 3}]),
            json!([call_site(
                "method",
                "svc.create().unwrap().get().write",
                "write"
            )]),
        );
        let associated = typed_caller(
            "ent_associated",
            "crate::app::associated",
            json!([]),
            json!([call_site(
                "method",
                "Store::new().unwrap().get().write",
                "write"
            )]),
        );
        let path = typed_caller(
            "ent_path",
            "crate::app::path",
            json!([]),
            json!([call_site(
                "method",
                "crate::create_repo().unwrap().get().write",
                "write"
            )]),
        );
        let entities = vec![
            app,
            store,
            inner,
            write.clone(),
            factory,
            crate_factory,
            create,
            new,
            get,
            ident.clone(),
            expect.clone(),
            member.clone(),
            associated.clone(),
            path.clone(),
        ];
        let relations = link_all(entities, Vec::new()).await;

        for caller in [&ident, &expect, &member, &associated, &path] {
            let calls = relations
                .iter()
                .filter(|relation| {
                    relation.kind == RelationKind::Calls && relation.from == caller.id
                })
                .collect::<Vec<_>>();
            assert_eq!(
                calls.len(),
                1,
                "inner unwrap hop must resolve for {}",
                caller.name
            );
            assert_eq!(calls[0].to, write.id);
            assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
            assert_eq!(calls[0].confidence, 0.8);
        }
    }

    #[tokio::test]
    async fn resolves_a_method_on_an_inner_unwrap_factory_initialized_binding() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_inner_write", "crate::app::Inner::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let mut caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        caller.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo()?.get", "line": 4}]);
        let entities = vec![store, inner, write.clone(), factory, get, caller.clone()];
        let relations = link_all(entities, Vec::new()).await;

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls && relation.from == caller.id)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_unwrap_without_try_yield_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let inner_write = method("ent_inner_write", "crate::app::Inner::write");
        let plain = factory_fn("ent_build", "crate::app::build", Some("Store"), None);
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let on_plain = typed_caller(
            "ent_on_plain",
            "crate::app::on_plain",
            json!([]),
            json!([call_site("method", "build().unwrap().write", "write")]),
        );
        let or_default = typed_caller(
            "ent_or_default",
            "crate::app::or_default",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().unwrap_or_default().write",
                "write"
            )]),
        );
        let inner_plain = typed_caller(
            "ent_inner_plain",
            "crate::app::inner_plain",
            json!([]),
            json!([call_site("method", "build().unwrap().get().write", "write")]),
        );
        let inner_third = typed_caller(
            "ent_inner_third",
            "crate::app::inner_third",
            json!([]),
            json!([call_site(
                "method",
                "create_repo().unwrap().get().extra().write",
                "write"
            )]),
        );
        let field = typed_caller(
            "ent_field",
            "crate::app::field",
            json!([{"name": "store", "type": "Store", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "store.unwrap().write", "write")]),
        );
        let entities = vec![
            store,
            inner,
            write,
            inner_write,
            plain,
            factory,
            get,
            on_plain,
            or_default,
            inner_plain,
            inner_third,
            field,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "unwrap on a non-Result factory, unwrap_or_default, a third inner-unwrap hop, or store.unwrap() must not resolve"
        );
    }

    #[tokio::test]
    async fn a_try_factory_without_question_stays_unresolved() {
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let inner = type_entity("ent_struct_inner", "crate::app::Inner", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let inner_write = method("ent_inner_write", "crate::app::Inner::write");
        let mut factory = factory_fn(
            "ent_create_repo",
            "crate::app::create_repo",
            Some("Store"),
            None,
        );
        factory.payload["try_yield"] = json!(true);
        let get = factory_fn(
            "ent_store_get",
            "crate::app::Store::get",
            Some("Inner"),
            None,
        );
        let no_try = typed_caller(
            "ent_no_try",
            "crate::app::no_try",
            json!([]),
            json!([call_site("method", "create_repo().write", "write")]),
        );
        let mut binding = typed_caller(
            "ent_binding",
            "crate::app::binding",
            json!([]),
            json!([call_site("method", "store.write", "write")]),
        );
        binding.payload["call_bindings"] =
            json!([{"name": "store", "callee": "create_repo", "line": 4}]);
        let plain = factory_fn("ent_build", "crate::app::build", Some("Store"), None);
        let question = typed_caller(
            "ent_question",
            "crate::app::question",
            json!([]),
            json!([call_site("method", "build()?.write", "write")]),
        );
        let nested_no_try = typed_caller(
            "ent_nested_no_try",
            "crate::app::nested_no_try",
            json!([]),
            json!([call_site("method", "create_repo().get().write", "write")]),
        );
        let inner_question = typed_caller(
            "ent_inner_question",
            "crate::app::inner_question",
            json!([]),
            json!([call_site("method", "build()?.get().write", "write")]),
        );
        let inner_third = typed_caller(
            "ent_inner_third",
            "crate::app::inner_third",
            json!([]),
            json!([call_site(
                "method",
                "create_repo()?.get().extra().write",
                "write"
            )]),
        );
        let entities = vec![
            store,
            inner,
            write,
            inner_write,
            factory,
            get,
            no_try,
            binding,
            plain,
            question,
            nested_no_try,
            inner_question,
            inner_third,
        ];
        let relations = link_all(entities, Vec::new()).await;

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a Result factory without ?, a ? on a non-Result factory, or a third inner-try hop must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_a_member_through_a_type_alias() {
        let mut client = type_entity("ent_alias_client", "crate::app::Client", "type_alias");
        client.payload["aliased_type"] = json!("Store");
        let store = type_entity("ent_struct_store", "crate::app::Store", "struct");
        let write = method("ent_store_write", "crate::app::Store::write");
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "item", "type": "Client", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "item.write", "write")]),
        );

        let entities = vec![client, store, write.clone(), caller.clone()];
        let relations = RustLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: entities.clone().into(),
                facts: Vec::new().into(),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, caller.id);
        assert_eq!(calls[0].to, write.id);
        assert_eq!(calls[0].payload["reason"], "rust_typed_receiver_method");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn a_type_alias_without_a_named_type_does_not_resolve() {
        let client = type_entity("ent_alias_client", "crate::app::Client", "type_alias");
        let write = method("ent_store_write", "crate::app::Store::write");
        let caller = typed_caller(
            "ent_run",
            "crate::app::run",
            json!([{"name": "item", "type": "Client", "kind": "annotation", "line": 3}]),
            json!([call_site("method", "item.write", "write")]),
        );

        let entities = vec![client, write, caller];
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
            "an unnamed alias must not unfold to a unique member"
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
