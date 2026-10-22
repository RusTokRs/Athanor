//! Call-graph linking for JavaScript and TypeScript declarations.
//!
//! The JS/TS extractor records statically named call sites per declaration.
//! This linker resolves those call sites to canonical declarations inside the
//! same snapshot and emits `Calls` relations, plus `TestedBy` relations when a
//! test-file declaration calls one from production code.
//!
//! Every relation carries the matched callee, its resolution reason, and the
//! call-site line, so a query can always be traced back to the source. Dynamic
//! call shapes (computed members, calls of call results, reflective dispatch) stay
//! unresolved instead of being guessed, and name-based fallbacks require a
//! unique candidate so ambiguity never silently becomes a fact.
//!
//! Call sites recorded by known callback combinators (`map`, `then`,
//! `setTimeout`, and similar) additionally bridge their statically named
//! arguments: an indirect invocation through a recorded callback argument
//! resolves with the same ordered rules and emits its own `Calls` relation
//! with reason `js_ts_callback_bridge`, keeping direct and indirect paths
//! distinct. Unresolvable or ambiguous callback arguments stay unresolved.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, InvalidationPolicy, InvalidationScope, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityKind, Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use crate::config::{
    ModuleResolutionConfig, build_module_resolution_config, resolve_module_specifier,
};
use crate::paths::normalize_path;

const MODULE_PREFIX: &str = "module://js-ts:";
const SYMBOL_PREFIX: &str = "symbol://js-ts:";

/// Upper bound on emitted call relations per snapshot.
///
/// Relations are sorted deterministically before the cap is applied, so the
/// same snapshot always yields the same bounded result.
const MAX_CALL_RELATIONS: usize = 10_000;

/// Directory names that mark a JavaScript/TypeScript test location.
const TEST_DIRECTORIES: &[&str] = &["tests", "test", "__tests__", "e2e"];

/// Confidence assigned to a call resolved to an exact same-module declaration.
const SAME_MODULE_EXACT_CONFIDENCE: f32 = 0.9;
/// Confidence assigned to a `this.method()` call resolved inside its class.
const SAME_MODULE_THIS_CONFIDENCE: f32 = 0.8;
/// Confidence assigned to a call resolved through an import binding.
const IMPORTED_EXACT_CONFIDENCE: f32 = 0.85;
/// Confidence assigned to a call resolved to a named symbol of an imported module.
const IMPORTED_NAME_CONFIDENCE: f32 = 0.7;
/// Confidence assigned to a unique name-only match (member or exported symbol).
const UNIQUE_NAME_CONFIDENCE: f32 = 0.5;
/// Confidence of a member the receiver's declared type inherits: the type
/// states the base and the base states the member, but the call site does not
/// name the class that owns the body.
const INHERITED_MEMBER_CONFIDENCE: f32 = 0.6;
/// Confidence assigned to a call resolved through a receiver a `new Type()` initializer typed.
const TYPED_CONSTRUCTION_CONFIDENCE: f32 = 0.8;
/// Confidence assigned to a call resolved through a receiver a type annotation typed.
const TYPED_ANNOTATION_CONFIDENCE: f32 = 0.7;
/// Confidence of a call resolved through a receiver whose type the source states
/// one derivation away: the element of a collection whose annotation the file
/// writes, or the value an `await` yields from a declared promise. The
/// annotation is written in the source, but not on the receiver's own binding,
/// so the tier is weaker than an annotation the binding carries.
const DERIVED_TYPE_CONFIDENCE: f32 = 0.65;
/// Upper bound on receiver segments walked for one type-aware dispatch.
const MAX_TYPE_PATH_SEGMENTS: usize = 4;
/// The most `extends` hops one inherited member lookup walks.
const MAX_HERITAGE_DEPTH: usize = 4;
/// Maximum number of named, default, wildcard, or namespace export hops per call resolution.
const MAX_REEXPORT_DEPTH: usize = 32;
/// Maximum resolver visits across all branches of one call's export resolution.
const MAX_REEXPORT_RESOLUTION_STEPS: usize = 1_024;

#[derive(Debug, Default, Clone, Copy)]
pub struct JsTsCallLinker;

#[async_trait]
impl Linker for JsTsCallLinker {
    fn name(&self) -> &str {
        "js-ts-calls"
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

        let mut modules = HashMap::new();
        let mut declarations = Vec::new();
        let mut type_aliases = Vec::new();
        for entity in input.entities.iter() {
            if is_js_ts_module(entity) {
                if let Some(path) = entity_path(entity) {
                    modules.insert(path, entity);
                }
            } else if is_js_ts_declaration(entity) {
                declarations.push(entity);
            } else if is_js_ts_type_alias(entity) {
                type_aliases.push(entity);
            }
        }

        let mut declarations_by_path = HashMap::<String, Vec<&Entity>>::new();
        let mut declarations_by_name = HashMap::<String, Vec<&Entity>>::new();
        for declaration in &declarations {
            if let Some(path) = entity_path(declaration) {
                declarations_by_path
                    .entry(path)
                    .or_default()
                    .push(declaration);
            }
            declarations_by_name
                .entry(declaration.name.clone())
                .or_default()
                .push(declaration);
        }
        for alias in &type_aliases {
            declarations_by_name
                .entry(alias.name.clone())
                .or_default()
                .push(alias);
        }
        for candidates in declarations_by_path.values_mut() {
            candidates.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        }
        for candidates in declarations_by_name.values_mut() {
            candidates.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        }

        let type_modules = build_type_module_index(input.entities.as_slice(), &modules);
        let config = build_module_resolution_config(input.entities.as_slice());
        let mut relations = Vec::new();
        let mut relation_ids = HashSet::new();

        for caller in &declarations {
            let Some(caller_path) = entity_path(caller) else {
                continue;
            };
            let Some(caller_module) = modules.get(&caller_path).copied() else {
                continue;
            };
            let Some(calls) = caller.payload.get("calls").and_then(Value::as_array) else {
                continue;
            };
            for call in calls {
                let Some(callee) = call.get("callee").and_then(Value::as_str) else {
                    continue;
                };
                let line = call.get("line").and_then(Value::as_u64);
                let column = call.get("column").and_then(Value::as_u64);
                let call_kind = call.get("kind").and_then(Value::as_str).unwrap_or("call");

                if let Some((target, resolution, confidence)) = resolve_call(
                    callee,
                    caller,
                    caller_module,
                    &modules,
                    &declarations_by_path,
                    &declarations_by_name,
                    &type_modules,
                    &config,
                    true,
                ) {
                    let affects_subset =
                        affected_ids.contains(&caller.id) || affected_ids.contains(&target.id);
                    if target.id != caller.id && affects_subset {
                        push_unique(
                            &mut relations,
                            &mut relation_ids,
                            call_relation(
                                &input.snapshot,
                                caller,
                                target,
                                callee,
                                call_kind,
                                resolution,
                                confidence,
                                line,
                                column,
                            ),
                        );
                        if is_test_file(&caller_path)
                            && let Some(target_path) = entity_path(target)
                            && !is_test_file(&target_path)
                        {
                            push_unique(
                                &mut relations,
                                &mut relation_ids,
                                tested_by_relation(&input.snapshot, target, caller, callee, line),
                            );
                        }
                    }
                }

                // Callback bridges: a combinator call such as `list.map(fn)`
                // invokes its argument indirectly, so each statically named
                // callback argument gets its own evidence-backed `Calls` edge.
                // Resolution reuses the direct-call order and its fail-closed
                // rules; an unresolvable callback stays unresolved.
                if let Some(callbacks) = call.get("callback_arguments").and_then(Value::as_array) {
                    for callback in callbacks {
                        let Some(callback_name) = callback.as_str() else {
                            continue;
                        };
                        let Some((target, resolution, confidence)) = resolve_call(
                            callback_name,
                            caller,
                            caller_module,
                            &modules,
                            &declarations_by_path,
                            &declarations_by_name,
                            &type_modules,
                            &config,
                            true,
                        ) else {
                            continue;
                        };
                        if target.id == caller.id {
                            continue;
                        }
                        if !affected_ids.contains(&caller.id) && !affected_ids.contains(&target.id)
                        {
                            continue;
                        }
                        push_unique(
                            &mut relations,
                            &mut relation_ids,
                            callback_relation(
                                &input.snapshot,
                                caller,
                                target,
                                callback_name,
                                callee,
                                resolution,
                                confidence,
                                line,
                                column,
                            ),
                        );
                        if is_test_file(&caller_path)
                            && let Some(target_path) = entity_path(target)
                            && !is_test_file(&target_path)
                        {
                            push_unique(
                                &mut relations,
                                &mut relation_ids,
                                tested_by_relation(
                                    &input.snapshot,
                                    target,
                                    caller,
                                    callback_name,
                                    line,
                                ),
                            );
                        }
                    }
                }
            }
        }

        relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        relations.dedup_by(|left, right| left.id == right.id);
        relations.truncate(MAX_CALL_RELATIONS);
        Ok(relations)
    }
}

fn is_js_ts_module(entity: &Entity) -> bool {
    entity.kind == EntityKind::Module && entity.stable_key.0.starts_with(MODULE_PREFIX)
}

/// Identifies a resolvable JS/TS declaration. Classes are included so
/// `new Foo()` construction sites and default-imported classes resolve
/// to their class declaration, alongside functions and methods.
fn is_js_ts_declaration(entity: &Entity) -> bool {
    matches!(entity.kind, EntityKind::Function | EntityKind::Class)
        && entity.stable_key.0.starts_with(SYMBOL_PREFIX)
}

fn is_js_ts_type_alias(entity: &Entity) -> bool {
    entity.kind == EntityKind::Symbol
        && entity.stable_key.0.starts_with(SYMBOL_PREFIX)
        && entity
            .payload
            .get("symbol_kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "type_alias")
}

fn entity_path(entity: &Entity) -> Option<String> {
    entity
        .source
        .as_ref()
        .map(|source| normalize_path(&source.path))
}

fn qualified_name(entity: &Entity) -> Option<&str> {
    entity.payload.get("qualified_name").and_then(Value::as_str)
}

fn is_exported(entity: &Entity) -> bool {
    entity
        .payload
        .get("exported")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Resolves one statically named callee to a canonical declaration.
///
/// Resolution is ordered from the most precise evidence to the least: the
/// calling declaration itself, its module, its import bindings, and finally a
/// unique name match. Later steps only fire when the earlier ones are silent,
/// and every name-based step requires exactly one candidate.
#[allow(clippy::too_many_arguments)]
fn resolve_call<'a>(
    callee: &str,
    caller: &'a Entity,
    caller_module: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
    allow_typed: bool,
) -> Option<(&'a Entity, &'static str, f32)> {
    let caller_path = entity_path(caller)?;
    let same_module = declarations_by_path.get(&caller_path);

    // 1. `this.method()` inside `Class.method` resolves to the sibling member.
    if let Some(method) = callee.strip_prefix("this.")
        && let Some(owner) = method_owner(caller)
    {
        let qualified = format!("{owner}.{method}");
        if let Some(target) = find_first(same_module, |candidate| {
            qualified_name(candidate) == Some(qualified.as_str())
        }) {
            return Some((
                target,
                "same_module_this_method",
                SAME_MODULE_THIS_CONFIDENCE,
            ));
        }
    }

    // 2. Exact qualified name inside the calling module.
    if let Some(target) = find_first(same_module, |candidate| {
        qualified_name(candidate) == Some(callee)
    }) {
        return Some((
            target,
            "same_module_exact_name",
            SAME_MODULE_EXACT_CONFIDENCE,
        ));
    }

    // 3. Import bindings: `import { login }`, `import api from`, `import * as ns`.
    // An unresolved explicit binding stays unresolved; it must not fall through
    // to a weaker snapshot-wide name guess. A synthetic call-result path
    // (`svc.build().save`, `createRepo().save`) is typed evidence, not an
    // imported member named `save`, so it skips this step.
    if !is_synthetic_call_path(callee)
        && let Some((head, tail)) = head_and_tail(callee)
        && let Some(binding) = import_binding(caller_module, head)
    {
        let (target_module, _) =
            resolve_module_specifier(caller_module, &binding.specifier, modules, config)?;
        if binding.named_import {
            let has_nested_member = callee
                .strip_prefix(head)
                .and_then(|remainder| remainder.strip_prefix('.'))
                .is_some_and(|remainder| remainder.contains('.'));
            if has_nested_member {
                return None;
            }
        }

        let mut visited = HashSet::new();
        let mut remaining_resolution_steps = MAX_REEXPORT_RESOLUTION_STEPS;
        let namespace_resolution = if binding.named_import && tail.is_some() {
            let (Some(namespace), Some(member)) = (binding.imported_name.as_deref(), tail) else {
                return None;
            };
            match resolve_namespace_exported_member(
                target_module,
                namespace,
                member,
                modules,
                declarations_by_path,
                &mut visited,
                &mut remaining_resolution_steps,
                0,
                config,
            ) {
                Some(Some((target, depth))) => Some((target, depth, member)),
                Some(None) => return None,
                None => {
                    if !has_direct_imported_member(
                        target_module,
                        namespace,
                        member,
                        declarations_by_path,
                    ) {
                        return None;
                    }
                    None
                }
            }
        } else {
            None
        };
        let (target, reexport_depth, wanted) =
            if let Some(namespace_resolution) = namespace_resolution {
                namespace_resolution
            } else {
                let wanted = tail.or(binding.imported_name.as_deref()).unwrap_or(head);
                let (target, depth) = resolve_exported_name(
                    target_module,
                    wanted,
                    modules,
                    declarations_by_path,
                    &mut visited,
                    &mut remaining_resolution_steps,
                    0,
                    config,
                )
                .into_resolved()?;
                (target, depth, wanted)
            };
        if reexport_depth > 0 {
            return Some((target, "imported_reexport_chain", IMPORTED_EXACT_CONFIDENCE));
        }
        if wanted == "default" {
            return Some((target, "imported_default_export", IMPORTED_EXACT_CONFIDENCE));
        }
        if qualified_name(target) == Some(wanted) {
            return Some((target, "imported_exact_name", IMPORTED_EXACT_CONFIDENCE));
        }
        return Some((target, "imported_symbol_name", IMPORTED_NAME_CONFIDENCE));
    }

    // 4. Type-aware receiver dispatch: `service.run()`, `this.repo.items.map()`,
    //    `super.save()`, `createRepo().save()`, and `(value as Repo).save()`
    //    resolve when every receiver segment names a binding whose type the
    //    extractor proved, or a factory, construction, assertion, or `super`
    //    the callee path itself names. A receiver that is only named, not typed,
    //    falls through to the weaker member-name guess below.
    if allow_typed
        && callee.contains('.')
        && let Some((target, kind, segments, inherited)) = resolve_typed_member(
            callee,
            caller,
            caller_module,
            declarations_by_name,
            type_modules,
            modules,
            declarations_by_path,
            config,
        )
    {
        let resolution = if inherited {
            if segments > 1 {
                "inherited_typed_receiver_path"
            } else {
                "inherited_typed_member"
            }
        } else if segments > 1 {
            "typed_receiver_path"
        } else {
            "typed_receiver_member"
        };
        let confidence = if inherited {
            INHERITED_MEMBER_CONFIDENCE
        } else if matches!(kind, "construction" | "returned") {
            TYPED_CONSTRUCTION_CONFIDENCE
        } else if matches!(kind, "element" | "awaited") {
            DERIVED_TYPE_CONFIDENCE
        } else {
            TYPED_ANNOTATION_CONFIDENCE
        };
        return Some((target, resolution, confidence));
    }

    // An index-access receiver (`items[].save`) is only evidence when the
    // collection's annotation types the element. A factory- or construction-
    // result receiver (`createRepo().save`, `Repo{}.save`) is only evidence
    // when that factory's type is stated. A unique member named `save` is
    // not that evidence, so the walk ends here instead of guessing.
    if is_synthetic_call_path(callee) || callee.starts_with("super.") {
        return None;
    }

    let name = callee.rsplit('.').next().unwrap_or(callee);
    if callee.contains('.') {
        // 5. Member call with an unknown receiver: accept a unique member name
        //    inside the calling module (`session.refresh()` -> `Session.refresh`).
        let member_suffix = format!(".{name}");
        if let Some(target) = unique_match(same_module, |candidate| {
            candidate.name == name
                && qualified_name(candidate).is_some_and(|qualified| {
                    qualified == name || qualified.ends_with(&member_suffix)
                })
        }) {
            return Some((target, "same_module_member_name", UNIQUE_NAME_CONFIDENCE));
        }
    } else {
        // 6. Bare call without a local or imported target: accept a unique
        //    exported declaration elsewhere in the snapshot.
        if let Some(target) = unique_match(declarations_by_name.get(name), |candidate| {
            candidate.id != caller.id && is_exported(candidate) && is_js_ts_declaration(candidate)
        }) {
            return Some((target, "unique_exported_name", UNIQUE_NAME_CONFIDENCE));
        }
    }

    None
}

/// A callee whose receiver is an index, factory result, construction, or
/// inline assertion.
///
/// These paths are only evidence when the typed walk names a type; a unique
/// member `save` is not that evidence.
fn is_synthetic_call_path(callee: &str) -> bool {
    callee.contains("[]")
        || callee.contains("()")
        || callee.contains("{}")
        || callee
            .split('.')
            .any(|segment| segment.len() > 1 && segment.ends_with('#') && !segment.starts_with('#'))
}

fn method_owner(entity: &Entity) -> Option<&str> {
    let qualified = qualified_name(entity)?;
    let (owner, _) = qualified.split_once('.')?;
    Some(owner)
}

fn head_and_tail(callee: &str) -> Option<(&str, Option<&str>)> {
    match callee.split_once('.') {
        Some((head, rest)) => Some((head, Some(rest.rsplit('.').next()?))),
        None => Some((callee, None)),
    }
}

struct CallImportBinding {
    specifier: String,
    imported_name: Option<String>,
    named_import: bool,
}

/// Returns the source specifier and imported name for one local import binding.
fn import_binding(module: &Entity, local: &str) -> Option<CallImportBinding> {
    let imports = module.payload.get("imports").and_then(Value::as_array)?;
    for import in imports {
        let Some(specifier) = import.get("source").and_then(Value::as_str) else {
            continue;
        };
        if import.get("default_name").and_then(Value::as_str) == Some(local) {
            return Some(CallImportBinding {
                specifier: specifier.to_string(),
                imported_name: Some("default".to_string()),
                named_import: false,
            });
        }
        if import.get("namespace_name").and_then(Value::as_str) == Some(local) {
            return Some(CallImportBinding {
                specifier: specifier.to_string(),
                imported_name: None,
                named_import: false,
            });
        }
        if let Some(binding) = import
            .get("named_bindings")
            .and_then(Value::as_array)
            .and_then(|bindings| {
                bindings
                    .iter()
                    .find(|binding| binding.get("local").and_then(Value::as_str) == Some(local))
            })
        {
            return Some(CallImportBinding {
                specifier: specifier.to_string(),
                imported_name: binding
                    .get("imported")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                named_import: true,
            });
        }
        if import
            .get("named")
            .and_then(Value::as_array)
            .is_some_and(|named| named.iter().any(|entry| entry.as_str() == Some(local)))
        {
            // Retain compatibility with normalized payloads produced before
            // named source/local pairs were recorded.
            return Some(CallImportBinding {
                specifier: specifier.to_string(),
                imported_name: Some(local.to_string()),
                named_import: true,
            });
        }
    }
    None
}

enum ExportedNameResolution<'a> {
    NotFound,
    Resolved(&'a Entity, usize),
    Ambiguous,
    Blocked,
}

impl<'a> ExportedNameResolution<'a> {
    fn into_resolved(self) -> Option<(&'a Entity, usize)> {
        match self {
            Self::Resolved(target, depth) => Some((target, depth)),
            Self::NotFound | Self::Ambiguous | Self::Blocked => None,
        }
    }
}

/// Resolves a named export through explicit local aliases, relative named
/// re-exports, source-backed `export *` clauses, or a named default export.
/// Missing names are skipped while aggregating wildcard sources, but ambiguous
/// mappings, unresolved sources, cycles, chains deeper than `MAX_REEXPORT_DEPTH`,
/// or more than `MAX_REEXPORT_RESOLUTION_STEPS` resolver visits block resolution.
#[allow(clippy::too_many_arguments)]
fn resolve_exported_name<'a>(
    module: &'a Entity,
    requested: &str,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    visited: &mut HashSet<String>,
    remaining_steps: &mut usize,
    depth: usize,
    config: &ModuleResolutionConfig,
) -> ExportedNameResolution<'a> {
    if depth > MAX_REEXPORT_DEPTH || *remaining_steps == 0 {
        return ExportedNameResolution::Blocked;
    }
    *remaining_steps -= 1;
    let Some(path) = entity_path(module) else {
        return ExportedNameResolution::Blocked;
    };
    if !visited.insert(path.clone()) {
        return ExportedNameResolution::Blocked;
    }

    let direct_matches = declarations_by_path
        .get(&path)
        .into_iter()
        .flat_map(|candidates| candidates.iter().copied())
        .filter(|candidate| {
            is_exported(candidate)
                && (qualified_name(candidate) == Some(requested) || candidate.name == requested)
        })
        .collect::<Vec<_>>();
    if !direct_matches.is_empty() {
        let result = unique_target(direct_matches.into_iter().map(|candidate| (candidate, 0)))
            .map_or(ExportedNameResolution::Ambiguous, |(target, depth)| {
                ExportedNameResolution::Resolved(target, depth)
            });
        visited.remove(&path);
        return result;
    }

    let mut has_matching_mapping = false;
    let mut unresolved_mapping = false;
    let mut mapped_targets = Vec::new();
    let mut star_exports = Vec::new();
    let mut default_exports = Vec::new();
    if let Some(exports) = module.payload.get("exports").and_then(Value::as_array) {
        for export in exports {
            if export.get("default_export").and_then(Value::as_bool) == Some(true) {
                default_exports.push(export);
            }
            if export.get("star").and_then(Value::as_bool) == Some(true) {
                star_exports.push(export);
                continue;
            }
            let source = export.get("source").and_then(Value::as_str);
            let Some(bindings) = export.get("bindings").and_then(Value::as_array) else {
                continue;
            };
            for binding in bindings {
                if binding.get("exported").and_then(Value::as_str) != Some(requested) {
                    continue;
                }
                has_matching_mapping = true;
                let Some(imported) = binding.get("imported").and_then(Value::as_str) else {
                    unresolved_mapping = true;
                    continue;
                };
                if let Some(specifier) = source {
                    let Some((target_module, _)) =
                        resolve_module_specifier(module, specifier, modules, config)
                    else {
                        unresolved_mapping = true;
                        continue;
                    };
                    match resolve_exported_name(
                        target_module,
                        imported,
                        modules,
                        declarations_by_path,
                        visited,
                        remaining_steps,
                        depth + 1,
                        config,
                    ) {
                        ExportedNameResolution::Resolved(target, nested_depth) => {
                            mapped_targets.push((target, nested_depth + 1));
                        }
                        ExportedNameResolution::NotFound
                        | ExportedNameResolution::Ambiguous
                        | ExportedNameResolution::Blocked => unresolved_mapping = true,
                    }
                } else {
                    let local_matches = declarations_by_path
                        .get(&path)
                        .into_iter()
                        .flat_map(|candidates| candidates.iter().copied())
                        .filter(|candidate| {
                            qualified_name(candidate) == Some(imported)
                                || candidate.name == imported
                        })
                        .collect::<Vec<_>>();
                    if !local_matches.is_empty() {
                        if let Some(target) =
                            unique_target(local_matches.into_iter().map(|candidate| (candidate, 1)))
                        {
                            mapped_targets.push(target);
                        } else {
                            unresolved_mapping = true;
                        }
                    } else if let Some(import_binding) = import_binding(module, imported) {
                        if let Some((imported_module, _)) = resolve_module_specifier(
                            module,
                            &import_binding.specifier,
                            modules,
                            config,
                        ) {
                            let imported_name =
                                import_binding.imported_name.as_deref().unwrap_or(imported);
                            match resolve_exported_name(
                                imported_module,
                                imported_name,
                                modules,
                                declarations_by_path,
                                visited,
                                remaining_steps,
                                depth + 1,
                                config,
                            ) {
                                ExportedNameResolution::Resolved(target, nested_depth) => {
                                    mapped_targets.push((target, nested_depth + 1));
                                }
                                ExportedNameResolution::NotFound
                                | ExportedNameResolution::Ambiguous
                                | ExportedNameResolution::Blocked => unresolved_mapping = true,
                            }
                        } else {
                            unresolved_mapping = true;
                        }
                    } else {
                        unresolved_mapping = true;
                    }
                }
            }
        }
    }

    let result = if has_matching_mapping {
        if unresolved_mapping || mapped_targets.is_empty() {
            ExportedNameResolution::Blocked
        } else {
            unique_target(mapped_targets)
                .map_or(ExportedNameResolution::Ambiguous, |(target, depth)| {
                    ExportedNameResolution::Resolved(target, depth)
                })
        }
    } else if requested == "default" && !default_exports.is_empty() {
        resolve_default_export(
            module,
            &default_exports,
            modules,
            declarations_by_path,
            visited,
            remaining_steps,
            depth,
            config,
        )
        .map_or(ExportedNameResolution::Blocked, |(target, depth)| {
            ExportedNameResolution::Resolved(target, depth)
        })
    } else if requested != "default" && !star_exports.is_empty() {
        let mut wildcard_targets = Vec::new();
        let mut unresolved_source = false;
        for star_export in star_exports {
            let Some(specifier) = star_export.get("source").and_then(Value::as_str) else {
                unresolved_source = true;
                break;
            };
            let Some((target_module, _)) =
                resolve_module_specifier(module, specifier, modules, config)
            else {
                unresolved_source = true;
                break;
            };
            match resolve_exported_name(
                target_module,
                requested,
                modules,
                declarations_by_path,
                visited,
                remaining_steps,
                depth + 1,
                config,
            ) {
                ExportedNameResolution::Resolved(target, nested_depth) => {
                    wildcard_targets.push((target, nested_depth + 1));
                }
                ExportedNameResolution::NotFound => {}
                ExportedNameResolution::Ambiguous | ExportedNameResolution::Blocked => {
                    unresolved_source = true;
                    break;
                }
            }
        }
        if unresolved_source {
            ExportedNameResolution::Blocked
        } else if wildcard_targets.is_empty() {
            ExportedNameResolution::NotFound
        } else {
            unique_target(wildcard_targets)
                .map_or(ExportedNameResolution::Ambiguous, |(target, depth)| {
                    ExportedNameResolution::Resolved(target, depth)
                })
        }
    } else {
        ExportedNameResolution::NotFound
    };
    visited.remove(&path);
    result
}

/// Resolves a member through a source-backed namespace re-export chain.
///
/// The first hop is a direct `export * as name from "…"` clause. When the
/// module declares no such clause, the namespace name itself may be
/// forwarded by a source-backed named re-export, so the chain follows that
/// alias to the module that declares the namespace. `None` means there is
/// no namespace alias for this import; `Some(None)` means a matching alias
/// exists but is ambiguous, unresolved, cyclic, or over the re-export depth
/// or visit bounds. The caller uses that distinction to preserve existing
/// directly qualified imported-class member resolution without falling
/// back from a broken namespace mapping.
#[allow(clippy::too_many_arguments)]
fn resolve_namespace_exported_member<'a>(
    module: &'a Entity,
    namespace: &str,
    member: &str,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    visited: &mut HashSet<String>,
    remaining_steps: &mut usize,
    depth: usize,
    config: &ModuleResolutionConfig,
) -> Option<Option<(&'a Entity, usize)>> {
    let exports = module.payload.get("exports").and_then(Value::as_array)?;
    let namespace_exports = exports
        .iter()
        .filter(|export| export.get("namespace_name").and_then(Value::as_str) == Some(namespace))
        .collect::<Vec<_>>();
    if namespace_exports.is_empty() {
        return resolve_forwarded_namespace_alias(
            module,
            namespace,
            member,
            modules,
            declarations_by_path,
            visited,
            remaining_steps,
            depth,
            config,
        );
    }
    if namespace_exports.len() != 1 {
        return Some(None);
    }

    let export = namespace_exports[0];
    if export.get("star").and_then(Value::as_bool) != Some(false)
        || !export
            .get("bindings")
            .and_then(Value::as_array)
            .is_some_and(|bindings| bindings.is_empty())
    {
        return Some(None);
    }
    let Some(specifier) = export.get("source").and_then(Value::as_str) else {
        return Some(None);
    };
    let Some(path) = entity_path(module) else {
        return Some(None);
    };
    if !visited.insert(path.clone()) {
        return Some(None);
    }

    let resolved = match resolve_module_specifier(module, specifier, modules, config) {
        Some((target_module, _)) => resolve_exported_name(
            target_module,
            member,
            modules,
            declarations_by_path,
            visited,
            remaining_steps,
            depth + 1,
            config,
        )
        .into_resolved(),
        None => None,
    };
    visited.remove(&path);
    Some(resolved.map(|(target, nested_depth)| (target, nested_depth + 1)))
}

/// Follows a namespace name that a module forwards through a
/// source-backed named re-export, so one namespace alias can span
/// several modules before its declaring `export * as name` clause.
///
/// Returns `None` when the module forwards no such namespace name, so
/// the caller keeps its non-namespace fallbacks, and `Some(None)` when
/// a forwarding exists but is ambiguous, unresolved, cyclic, or over the
/// re-export depth or visit bounds.
#[allow(clippy::too_many_arguments)]
fn resolve_forwarded_namespace_alias<'a>(
    module: &'a Entity,
    namespace: &str,
    member: &str,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    visited: &mut HashSet<String>,
    remaining_steps: &mut usize,
    depth: usize,
    config: &ModuleResolutionConfig,
) -> Option<Option<(&'a Entity, usize)>> {
    if depth > MAX_REEXPORT_DEPTH || *remaining_steps == 0 {
        return Some(None);
    }
    *remaining_steps -= 1;
    let Some(path) = entity_path(module) else {
        return Some(None);
    };
    let exports = module.payload.get("exports").and_then(Value::as_array)?;
    let mut forwardings = Vec::new();
    for export in exports {
        if export
            .get("namespace_name")
            .and_then(Value::as_str)
            .is_some()
            || export.get("star").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        let Some(specifier) = export.get("source").and_then(Value::as_str) else {
            continue;
        };
        let Some(bindings) = export.get("bindings").and_then(Value::as_array) else {
            continue;
        };
        for binding in bindings {
            if binding.get("exported").and_then(Value::as_str) != Some(namespace) {
                continue;
            }
            if let Some(imported) = binding.get("imported").and_then(Value::as_str) {
                forwardings.push((specifier.to_string(), imported.to_string()));
            }
        }
    }
    if forwardings.is_empty() {
        return None;
    }
    if forwardings.len() != 1 {
        return Some(None);
    }
    let (specifier, imported) = forwardings.into_iter().next()?;
    if !visited.insert(path.clone()) {
        return Some(None);
    }
    let resolved = match resolve_module_specifier(module, &specifier, modules, config) {
        Some((target_module, _)) => resolve_namespace_exported_member(
            target_module,
            &imported,
            member,
            modules,
            declarations_by_path,
            visited,
            remaining_steps,
            depth + 1,
            config,
        ),
        None => None,
    };
    visited.remove(&path);
    Some(resolved.flatten())
}

/// Retains direct `ImportedClass.method()` resolution without treating an
/// unrelated exported `method` as evidence for a missing namespace alias.
fn has_direct_imported_member(
    module: &Entity,
    imported: &str,
    member: &str,
    declarations_by_path: &HashMap<String, Vec<&Entity>>,
) -> bool {
    let Some(path) = entity_path(module) else {
        return false;
    };
    let expected = format!("{imported}.{member}");
    declarations_by_path
        .get(&path)
        .into_iter()
        .flat_map(|candidates| candidates.iter().copied())
        .any(|candidate| {
            is_exported(candidate)
                && candidate.name == member
                && qualified_name(candidate) == Some(expected.as_str())
        })
}

/// Resolves one explicit default export to a unique same-module or imported function binding.
#[allow(clippy::too_many_arguments)]
fn resolve_default_export<'a>(
    module: &'a Entity,
    default_exports: &[&Value],
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    visited: &mut HashSet<String>,
    remaining_steps: &mut usize,
    depth: usize,
    config: &ModuleResolutionConfig,
) -> Option<(&'a Entity, usize)> {
    if default_exports.len() != 1 {
        return None;
    }
    let local_name = default_exports[0]
        .get("default_name")
        .and_then(Value::as_str)?;
    let path = entity_path(module)?;
    let local_matches = declarations_by_path
        .get(&path)
        .into_iter()
        .flat_map(|candidates| candidates.iter().copied())
        .filter(|candidate| qualified_name(candidate) == Some(local_name))
        .collect::<Vec<_>>();
    if !local_matches.is_empty() {
        return unique_target(local_matches.into_iter().map(|candidate| (candidate, 0)));
    }

    let binding = import_binding(module, local_name)?;
    let imported_name = binding.imported_name.as_deref()?;
    let (target_module, _) = resolve_module_specifier(module, &binding.specifier, modules, config)?;
    resolve_exported_name(
        target_module,
        imported_name,
        modules,
        declarations_by_path,
        visited,
        remaining_steps,
        depth + 1,
        config,
    )
    .into_resolved()
    .map(|(target, nested_depth)| (target, nested_depth + 1))
}

fn unique_target<'a>(
    candidates: impl IntoIterator<Item = (&'a Entity, usize)>,
) -> Option<(&'a Entity, usize)> {
    let mut found: Option<(&'a Entity, usize)> = None;
    for candidate in candidates {
        if let Some((existing, existing_depth)) = found {
            if existing.id != candidate.0.id {
                return None;
            }
            if candidate.1 < existing_depth {
                found = Some(candidate);
            }
        } else {
            found = Some(candidate);
        }
    }
    found
}

/// The recorded type of one receiver binding.
#[derive(Clone)]
struct TypedReceiver {
    type_name: String,
    kind: &'static str,
}

/// Reads the receiver types one module proves.
///
/// A name bound to two different types is ambiguous and names nothing, so it is
/// recorded as unresolved instead of letting one declaration win by accident.
fn typed_receivers(module: &Entity) -> HashMap<String, Option<TypedReceiver>> {
    let mut receivers: HashMap<String, Option<TypedReceiver>> = HashMap::new();
    let Some(bindings) = module
        .payload
        .get("typed_bindings")
        .and_then(Value::as_array)
    else {
        return receivers;
    };
    for binding in bindings {
        let Some(name) = binding.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(type_name) = binding.get("type").and_then(Value::as_str) else {
            continue;
        };
        let kind = match binding.get("kind").and_then(Value::as_str) {
            Some("construction") => "construction",
            Some("element") => "element",
            _ => "annotation",
        };
        let recorded = TypedReceiver {
            type_name: type_name.to_string(),
            kind,
        };
        let prior = receivers.get(name).map(|entry| {
            entry
                .as_ref()
                .map(|receiver| (receiver.type_name.clone(), receiver.kind))
        });
        match prior {
            None => {
                receivers.insert(name.to_string(), Some(recorded));
            }
            Some(None) => {}
            Some(Some((existing_type, existing_kind))) => {
                if existing_type != type_name {
                    receivers.insert(name.to_string(), None);
                } else if receiver_strength(kind) > receiver_strength(existing_kind) {
                    receivers.insert(name.to_string(), Some(recorded));
                }
            }
        }
    }
    receivers
}

/// How strong one recorded receiver type is: a construction the module performs
/// beats an annotation, and an annotation on the binding itself beats the
/// element type read from the collection the binding came out of.
fn receiver_strength(kind: &str) -> u8 {
    match kind {
        "construction" => 2,
        "annotation" => 1,
        _ => 0,
    }
}

/// Maps a type name to the module that declares it.
///
/// Only an exported top-level class, interface, or type alias can own the field
/// types a receiver path walks, and a name declared in two modules is ambiguous,
/// so the value is `None` and no field is read from either module.
fn build_type_module_index<'a>(
    entities: &'a [Entity],
    modules: &HashMap<String, &'a Entity>,
) -> HashMap<String, Option<&'a Entity>> {
    let mut owners: HashMap<String, Option<&'a Entity>> = HashMap::new();
    for entity in entities {
        if !matches!(entity.kind, EntityKind::Class | EntityKind::Symbol) {
            continue;
        }
        let Some(name) = qualified_name(entity) else {
            continue;
        };
        if name.contains('.') || !is_exported(entity) {
            continue;
        }
        let kind = entity
            .payload
            .get("symbol_kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(kind, "class" | "interface" | "type_alias") {
            continue;
        }
        let Some(path) = entity_path(entity) else {
            continue;
        };
        let Some(module) = modules.get(&path).copied() else {
            continue;
        };
        let prior = owners.get(name).copied();
        match prior {
            None => {
                owners.insert(name.to_string(), Some(module));
            }
            Some(Some(existing)) => {
                if existing.id != module.id {
                    owners.insert(name.to_string(), None);
                }
            }
            Some(None) => {}
        }
    }
    owners
}

/// Resolutions strong enough to name a factory whose declared return type may
/// type a receiver. A unique-name guess does not prove which function ran, so it
/// cannot decide what a receiver is.
const FACTORY_RESOLUTIONS: &[&str] = &[
    "same_module_this_method",
    "same_module_exact_name",
    "imported_exact_name",
    "imported_symbol_name",
    "imported_default_export",
    "imported_reexport_chain",
];

/// Reads the declared return type one declaration records.
fn declared_return_type(entity: &Entity) -> Option<&str> {
    entity.payload.get("return_type").and_then(Value::as_str)
}

/// The type an `await` on a call to this declaration yields: `Promise<Repo>` and
/// an `async` declaration's bare annotation both name `Repo`.
fn awaited_return_type(entity: &Entity) -> Option<&str> {
    entity
        .payload
        .get("awaited_return_type")
        .and_then(Value::as_str)
}

/// The type a declaration constructs and returns: a body that builds the value
/// it hands back states the type even when the declaration carries no
/// annotation.
fn returned_type(entity: &Entity) -> Option<&str> {
    entity.payload.get("returned_type").and_then(Value::as_str)
}

/// The type a declaration asserts on every `return`: `return load() as Repo`.
fn asserted_type(entity: &Entity) -> Option<&str> {
    entity.payload.get("asserted_type").and_then(Value::as_str)
}

/// The type a function-type annotation yields: `(): () => Repo` records `Repo`.
fn returned_function_type(entity: &Entity) -> Option<&str> {
    entity
        .payload
        .get("returned_function_type")
        .and_then(Value::as_str)
}

/// The one factory a wrapper's body returns a call to, when it constructs
/// nothing of its own.
fn returned_callee(entity: &Entity) -> Option<&str> {
    entity
        .payload
        .get("returned_callee")
        .and_then(Value::as_str)
}

fn is_async_declaration(entity: &Entity) -> bool {
    entity
        .payload
        .get("async")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Resolves the one factory a wrapper's `returned_callee` names, from the
/// wrapper's module, with the same provenance a receiver factory needs.
#[allow(clippy::too_many_arguments)]
fn forwarded_inner<'a>(
    factory: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<&'a Entity> {
    let callee = returned_callee(factory)?;
    let factory_module = *modules.get(&entity_path(factory)?)?;
    let inner = resolve_call(
        callee,
        factory,
        factory_module,
        modules,
        declarations_by_path,
        declarations_by_name,
        type_modules,
        config,
        false,
    )?;
    FACTORY_RESOLUTIONS.contains(&inner.1).then_some(inner.0)
}

/// One hop through a factory that only returns another factory's call.
///
/// The wrapper names the callee, not the type, so the linker reads the inner
/// factory's annotation or constructed return from the wrapper's module. The
/// hop does not follow another `returned_callee`: a chain of wrappers is not
/// the type the source states. An `async` wrapper, and an inner factory that
/// yields a promise, type nothing on an unawaited binding: the binding holds
/// the promise, not the awaited value.
#[allow(clippy::too_many_arguments)]
fn forwarded_return_type<'a>(
    factory: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<(String, &'static str)> {
    if is_async_declaration(factory) {
        return None;
    }
    let inner = forwarded_inner(
        factory,
        modules,
        declarations_by_path,
        declarations_by_name,
        type_modules,
        config,
    )?;
    if let Some(declared) = declared_return_type(inner) {
        return Some((declared.to_string(), "annotation"));
    }
    // An inner factory that yields a promise does not type an unawaited
    // wrapper: `return makeRepo()` holds the promise, not the value `await`
    // would unwrap. The hop does not read `awaited_return_type`.
    if awaited_return_type(inner).is_some() {
        return None;
    }
    if let Some(returned) = returned_type(inner) {
        return Some((returned.to_string(), "returned"));
    }
    if let Some(asserted) = asserted_type(inner) {
        return Some((asserted.to_string(), "annotation"));
    }
    None
}

/// One hop through an `async` wrapper the module awaits.
///
/// `await wrap()` yields what the inner factory yields: its awaited type if it
/// returns a promise, otherwise its annotation or constructed return. A second
/// hop is not the type the source states.
#[allow(clippy::too_many_arguments)]
fn forwarded_awaited_type<'a>(
    factory: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<(String, &'static str)> {
    let inner = forwarded_inner(
        factory,
        modules,
        declarations_by_path,
        declarations_by_name,
        type_modules,
        config,
    )?;
    if let Some(awaited) = awaited_return_type(inner) {
        return Some((awaited.to_string(), "awaited"));
    }
    if let Some(declared) = declared_return_type(inner) {
        return Some((declared.to_string(), "annotation"));
    }
    if let Some(returned) = returned_type(inner) {
        return Some((returned.to_string(), "returned"));
    }
    if let Some(asserted) = asserted_type(inner) {
        return Some((asserted.to_string(), "annotation"));
    }
    None
}

/// One binding the module initializes with a call.
#[derive(Clone)]
struct CallReceiver {
    callee: String,
    awaited: bool,
    invoked: bool,
}

/// Reads the bindings one module initializes with a call.
///
/// A name bound to two different factories is ambiguous and names nothing, so it
/// is recorded as unresolved instead of letting one call win by accident.
fn call_receivers(module: &Entity) -> HashMap<String, Option<CallReceiver>> {
    let mut receivers: HashMap<String, Option<CallReceiver>> = HashMap::new();
    let Some(bindings) = module
        .payload
        .get("call_bindings")
        .and_then(Value::as_array)
    else {
        return receivers;
    };
    for binding in bindings {
        let Some(name) = binding.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(callee) = binding.get("callee").and_then(Value::as_str) else {
            continue;
        };
        let recorded = CallReceiver {
            callee: callee.to_string(),
            awaited: binding
                .get("awaited")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            invoked: binding
                .get("invoked")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        let prior = receivers.get(name).cloned();
        match prior {
            None => {
                receivers.insert(name.to_string(), Some(recorded));
            }
            Some(Some(existing)) => {
                if existing.callee != callee
                    || existing.awaited != recorded.awaited
                    || existing.invoked != recorded.invoked
                {
                    receivers.insert(name.to_string(), None);
                }
            }
            Some(None) => {}
        }
    }
    receivers
}

/// Reads the type one field of a typed owner declares.
///
/// A field row lives in the module that declares its owner, so a receiver that
/// reaches into a type from another module is resolved there: `App.repo` is a
/// local row, while `Repo.items` is read from the module declaring `Repo`.
fn typed_field(
    owner: &str,
    segment: &str,
    receivers: &HashMap<String, Option<TypedReceiver>>,
    type_modules: &HashMap<String, Option<&Entity>>,
) -> Option<TypedReceiver> {
    let field = format!("{owner}.{segment}");
    if let Some(found) = receivers.get(&field) {
        return found.clone();
    }
    let module = type_modules.get(owner)?.as_ref()?;
    typed_receivers(module).get(&field)?.clone()
}

/// A synthetic first segment the extractor records for a member of a call,
/// construction, or inline assertion that has no binding: `createRepo()`,
/// `createRepo()@`, `create()()`, `Repo{}`, `Repo#`.
enum CallResultRoot {
    Factory {
        name: String,
        awaited: bool,
        invoked: bool,
    },
    Construction {
        name: String,
    },
    Assertion {
        name: String,
    },
}

/// Reads a factory- or construction-result segment the extractor named.
fn call_result_root(segment: &str) -> Option<CallResultRoot> {
    if let Some(name) = segment.strip_suffix("{}") {
        if call_result_ident(name) {
            return Some(CallResultRoot::Construction {
                name: name.to_string(),
            });
        }
        return None;
    }
    if let Some(name) = segment.strip_suffix("()@") {
        if call_result_ident(name) {
            return Some(CallResultRoot::Factory {
                name: name.to_string(),
                awaited: true,
                invoked: false,
            });
        }
        return None;
    }
    if let Some(name) = segment.strip_suffix("()") {
        if call_result_ident(name) {
            return Some(CallResultRoot::Factory {
                name: name.to_string(),
                awaited: false,
                invoked: false,
            });
        }
        if let Some(inner) = name.strip_suffix("()") {
            if call_result_ident(inner) {
                return Some(CallResultRoot::Factory {
                    name: inner.to_string(),
                    awaited: false,
                    invoked: true,
                });
            }
        }
        return None;
    }
    if let Some(name) = segment.strip_suffix('#') {
        if call_result_ident(name) {
            return Some(CallResultRoot::Assertion {
                name: name.to_string(),
            });
        }
        return None;
    }
    None
}

fn call_result_ident(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('.')
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_' || character == '$')
}

/// Types a receiver from the factory the source named, with the same provenance
/// a `call_bindings` row uses: a local, imported, or re-exported declaration,
/// never a unique-name guess.
#[allow(clippy::too_many_arguments)]
fn factory_typed_receiver<'a>(
    callee: &str,
    awaited: bool,
    invoked: bool,
    caller: &'a Entity,
    caller_module: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<TypedReceiver> {
    let factory = resolve_call(
        callee,
        caller,
        caller_module,
        modules,
        declarations_by_path,
        declarations_by_name,
        type_modules,
        config,
        false,
    )?;
    if !FACTORY_RESOLUTIONS.contains(&factory.1) {
        return None;
    }
    factory_return_receiver(
        factory.0,
        awaited,
        invoked,
        modules,
        declarations_by_path,
        declarations_by_name,
        type_modules,
        config,
    )
}

/// The type a factory declaration states for a receiver: annotation, constructed
/// return, asserted return, or one forwarded hop, and the awaited type when the
/// source awaits the call. An extra call of the result (`create()()`) takes the
/// function-type yield instead, and not the value the first call returns.
#[allow(clippy::too_many_arguments)]
fn factory_return_receiver<'a>(
    factory: &'a Entity,
    awaited: bool,
    invoked: bool,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<TypedReceiver> {
    if invoked {
        if awaited {
            return None;
        }
        let type_name = returned_function_type(factory)?.to_string();
        return Some(TypedReceiver {
            type_name,
            kind: "annotation",
        });
    }
    // `await f()` takes what the promise yields rather than what the call
    // returns, so the binding asks for the awaited type when the source
    // awaits it. Otherwise the annotation the factory declares wins, a
    // factory that declares none is typed by the one type every `return` of
    // its body constructs, and a wrapper that constructs nothing is typed
    // by the one factory it returns a call to — one hop, and no further.
    let declared = declared_return_type(factory);
    let (type_name, kind) = if awaited {
        if let Some(awaited_type) = awaited_return_type(factory) {
            (awaited_type.to_string(), "awaited")
        } else {
            forwarded_awaited_type(
                factory,
                modules,
                declarations_by_path,
                declarations_by_name,
                type_modules,
                config,
            )?
        }
    } else if let Some(declared) = declared {
        (declared.to_string(), "annotation")
    } else if let Some(returned) = returned_type(factory) {
        (returned.to_string(), "returned")
    } else if let Some(asserted) = asserted_type(factory) {
        (asserted.to_string(), "annotation")
    } else {
        forwarded_return_type(
            factory,
            modules,
            declarations_by_path,
            declarations_by_name,
            type_modules,
            config,
        )?
    };
    Some(TypedReceiver { type_name, kind })
}

/// The method `Owner.method` when the snapshot states it, including a method
/// the owner inherits through `extends`.
fn typed_method<'a>(
    owner: &str,
    method: &str,
    owner_hint: Option<&str>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
) -> Option<&'a Entity> {
    let qualified = format!("{owner}.{method}");
    if let Some(target) = unique_match(declarations_by_name.get(method), |candidate| {
        qualified_name(candidate) == Some(qualified.as_str())
    }) {
        return Some(target);
    }
    inherited_member(owner, method, owner_hint, declarations_by_name)
}

/// Types a receiver from a call of a method on the current type: `svc.build()`.
#[allow(clippy::too_many_arguments)]
fn method_call_receiver<'a>(
    owner: &str,
    method: &str,
    awaited: bool,
    invoked: bool,
    caller_module: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<TypedReceiver> {
    let owner_hint = type_modules
        .get(owner)
        .and_then(|module| module.as_ref())
        .and_then(|module| entity_path(module))
        .or_else(|| entity_path(caller_module));
    let factory = typed_method(owner, method, owner_hint.as_deref(), declarations_by_name)?;
    factory_return_receiver(
        factory,
        awaited,
        invoked,
        modules,
        declarations_by_path,
        declarations_by_name,
        type_modules,
        config,
    )
}

/// One hop through a type alias the source writes: `type T = Repo` names `Repo`.
///
/// A union, a function type, and an alias that names itself stay the original
/// name, so the walk does not invent a type. A second hop is not taken.
fn unfold_alias_type<'a>(
    type_name: &str,
    hint: Option<&str>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
) -> String {
    let Some(alias) = type_alias_declaration(type_name, hint, declarations_by_name) else {
        return type_name.to_string();
    };
    match alias.payload.get("aliased_type").and_then(Value::as_str) {
        Some(aliased) if !aliased.is_empty() && aliased != type_name => aliased.to_string(),
        _ => type_name.to_string(),
    }
}

/// The unique `type` declaration that `type_name` names, when the snapshot
/// states exactly one.
fn type_alias_declaration<'a>(
    type_name: &str,
    hint: Option<&str>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
) -> Option<&'a Entity> {
    let candidates = declarations_by_name
        .get(type_name)?
        .iter()
        .copied()
        .filter(|candidate| {
            candidate
                .payload
                .get("symbol_kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == "type_alias")
        })
        .collect::<Vec<_>>();
    if let Some(hint) = hint {
        let same_module = candidates
            .iter()
            .copied()
            .filter(|candidate| entity_path(candidate).as_deref() == Some(hint))
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

/// The type `super` names: the one class the caller's owner extends.
fn super_receiver<'a>(
    caller: &'a Entity,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
) -> Option<TypedReceiver> {
    let owner = method_owner(caller)?;
    let hint = entity_path(caller);
    let owner_entity = heritage_base(owner, hint.as_deref(), declarations_by_name)?;
    let base = extends_name(owner_entity)?;
    Some(TypedReceiver {
        type_name: base,
        kind: "construction",
    })
}

/// Resolves a member call whose receiver the source typed.
///
/// The receiver path is walked segment by segment: `this` starts at the calling
/// declaration's class, `super` starts at the class that owner extends, and
/// every other segment must name a binding of the calling module or a field of
/// the current type whose recorded type continues the path. A type alias
/// unfolds one hop (`type Client = Repo` continues as `Repo`). A field is read from the calling module first and then from the
/// module that declares the owning type, so a path can leave the calling file.
/// The root may also be a factory or construction the callee path itself names
/// (`createRepo().save`, `create()().save`, `Repo{}.save`, `Repo#.save`) with the same type evidence
/// a bound `const repo = createRepo()` or `value as Repo` uses. A later segment may be a call of a method
/// on the current type (`svc.build().save`, `createRepo().getStore().save`):
/// the method's stated return type continues the path. The walk is bounded, a
/// repeated type ends it, and an untyped or ambiguous segment leaves the call
/// unresolved rather than falling back to a name match.
#[allow(clippy::too_many_arguments)]
fn resolve_typed_member<'a>(
    callee: &str,
    caller: &'a Entity,
    caller_module: &'a Entity,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
    type_modules: &HashMap<String, Option<&'a Entity>>,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    config: &ModuleResolutionConfig,
) -> Option<(&'a Entity, &'static str, usize, bool)> {
    let (receiver, member) = callee.rsplit_once('.')?;
    if receiver.is_empty() || member.is_empty() {
        return None;
    }
    let receivers = typed_receivers(caller_module);
    let call_receivers = call_receivers(caller_module);
    let segments = receiver.split('.').collect::<Vec<_>>();
    if segments.len() > MAX_TYPE_PATH_SEGMENTS || segments.iter().any(|segment| segment.is_empty())
    {
        return None;
    }

    let root = if segments[0] == "this" {
        TypedReceiver {
            type_name: method_owner(caller)?.to_string(),
            kind: "construction",
        }
    } else if segments[0] == "super" {
        // `super.save()` names the base class the owner extends: the heritage
        // clause is written in the source, so the walk starts at that type.
        super_receiver(caller, declarations_by_name)?
    } else if let Some(Some(binding)) = receivers.get(segments[0]) {
        binding.clone()
    } else if let Some(call_root) = call_result_root(segments[0]) {
        // `createRepo().save()` has no binding: the callee path itself names
        // the factory or constructor, so the walk starts at that result.
        match call_root {
            CallResultRoot::Construction { name } => TypedReceiver {
                type_name: name,
                kind: "construction",
            },
            CallResultRoot::Assertion { name } => TypedReceiver {
                type_name: name,
                kind: "annotation",
            },
            CallResultRoot::Factory {
                name,
                awaited,
                invoked,
            } => factory_typed_receiver(
                &name,
                awaited,
                invoked,
                caller,
                caller_module,
                modules,
                declarations_by_path,
                declarations_by_name,
                type_modules,
                config,
            )?,
        }
    } else {
        // A binding initialized by a call takes the factory's declared return
        // type: the type is stated by the factory's own annotation, not inferred
        // from the call's name.
        let call = call_receivers.get(segments[0])?.as_ref()?;
        factory_typed_receiver(
            &call.callee,
            call.awaited,
            call.invoked,
            caller,
            caller_module,
            modules,
            declarations_by_path,
            declarations_by_name,
            type_modules,
            config,
        )?
    };
    let hint = entity_path(caller).or_else(|| entity_path(caller_module));
    let mut current = unfold_alias_type(&root.type_name, hint.as_deref(), declarations_by_name);
    let mut kind = root.kind;

    let mut visited = vec![current.clone()];
    for segment in segments.iter().skip(1) {
        let next = if let Some(call_root) = call_result_root(segment) {
            // `svc.build()` calls a method of the current type and continues
            // from that method's stated return type. A nested construction
            // (`Repo{}.save` as a non-root segment) is not a method call.
            match call_root {
                CallResultRoot::Construction { .. } | CallResultRoot::Assertion { .. } => {
                    return None;
                }
                CallResultRoot::Factory {
                    name,
                    awaited,
                    invoked,
                } => method_call_receiver(
                    &current,
                    &name,
                    awaited,
                    invoked,
                    caller_module,
                    modules,
                    declarations_by_path,
                    declarations_by_name,
                    type_modules,
                    config,
                )?,
            }
        } else {
            typed_field(&current, segment, &receivers, type_modules)?
        };
        current = unfold_alias_type(&next.type_name, hint.as_deref(), declarations_by_name);
        kind = next.kind;
        if visited.contains(&current) {
            return None;
        }
        visited.push(current.clone());
    }

    let qualified = format!("{current}.{member}");
    if let Some(target) = unique_match(declarations_by_name.get(member), |candidate| {
        qualified_name(candidate) == Some(qualified.as_str())
    }) {
        return Some((target, kind, segments.len(), false));
    }

    // A class need not declare the member it is called with: it inherits the
    // members of the class it extends. The heritage clause is written in the
    // source, so the base is named evidence — but it is one step further from
    // the call site than a member the receiver's own type declares.
    let owner_hint = type_modules
        .get(&current)
        .and_then(|module| module.as_ref())
        .and_then(|module| entity_path(module))
        .or_else(|| entity_path(caller_module));
    let target = inherited_member(
        &current,
        member,
        owner_hint.as_deref(),
        declarations_by_name,
    )?;
    Some((target, kind, segments.len(), true))
}

/// Resolves a member a type inherits from the class it extends.
///
/// The walk is bounded, a type already seen ends it, and a base that is
/// ambiguous or absent ends it: an inherited member is only named when the
/// chain from the receiver's type to the declaring class is unbroken. Interfaces
/// declare no bodies to call, so only `extends` is followed.
fn inherited_member<'a>(
    type_name: &str,
    member: &str,
    owner_hint: Option<&str>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
) -> Option<&'a Entity> {
    let mut current = heritage_base(type_name, owner_hint, declarations_by_name)?;
    let mut hint = entity_path(current);
    let mut visited = vec![type_name.to_string()];
    for _ in 0..MAX_HERITAGE_DEPTH {
        let base = extends_name(current)?;
        if visited.iter().any(|seen| seen == &base) {
            return None;
        }
        // The base itself has to be a declaration the snapshot states, for the
        // same reason as the derived type: a name two files declare is not
        // evidence of which one holds the member.
        let owner = heritage_base(&base, hint.as_deref(), declarations_by_name)?;
        let qualified = format!("{base}.{member}");
        if let Some(target) = unique_match(declarations_by_name.get(member), |candidate| {
            qualified_name(candidate) == Some(qualified.as_str())
        }) {
            return Some(target);
        }
        visited.push(base);
        hint = entity_path(owner);
        current = owner;
    }
    None
}

/// Finds the one class or interface entity a type name states, preferring a
/// unique declaration in the file the hint names.
fn heritage_base<'a>(
    type_name: &str,
    hint: Option<&str>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
) -> Option<&'a Entity> {
    let candidates = declarations_by_name
        .get(type_name)?
        .iter()
        .copied()
        .filter(|candidate| {
            matches!(
                candidate
                    .payload
                    .get("symbol_kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                "class" | "interface"
            )
        })
        .collect::<Vec<_>>();
    if let Some(hint) = hint {
        let same_module = candidates
            .iter()
            .copied()
            .filter(|candidate| entity_path(candidate).as_deref() == Some(hint))
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

/// The one class a type's `extends` clause names, when it names any.
fn extends_name(entity: &Entity) -> Option<String> {
    entity
        .payload
        .get("extends")?
        .as_array()?
        .iter()
        .find_map(Value::as_str)
        .map(str::to_string)
}

fn find_first<'a>(
    candidates: Option<&Vec<&'a Entity>>,
    predicate: impl Fn(&Entity) -> bool,
) -> Option<&'a Entity> {
    candidates?
        .iter()
        .copied()
        .find(|candidate| predicate(candidate))
}

fn unique_match<'a>(
    candidates: Option<&Vec<&'a Entity>>,
    predicate: impl Fn(&Entity) -> bool,
) -> Option<&'a Entity> {
    let mut found: Option<&'a Entity> = None;
    for candidate in candidates? {
        if predicate(candidate) {
            if found.is_some() {
                return None;
            }
            found = Some(candidate);
        }
    }
    found
}

/// Reports whether a path sits in a JavaScript/TypeScript test location.
///
/// The extractor does not run a test framework, so the location itself is the
/// evidence: `.test.`/`.spec.` file names and conventional test directories.
fn is_test_file(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    if stem.ends_with(".test") || stem.ends_with(".spec") {
        return true;
    }
    path.split('/')
        .any(|segment| TEST_DIRECTORIES.contains(&segment))
}

#[allow(clippy::too_many_arguments)]
fn call_relation(
    snapshot: &SnapshotId,
    caller: &Entity,
    target: &Entity,
    callee: &str,
    call_kind: &str,
    resolution: &str,
    confidence: f32,
    line: Option<u64>,
    column: Option<u64>,
) -> Relation {
    let id_material = format!(
        "js_ts_call\0{}\0{}\0{}",
        caller.stable_key.0, target.stable_key.0, callee
    );
    Relation {
        id: RelationId(format!(
            "rel_js_ts_call_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Calls,
        from: caller.id.clone(),
        to: target.id.clone(),
        status: RelationStatus::Inferred,
        confidence,
        evidence: call_evidence(caller, target, line, confidence),
        ownership: combined_ownership(caller, target),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": caller.stable_key.0,
            "to": target.stable_key.0,
            "callee": callee,
            "callee_kind": call_kind,
            "resolution": resolution,
            "line": line,
            "column": column,
            "reason": "js_ts_static_call",
        }),
    }
}

/// Builds the indirect call edge for one resolved callback argument.
///
/// The relation id uses its own material prefix so a declaration that is
/// both called directly and passed as a callback keeps both edges: the
/// evidence shapes differ, and blast-radius queries deserve the explicit
/// indirect path.
#[allow(clippy::too_many_arguments)]
fn callback_relation(
    snapshot: &SnapshotId,
    caller: &Entity,
    target: &Entity,
    callback: &str,
    host_callee: &str,
    resolution: &str,
    confidence: f32,
    line: Option<u64>,
    column: Option<u64>,
) -> Relation {
    let id_material = format!(
        "js_ts_callback\0{}\0{}\0{}",
        caller.stable_key.0, target.stable_key.0, callback
    );
    Relation {
        id: RelationId(format!(
            "rel_js_ts_callback_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Calls,
        from: caller.id.clone(),
        to: target.id.clone(),
        status: RelationStatus::Inferred,
        confidence,
        evidence: call_evidence(caller, target, line, confidence),
        ownership: combined_ownership(caller, target),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": caller.stable_key.0,
            "to": target.stable_key.0,
            "callee": callback,
            "host_callee": host_callee,
            "callee_kind": "callback",
            "resolution": resolution,
            "line": line,
            "column": column,
            "reason": "js_ts_callback_bridge",
        }),
    }
}

fn tested_by_relation(
    snapshot: &SnapshotId,
    tested: &Entity,
    test: &Entity,
    callee: &str,
    line: Option<u64>,
) -> Relation {
    let id_material = format!(
        "js_ts_tested_by\0{}\0{}\0{}",
        tested.stable_key.0, test.stable_key.0, callee
    );
    Relation {
        id: RelationId(format!(
            "rel_js_ts_tested_by_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::TestedBy,
        from: tested.id.clone(),
        to: test.id.clone(),
        status: RelationStatus::Inferred,
        confidence: 0.6,
        evidence: call_evidence(test, tested, line, 0.6),
        ownership: combined_ownership(tested, test),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": tested.stable_key.0,
            "to": test.stable_key.0,
            "callee": callee,
            "line": line,
            "reason": "js_ts_test_call",
        }),
    }
}

fn call_evidence(
    caller: &Entity,
    target: &Entity,
    line: Option<u64>,
    confidence: f32,
) -> Vec<Evidence> {
    let mut evidence = Vec::new();
    if let Some(location) = &caller.source {
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: line.and_then(|line| u32::try_from(line).ok()),
            line_end: line.and_then(|line| u32::try_from(line).ok()),
            extractor: Some("js-ts-calls".to_string()),
            commit_hash: None,
            confidence,
            status: EvidenceStatus::Inferred,
        });
    }
    if let Some(location) = &target.source {
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: location.line_start,
            line_end: location.line_end,
            extractor: Some("js-ts-calls".to_string()),
            commit_hash: None,
            confidence,
            status: EvidenceStatus::Inferred,
        });
    }
    evidence
}

fn combined_ownership(source: &Entity, target: &Entity) -> Vec<Ownership> {
    let mut ownership = source.ownership.clone();
    for owner in &target.ownership {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
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
    use athanor_domain::{EntityId, LanguageCode, SourceLocation, StableKey};

    use super::*;

    #[tokio::test]
    async fn links_same_module_this_imported_and_unique_cross_module_calls() {
        let client = module("ent_client", "src/client.ts", json!({"imports": []}));
        let session_module = module("ent_session", "src/session.ts", json!({"imports": []}));
        let auth = module(
            "ent_auth",
            "src/auth.ts",
            json!({
                "imports": [
                    {"source": "./client", "default_name": "api", "named": ["login"]},
                    {"source": "./session", "namespace_name": "session"}
                ]
            }),
        );
        let login = declaration(
            "ent_login",
            "src/client.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let session = declaration(
            "ent_session_class",
            "src/session.ts",
            "Session",
            "Session",
            true,
            json!([]),
        );
        let refresh = declaration(
            "ent_refresh",
            "src/session.ts",
            "Session.refresh",
            "refresh",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_run",
            "src/auth.ts",
            "Session.run",
            "run",
            true,
            json!([
                call_site("login", "login", 4),
                call_site("session.Session.refresh", "refresh", 5),
                call_site("api.send", "send", 6),
                call_site("helper", "helper", 7)
            ]),
        );

        let entities = vec![
            client,
            session_module,
            auth,
            login.clone(),
            session,
            refresh.clone(),
            run.clone(),
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(!calls.iter().any(|relation| {
            relation.payload["callee"] == "api.send" || relation.payload["callee"] == "helper"
        }));
        assert!(calls.iter().any(|relation| {
            relation.to == login.id
                && relation.payload["resolution"] == "imported_exact_name"
                && relation.payload["callee"] == "login"
                && relation.payload["line"].as_u64() == Some(4)
        }));
        assert!(calls.iter().any(|relation| {
            relation.to == refresh.id && relation.payload["resolution"] == "imported_symbol_name"
        }));
        assert!(calls.iter().all(|relation| {
            relation.status == RelationStatus::Inferred
                && relation.evidence.len() == 2
                && !relation.ownership.is_empty()
        }));
    }

    #[tokio::test]
    async fn resolves_calls_through_named_reexport_chains_and_import_aliases() {
        let leaf_module = module("ent_leaf_module", "src/leaf.ts", json!({"imports": []}));
        let bridge_module = module(
            "ent_bridge_module",
            "src/bridge.ts",
            json!({
                "imports": [{
                    "source": "./leaf",
                    "named": ["auth"],
                    "named_bindings": [{"imported": "login", "local": "auth"}]
                }],
                "exports": [{
                    "source": null,
                    "bindings": [{"imported": "auth", "exported": "authenticate"}]
                }]
            }),
        );
        let barrel_module = module(
            "ent_barrel_module",
            "src/barrel.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./bridge",
                    "bindings": [{"imported": "authenticate", "exported": "signIn"}]
                }]
            }),
        );
        let index_module = module(
            "ent_index_module",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./barrel",
                    "bindings": [{"imported": "signIn", "exported": "publicSignIn"}]
                }]
            }),
        );
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["perform"],
                    "named_bindings": [{"imported": "publicSignIn", "local": "perform"}]
                }]
            }),
        );
        let login = declaration(
            "ent_leaf_login",
            "src/leaf.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("perform", "perform", 4)]),
        );
        let entities = vec![
            leaf_module,
            bridge_module,
            barrel_module,
            index_module,
            app_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn resolves_named_imports_through_single_source_wildcard_chains() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["perform"],
                    "named_bindings": [{"imported": "login", "local": "perform"}]
                }]
            }),
        );
        let index_module = module(
            "ent_index_module",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./barrel",
                    "bindings": [],
                    "star": true
                }]
            }),
        );
        let barrel_module = module(
            "ent_barrel_module",
            "src/barrel.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./auth", "bindings": [], "star": true}]
            }),
        );
        let auth_module = module("ent_auth_module", "src/auth.ts", json!({"imports": []}));
        let login = declaration(
            "ent_auth_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("perform", "perform", 5)]),
        );
        let entities = vec![
            app_module,
            index_module,
            barrel_module,
            auth_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn resolves_default_imports_to_named_default_exports() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{"source": "./auth", "default_name": "authenticate"}]
            }),
        );
        let auth_module = module(
            "ent_auth_module",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "login"
                }]
            }),
        );
        let login = declaration(
            "ent_auth_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("authenticate", "authenticate", 4)]),
        );
        let entities = vec![app_module, auth_module, login.clone(), run.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_default_export");
    }

    #[tokio::test]
    async fn resolves_default_imports_to_named_default_class_exports() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{"source": "./auth", "default_name": "Controller"}]
            }),
        );
        let auth_module = module(
            "ent_auth_module",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "Controller"
                }]
            }),
        );
        let controller = class_declaration(
            "ent_auth_controller",
            "src/auth.ts",
            "Controller",
            "Controller",
            true,
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([construction_site("Controller", "Controller", 4)]),
        );
        let entities = vec![app_module, auth_module, controller.clone(), run.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, controller.id);
        assert_eq!(calls[0].payload["resolution"], "imported_default_export");
        assert_eq!(calls[0].payload["callee_kind"], "new");
    }

    #[tokio::test]
    async fn resolves_same_module_construction_of_local_class() {
        let module_entity = module(
            "ent_widget_module",
            "src/widget.ts",
            json!({"imports": [], "exports": []}),
        );
        let widget = class_declaration(
            "ent_widget_class",
            "src/widget.ts",
            "Widget",
            "Widget",
            false,
        );
        let build = declaration(
            "ent_widget_build",
            "src/widget.ts",
            "build",
            "build",
            false,
            json!([construction_site("Widget", "Widget", 9)]),
        );
        let entities = vec![module_entity, widget.clone(), build.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, build.id);
        assert_eq!(calls[0].to, widget.id);
        assert_eq!(calls[0].payload["resolution"], "same_module_exact_name");
    }

    #[tokio::test]
    async fn resolves_default_reexports_through_named_export_aliases() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{
                    "source": "./barrel",
                    "named": ["authenticate"],
                    "named_bindings": [{"imported": "signIn", "local": "authenticate"}]
                }]
            }),
        );
        let barrel_module = module(
            "ent_barrel_module",
            "src/barrel.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./auth",
                    "bindings": [{"imported": "default", "exported": "signIn"}]
                }]
            }),
        );
        let auth_module = module(
            "ent_auth_module",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "login"
                }]
            }),
        );
        let login = declaration(
            "ent_auth_login",
            "src/auth.ts",
            "login",
            "login",
            false,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("authenticate", "authenticate", 5)]),
        );
        let entities = vec![
            app_module,
            barrel_module,
            auth_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn resolves_member_calls_through_a_construction_typed_receiver() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "service", "type": "Service", "kind": "construction", "line": 2}
                ]
            }),
        );
        let service_run = declaration(
            "ent_service_run",
            "src/service.ts",
            "Service.run",
            "run",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("service.run", "run", 5)]),
        );
        let entities = vec![app_module, service_run.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, service_run.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_call_on_the_element_of_a_declared_collection() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "item", "type": "Item", "kind": "element", "line": 2}
                ]
            }),
        );
        let save = declaration(
            "ent_item_save",
            "src/item.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("item.save", "save", 5)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.65);
    }

    #[tokio::test]
    async fn resolves_a_call_on_an_indexed_element_of_a_declared_collection() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "items[]", "type": "Item", "kind": "element", "line": 1}
                ]
            }),
        );
        let save = declaration(
            "ent_item_save",
            "src/item.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("items[].save", "save", 3)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.65);
    }

    #[tokio::test]
    async fn resolves_an_indexed_field_on_this() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "App.fields[]", "type": "Field", "kind": "element", "line": 2}
                ]
            }),
        );
        let save = declaration(
            "ent_field_save",
            "src/field.ts",
            "Field.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            false,
            json!([call_site("this.fields[].save", "save", 5)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.65);
    }

    #[tokio::test]
    async fn resolves_an_indexed_field_on_a_type_declared_in_another_module() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "repo", "type": "Repo", "kind": "construction", "line": 2}
                ]
            }),
        );
        let repo_module = module(
            "ent_repo_module",
            "src/repo.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items[]", "type": "Item", "kind": "element", "line": 3}
                ]
            }),
        );
        let repo_class = class_declaration("ent_repo_class", "src/repo.ts", "Repo", "Repo", true);
        let save = declaration(
            "ent_item_save",
            "src/item.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.items[].save", "save", 4)]),
        );
        let entities = vec![
            app_module,
            repo_module,
            repo_class,
            save.clone(),
            start.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.65);
    }

    #[tokio::test]
    async fn an_untyped_index_access_does_not_guess_a_unique_member() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": []}),
        );
        let save = declaration(
            "ent_item_save",
            "src/app.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("items[].save", "save", 3)]),
        );
        let entities = vec![app_module, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped index access must not fall through to a unique member name"
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_an_element_an_array_literal_constructs() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "items[]", "type": "Item", "kind": "construction", "line": 1}
                ]
            }),
        );
        let save = declaration(
            "ent_item_save",
            "src/item.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("items[].save", "save", 3)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_member_inherited_from_a_base_class() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "service", "type": "Service", "kind": "construction", "line": 2}
                ]
            }),
        );
        let service = subclass("ent_service", "src/service.ts", "Service", true, &["Base"]);
        let base = subclass("ent_base", "src/base.ts", "Base", true, &[]);
        let reset = declaration(
            "ent_base_reset",
            "src/base.ts",
            "Base.reset",
            "reset",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("service.reset", "reset", 5)]),
        );
        let entities = vec![app_module, service, base, reset.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, reset.id);
        assert_eq!(calls[0].payload["resolution"], "inherited_typed_member");
        assert_eq!(calls[0].confidence, 0.6);
    }

    #[tokio::test]
    async fn walks_a_chain_of_base_classes_within_its_bound() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "leaf", "type": "Leaf", "kind": "construction", "line": 2}
                ]
            }),
        );
        let leaf = subclass("ent_leaf", "src/leaf.ts", "Leaf", true, &["Middle"]);
        let middle = subclass("ent_middle", "src/middle.ts", "Middle", true, &["Base"]);
        let base = subclass("ent_base", "src/base.ts", "Base", true, &[]);
        let reset = declaration(
            "ent_base_reset",
            "src/base.ts",
            "Base.reset",
            "reset",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("leaf.reset", "reset", 5)]),
        );
        let entities = vec![app_module, leaf, middle, base, reset.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, reset.id);
        assert_eq!(calls[0].payload["resolution"], "inherited_typed_member");
    }

    #[tokio::test]
    async fn leaves_an_ambiguous_cyclic_or_absent_base_class_unresolved() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "service", "type": "Service", "kind": "construction", "line": 2}
                ]
            }),
        );
        let service = subclass("ent_service", "src/service.ts", "Service", true, &["Base"]);
        let first_base = subclass("ent_base_one", "src/one/base.ts", "Base", true, &[]);
        let second_base = subclass("ent_base_two", "src/two/base.ts", "Base", true, &[]);
        let reset = declaration(
            "ent_base_reset",
            "src/one/base.ts",
            "Base.reset",
            "reset",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("service.reset", "reset", 5)]),
        );
        let entities = vec![
            app_module,
            service,
            first_base,
            second_base,
            reset,
            start.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls)
        );

        // A cycle ends the walk instead of looping.
        let left = subclass("ent_left", "src/left.ts", "Left", true, &["Right"]);
        let right = subclass("ent_right", "src/right.ts", "Right", true, &["Left"]);
        let cyclic_module = module(
            "ent_cyclic_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "left", "type": "Left", "kind": "construction", "line": 2}
                ]
            }),
        );
        let cyclic_start = declaration(
            "ent_cyclic_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("left.missing", "missing", 5)]),
        );
        let entities = vec![cyclic_module, left, right, cyclic_start];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls)
        );
        // A base the snapshot never declares names nothing, even when a
        // declaration carries the qualified name it would own.
        let orphan_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "service", "type": "Service", "kind": "construction", "line": 2}
                ]
            }),
        );
        let orphaned_service = subclass(
            "ent_service",
            "src/service.ts",
            "Service",
            true,
            &["External"],
        );
        let orphaned = declaration(
            "ent_external_reset",
            "src/vendor.ts",
            "External.reset",
            "reset",
            true,
            json!([]),
        );
        let orphaned_start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("service.reset", "reset", 5)]),
        );
        let entities = vec![orphan_module, orphaned_service, orphaned, orphaned_start];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn resolves_deep_member_paths_through_typed_fields() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "App.repo", "type": "Repo", "kind": "construction", "line": 3},
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 9}
                ]
            }),
        );
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            true,
            json!([call_site("this.repo.items.map", "map", 6)]),
        );
        let entities = vec![app_module, items_map.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, items_map.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_member_paths_through_a_type_declared_in_another_module() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "App.repo", "type": "Repo", "kind": "annotation", "line": 3}
                ]
            }),
        );
        let repo_module = module(
            "ent_repo_module",
            "src/repo.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 5}
                ]
            }),
        );
        let repo_class = class_declaration("ent_repo_class", "src/repo.ts", "Repo", "Repo", true);
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            true,
            json!([call_site("this.repo.items.map", "map", 6)]),
        );
        let entities = vec![
            app_module,
            repo_module,
            repo_class,
            items_map.clone(),
            start.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, items_map.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn unexported_and_ambiguous_type_owners_stay_unresolved() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "App.repo", "type": "Repo", "kind": "annotation", "line": 3}
                ]
            }),
        );
        let repo_module = module(
            "ent_repo_module",
            "src/repo.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 5}
                ]
            }),
        );
        let other_module = module(
            "ent_other_module",
            "src/other.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "Other", "kind": "annotation", "line": 5}
                ]
            }),
        );
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            false,
            json!([]),
        );
        let unexported_owner = class_declaration(
            "ent_repo_class_unexported",
            "src/repo.ts",
            "Repo",
            "Repo",
            false,
        );
        let unexported_caller = declaration(
            "ent_app_start_unexported",
            "src/app.ts",
            "App.start",
            "start",
            true,
            json!([call_site("this.repo.items.map", "map", 6)]),
        );

        let entities = vec![
            app_module.clone(),
            repo_module.clone(),
            unexported_owner,
            items_map,
            unexported_caller,
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "a type the snapshot does not export cannot own the field types"
        );

        let first_owner =
            class_declaration("ent_repo_class_a", "src/repo.ts", "Repo", "Repo", true);
        let second_owner =
            class_declaration("ent_repo_class_b", "src/other.ts", "Repo", "Repo", true);
        let ambiguous_caller = declaration(
            "ent_app_start_ambiguous",
            "src/app.ts",
            "App.start",
            "start",
            true,
            json!([call_site("this.repo.items.map", "map", 6)]),
        );
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            false,
            json!([]),
        );
        let entities = vec![
            app_module.clone(),
            repo_module,
            other_module,
            first_owner,
            second_owner,
            items_map,
            ambiguous_caller,
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "a type declared in two modules has no proven owner"
        );
    }

    #[tokio::test]
    async fn resolves_a_receiver_through_a_factory_return_type() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 8}
                ],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3}
                ]
            }),
        );
        let factory = factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.items.map", "map", 6)]),
        );
        let entities = vec![app_module, factory, items_map.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, items_map.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_receiver_a_factory_constructs_and_returns() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let factory = returned_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_receiver_a_wrapper_forwards_to_a_factory() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let wrapper = forwarded_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("makeRepo"),
        );
        let inner = returned_factory_declaration(
            "ent_make_repo",
            "src/app.ts",
            "makeRepo",
            "makeRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, wrapper, inner, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn a_wrapper_inherits_the_annotation_the_inner_factory_declares() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let wrapper = forwarded_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("makeRepo"),
        );
        let inner = factory_declaration(
            "ent_make_repo",
            "src/app.ts",
            "makeRepo",
            "makeRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, wrapper, inner, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn a_second_returned_callee_hop_stays_unresolved() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let wrapper = forwarded_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("wrap"),
        );
        let middle = forwarded_factory_declaration(
            "ent_wrap",
            "src/app.ts",
            "wrap",
            "wrap",
            true,
            Some("makeRepo"),
        );
        let inner = returned_factory_declaration(
            "ent_make_repo",
            "src/app.ts",
            "makeRepo",
            "makeRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, wrapper, middle, inner, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a chain of wrappers is not the type the source states"
        );
    }

    #[tokio::test]
    async fn a_wrapper_through_a_promise_factory_stays_unresolved() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let wrapper = forwarded_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("makeRepo"),
        );
        let inner = promise_factory_declaration(
            "ent_make_repo",
            "src/app.ts",
            "makeRepo",
            "makeRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, wrapper, inner, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an inner promise is not the value an unawaited wrapper holds"
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_receiver_the_module_awaits_through_a_wrapper() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": true}
                ]
            }),
        );
        let mut wrapper = forwarded_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("makeRepo"),
        );
        wrapper.payload["async"] = json!(true);
        let inner = returned_factory_declaration(
            "ent_make_repo",
            "src/app.ts",
            "makeRepo",
            "makeRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, wrapper, inner, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_unawaited_async_wrapper_types_nothing() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let mut wrapper = forwarded_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("makeRepo"),
        );
        wrapper.payload["async"] = json!(true);
        let inner = returned_factory_declaration(
            "ent_make_repo",
            "src/app.ts",
            "makeRepo",
            "makeRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, wrapper, inner, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an unawaited async wrapper holds a promise, not the inner type"
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_receiver_a_factory_asserts() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let factory = asserted_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_receiver_an_initializer_asserts() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "repo", "type": "Repo", "kind": "annotation", "line": 2}
                ],
                "call_bindings": []
            }),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let factory = factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("createRepo().save", "save", 4)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
        assert_eq!(calls[0].payload["callee"], "createRepo().save");
    }

    #[tokio::test]
    async fn resolves_a_call_on_an_invoked_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let mut factory =
            factory_declaration("ent_create", "src/app.ts", "create", "create", true, None);
        factory.payload["returned_function_type"] = json!("Repo");
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("create()().save", "save", 4)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
        assert_eq!(calls[0].payload["callee"], "create()().save");
    }

    #[tokio::test]
    async fn an_uninvoked_function_factory_does_not_type_the_first_call() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let mut factory =
            factory_declaration("ent_create", "src/app.ts", "create", "create", true, None);
        factory.payload["returned_function_type"] = json!("Repo");
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("create().save", "save", 3)]),
        );
        let entities = vec![app_module, factory, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "create().save must not take the type create()() yields"
        );
    }

    #[tokio::test]
    async fn resolves_a_binding_initialized_by_an_invoked_factory() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "create", "line": 3, "awaited": false, "invoked": true}
                ]
            }),
        );
        let mut factory =
            factory_declaration("ent_create", "src/app.ts", "create", "create", true, None);
        factory.payload["returned_function_type"] = json!("Repo");
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 4)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn a_triple_call_hop_does_not_guess_a_unique_member() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let mut factory =
            factory_declaration("ent_create", "src/app.ts", "create", "create", true, None);
        factory.payload["returned_function_type"] = json!("Repo");
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("create()()().save", "save", 3)]),
        );
        let entities = vec![app_module, factory, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a third call hop must not fall through to a unique name"
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_construction_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("Repo{}.save", "save", 4)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
        assert_eq!(calls[0].payload["callee"], "Repo{}.save");
    }

    #[tokio::test]
    async fn resolves_a_call_on_an_awaited_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let factory = promise_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("createRepo()@.save", "save", 4)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.65);
    }

    #[tokio::test]
    async fn resolves_a_field_walk_on_a_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 8}
                ],
                "call_bindings": []
            }),
        );
        let factory = factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("createRepo().items.map", "map", 4)]),
        );
        let entities = vec![app_module, factory, items_map.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, items_map.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn an_untyped_factory_result_does_not_guess_a_unique_member() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let save = declaration(
            "ent_item_save",
            "src/app.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("createRepo().save", "save", 3)]),
        );
        let entities = vec![app_module, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped factory result must not fall through to a unique member name"
        );
    }

    #[tokio::test]
    async fn an_unawaited_async_factory_result_types_nothing() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let factory = promise_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("createRepo().save", "save", 4)]),
        );
        let entities = vec![app_module, factory, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an unawaited factory result holds the promise, not the awaited type"
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_member_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "svc", "type": "Service", "kind": "annotation", "line": 2}
                ],
                "call_bindings": []
            }),
        );
        let build = factory_declaration(
            "ent_service_build",
            "src/service.ts",
            "Service.build",
            "build",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("svc.build().save", "save", 4)]),
        );
        let entities = vec![app_module, build, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
        assert_eq!(calls[0].payload["callee"], "svc.build().save");
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_nested_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let factory = factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let get_store = factory_declaration(
            "ent_repo_get_store",
            "src/repo.ts",
            "Repo.getStore",
            "getStore",
            true,
            Some("Store"),
        );
        let save = declaration(
            "ent_store_save",
            "src/store.ts",
            "Store.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("createRepo().getStore().save", "save", 4)]),
        );
        let entities = vec![app_module, factory, get_store, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_call_on_this_member_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let factory = factory_declaration(
            "ent_app_factory",
            "src/app.ts",
            "App.factory",
            "factory",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            false,
            json!([call_site("this.factory().save", "save", 4)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_call_on_an_inherited_member_factory_result() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let app = subclass("ent_app_class", "src/app.ts", "App", true, &["Base"]);
        let base = class_declaration("ent_base_class", "src/base.ts", "Base", "Base", true);
        let build = factory_declaration(
            "ent_base_build",
            "src/base.ts",
            "Base.build",
            "build",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            false,
            json!([call_site("this.build().save", "save", 4)]),
        );
        let entities = vec![app_module, app, base, build, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn resolves_a_super_call_to_the_base_class_method() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let app = subclass("ent_app_class", "src/app.ts", "App", true, &["Base"]);
        let base = class_declaration("ent_base_class", "src/base.ts", "Base", "Base", true);
        let save = declaration(
            "ent_base_save",
            "src/base.ts",
            "Base.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            false,
            json!([call_site("super.save", "save", 4)]),
        );
        let entities = vec![app_module, app, base, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.8);
        assert_eq!(calls[0].payload["callee"], "super.save");
    }

    #[tokio::test]
    async fn a_super_call_without_a_base_class_does_not_guess_a_unique_member() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let app = class_declaration("ent_app_class", "src/app.ts", "App", "App", true);
        let save = declaration(
            "ent_item_save",
            "src/app.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "App.start",
            "start",
            false,
            json!([call_site("super.save", "save", 4)]),
        );
        let entities = vec![app_module, app, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "super without a declared base must not fall through to a unique member name"
        );
    }

    #[tokio::test]
    async fn resolves_a_member_through_a_type_alias() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "repo", "type": "Client", "kind": "annotation", "line": 3}
                ],
                "call_bindings": []
            }),
        );
        let client = type_alias_entity(
            "ent_client_alias",
            "src/app.ts",
            "Client",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 4)]),
        );
        let entities = vec![app_module, client, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
        assert_eq!(calls[0].payload["callee"], "repo.save");
    }

    #[tokio::test]
    async fn a_type_alias_without_a_named_type_does_not_resolve() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "repo", "type": "Client", "kind": "annotation", "line": 3}
                ],
                "call_bindings": []
            }),
        );
        let client = type_alias_entity("ent_client_alias", "src/app.ts", "Client", true, None);
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 4)]),
        );
        let entities = vec![app_module, client, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "a union or unnamed alias must not unfold to a unique member"
        );
    }

    #[tokio::test]
    async fn an_untyped_member_factory_does_not_guess_a_unique_member() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let save = declaration(
            "ent_item_save",
            "src/app.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("svc.build().save", "save", 3)]),
        );
        let entities = vec![app_module, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an untyped member factory must not fall through to a unique member name"
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_an_inline_receiver_assertion() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("Repo#.save", "save", 4)]),
        );
        let entities = vec![app_module, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.7);
        assert_eq!(calls[0].payload["callee"], "Repo#.save");
    }

    #[tokio::test]
    async fn resolves_a_field_walk_on_an_inline_receiver_assertion() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 8}
                ],
                "call_bindings": []
            }),
        );
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("Repo#.items.map", "map", 4)]),
        );
        let entities = vec![app_module, items_map.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, items_map.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.7);
    }

    #[tokio::test]
    async fn an_unresolved_inline_assertion_does_not_guess_a_unique_member() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [], "typed_bindings": [], "call_bindings": []}),
        );
        let save = declaration(
            "ent_item_save",
            "src/app.ts",
            "Item.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("Repo#.save", "save", 3)]),
        );
        let entities = vec![app_module, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls),
            "an assertion of a type with no such member must not fall through to a unique name"
        );
    }

    #[tokio::test]
    async fn a_declared_annotation_beats_the_type_a_factory_constructs() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        // The annotation says the factory returns an `Other`, so the `Repo` its
        // body constructs is not what the receiver holds.
        let mut factory = returned_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        factory.payload["return_type"] = json!("Other");
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, factory, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn resolves_a_member_of_a_structural_object_literal() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "config", "type": "config$object", "kind": "structural", "line": 2},
                    {"name": "config$object.logger", "type": "Logger", "kind": "construction", "line": 3}
                ]
            }),
        );
        let flush = declaration(
            "ent_logger_flush",
            "src/logger.ts",
            "Logger.flush",
            "flush",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("config.logger.flush", "flush", 6)]),
        );
        let entities = vec![app_module, flush.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, flush.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_path");
        assert_eq!(calls[0].confidence, 0.8);
    }

    #[tokio::test]
    async fn an_object_literal_that_constructs_nothing_types_nothing() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "plain", "type": "plain$object", "kind": "structural", "line": 2}
                ]
            }),
        );
        let flush = declaration(
            "ent_logger_flush",
            "src/logger.ts",
            "Logger.flush",
            "flush",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("plain.logger.flush", "flush", 6)]),
        );
        let entities = vec![app_module, flush, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn resolves_a_call_on_a_receiver_the_module_awaits() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3, "awaited": true}
                ]
            }),
        );
        let factory = promise_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.save", "save", 6)]),
        );
        let entities = vec![app_module, factory, save.clone(), start.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, start.id);
        assert_eq!(calls[0].to, save.id);
        assert_eq!(calls[0].payload["resolution"], "typed_receiver_member");
        assert_eq!(calls[0].confidence, 0.65);
    }

    #[tokio::test]
    async fn an_unawaited_promise_factory_types_nothing() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [],
                "call_bindings": [
                    {"name": "pending", "callee": "createRepo", "line": 3, "awaited": false}
                ]
            }),
        );
        let factory = promise_factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let save = declaration(
            "ent_repo_save",
            "src/repo.ts",
            "Repo.save",
            "save",
            true,
            json!([]),
        );
        let start = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("pending.save", "save", 6)]),
        );
        let entities = vec![app_module, factory, save, start];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.kind != RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn unproven_factory_return_types_stay_unresolved() {
        let items_map = declaration(
            "ent_store_map",
            "src/store.ts",
            "ItemStore.map",
            "map",
            false,
            json!([]),
        );

        // A factory without an annotated return type names nothing.
        let module_without_annotation = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 8}
                ],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3}
                ]
            }),
        );
        let factory = factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            None,
        );
        let caller = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.items.map", "map", 6)]),
        );
        let entities = vec![
            module_without_annotation,
            factory,
            items_map.clone(),
            caller,
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "a factory without a declared return type types nothing"
        );

        // The same name bound to two factories is ambiguous.
        let ambiguous_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 8}
                ],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3},
                    {"name": "repo", "callee": "buildRepo", "line": 4}
                ]
            }),
        );
        let first = factory_declaration(
            "ent_create_repo",
            "src/app.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let second = factory_declaration(
            "ent_build_repo",
            "src/app.ts",
            "buildRepo",
            "buildRepo",
            true,
            Some("Repo"),
        );
        let caller = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.items.map", "map", 6)]),
        );
        let entities = vec![ambiguous_module, first, second, items_map.clone(), caller];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "a name bound to two factories types nothing"
        );

        // A factory reachable only by a unique name elsewhere proves no import.
        let importing_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "Repo.items", "type": "ItemStore", "kind": "annotation", "line": 8}
                ],
                "call_bindings": [
                    {"name": "repo", "callee": "createRepo", "line": 3}
                ]
            }),
        );
        let remote_factory = factory_declaration(
            "ent_create_repo",
            "src/factory.ts",
            "createRepo",
            "createRepo",
            true,
            Some("Repo"),
        );
        let caller = declaration(
            "ent_app_start",
            "src/app.ts",
            "start",
            "start",
            false,
            json!([call_site("repo.items.map", "map", 6)]),
        );
        let entities = vec![importing_module, remote_factory, items_map, caller];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "a name-unique factory in an unimported module types nothing"
        );
    }

    #[tokio::test]
    async fn ambiguous_and_unprovable_receiver_paths_stay_unresolved() {
        let ambiguous = module(
            "ent_ambiguous_module",
            "src/ambiguous.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "service", "type": "Service", "kind": "construction", "line": 2},
                    {"name": "service", "type": "Other", "kind": "annotation", "line": 3}
                ]
            }),
        );
        let untyped = module(
            "ent_untyped_module",
            "src/untyped.ts",
            json!({"imports": [], "typed_bindings": []}),
        );
        let service_run = declaration(
            "ent_service_run",
            "src/service.ts",
            "Service.run",
            "run",
            false,
            json!([]),
        );
        let ambiguous_caller = declaration(
            "ent_ambiguous_caller",
            "src/ambiguous.ts",
            "start",
            "start",
            false,
            json!([call_site("service.run", "run", 5)]),
        );
        let untyped_caller = declaration(
            "ent_untyped_caller",
            "src/untyped.ts",
            "start",
            "start",
            false,
            json!([call_site("session.run", "run", 7)]),
        );
        let entities = vec![
            ambiguous,
            untyped,
            service_run,
            ambiguous_caller,
            untyped_caller,
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "an ambiguous or untyped receiver must not resolve"
        );
    }

    #[tokio::test]
    async fn cyclic_and_over_bound_receiver_paths_stay_unresolved() {
        let module_entity = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [],
                "typed_bindings": [
                    {"name": "App.first", "type": "Second", "kind": "annotation", "line": 3},
                    {"name": "Second.second", "type": "First", "kind": "annotation", "line": 4},
                    {"name": "First.first", "type": "Second", "kind": "annotation", "line": 5}
                ]
            }),
        );
        let run = declaration(
            "ent_target_run",
            "src/target.ts",
            "Second.run",
            "run",
            false,
            json!([]),
        );
        let cyclic = declaration(
            "ent_app_cyclic",
            "src/app.ts",
            "App.start",
            "start",
            true,
            json!([call_site("this.first.second.first.run", "run", 6)]),
        );
        let over_bound = declaration(
            "ent_app_over_bound",
            "src/app.ts",
            "App.deep",
            "deep",
            true,
            json!([call_site("this.a.b.c.d.run", "run", 7)]),
        );
        let entities = vec![module_entity, run, cyclic, over_bound];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation.kind == RelationKind::Calls)
                .count(),
            0,
            "a cyclic or over-bound receiver path must not resolve"
        );
    }

    #[tokio::test]
    async fn resolves_default_imports_to_anonymous_default_functions() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{"source": "./anon", "default_name": "request"}]
            }),
        );
        let anon_module = module(
            "ent_anon_module",
            "src/anon.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "default"
                }]
            }),
        );
        let handler = declaration(
            "ent_anon_default",
            "src/anon.ts",
            "default",
            "default",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("request", "request", 4)]),
        );
        let entities = vec![app_module, anon_module, handler.clone(), run.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, handler.id);
        assert_eq!(calls[0].payload["resolution"], "imported_default_export");
    }

    #[tokio::test]
    async fn resolves_default_imported_construction_of_anonymous_default_classes() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{"source": "./anon", "default_name": "Controller"}]
            }),
        );
        let anon_module = module(
            "ent_anon_module",
            "src/anon.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "default"
                }]
            }),
        );
        let controller = class_declaration(
            "ent_anon_default_class",
            "src/anon.ts",
            "default",
            "default",
            true,
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([construction_site("Controller", "Controller", 4)]),
        );
        let entities = vec![app_module, anon_module, controller.clone(), run.clone()];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, controller.id);
        assert_eq!(calls[0].payload["resolution"], "imported_default_export");
        assert_eq!(calls[0].payload["callee_kind"], "new");
    }

    #[tokio::test]
    async fn resolves_anonymous_defaults_forwarded_through_reexport_aliases() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{
                    "source": "./barrel",
                    "named": ["authenticate"],
                    "named_bindings": [{"imported": "signIn", "local": "authenticate"}]
                }]
            }),
        );
        let barrel_module = module(
            "ent_barrel_module",
            "src/barrel.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./anon",
                    "bindings": [{"imported": "default", "exported": "signIn"}]
                }]
            }),
        );
        let anon_module = module(
            "ent_anon_module",
            "src/anon.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "default"
                }]
            }),
        );
        let handler = declaration(
            "ent_anon_default",
            "src/anon.ts",
            "default",
            "default",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("authenticate", "authenticate", 5)]),
        );
        let entities = vec![
            app_module,
            barrel_module,
            anon_module,
            handler.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, handler.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn resolves_default_exports_forwarded_from_relative_imports() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{"source": "./auth", "default_name": "authenticate"}]
            }),
        );
        let auth_module = module(
            "ent_auth_module",
            "src/auth.ts",
            json!({
                "imports": [{
                    "source": "./service",
                    "named": ["login"],
                    "named_bindings": [{"imported": "login", "local": "login"}]
                }],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "login"
                }]
            }),
        );
        let service_module = module(
            "ent_service_module",
            "src/service.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_service_login",
            "src/service.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("authenticate", "authenticate", 6)]),
        );
        let entities = vec![
            app_module,
            auth_module,
            service_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn wildcard_reexports_do_not_expose_default_or_namespace_exports() {
        let default_app = module(
            "ent_default_app",
            "src/default-app.ts",
            json!({
                "imports": [{"source": "./index", "default_name": "login"}]
            }),
        );
        let namespace_app = module(
            "ent_namespace_app",
            "src/namespace-app.ts",
            json!({
                "imports": [{"source": "./namespace", "named": ["login"]}]
            }),
        );
        let anonymous_app = module(
            "ent_anonymous_app",
            "src/anonymous-app.ts",
            json!({"imports": [{"source": "./anonymous", "default_name": "anonymous"}]}),
        );
        let anonymous_module = module(
            "ent_anonymous_module",
            "src/anonymous.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": null
                }]
            }),
        );
        let index_module = module(
            "ent_index_module",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./auth", "bindings": [], "star": true},
                    {"source": "./other", "bindings": [], "star": true}
                ]
            }),
        );
        let other_module = module(
            "ent_wildcard_other_module",
            "src/other.ts",
            json!({"imports": []}),
        );
        let namespace_module = module(
            "ent_namespace_module",
            "src/namespace.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./auth", "bindings": [], "star": false}]
            }),
        );
        let auth_module = module(
            "ent_auth_module",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": null,
                    "bindings": [],
                    "default_export": true,
                    "default_name": "login"
                }]
            }),
        );
        let login = declaration(
            "ent_auth_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let anonymous_decoy = declaration(
            "ent_anonymous_decoy",
            "src/decoy.ts",
            "anonymous",
            "anonymous",
            true,
            json!([]),
        );
        let default_call = declaration(
            "ent_default_run",
            "src/default-app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 3)]),
        );
        let namespace_call = declaration(
            "ent_namespace_run",
            "src/namespace-app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 3)]),
        );
        let anonymous_call = declaration(
            "ent_anonymous_run",
            "src/anonymous-app.ts",
            "run",
            "run",
            false,
            json!([call_site("anonymous", "anonymous", 3)]),
        );
        let entities = vec![
            default_app,
            namespace_app,
            anonymous_app,
            index_module,
            other_module,
            namespace_module,
            anonymous_module,
            auth_module,
            login,
            anonymous_decoy,
            default_call,
            namespace_call,
            anonymous_call,
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn resolves_named_import_members_through_direct_namespace_reexports() {
        let app_module = module(
            "ent_namespace_app",
            "src/app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["client", "Session"],
                    "named_bindings": [
                        {"imported": "api", "local": "client"},
                        {"imported": "Session", "local": "Session"}
                    ]
                }]
            }),
        );
        let index_module = module(
            "ent_namespace_index",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./auth",
                    "bindings": [],
                    "star": false,
                    "namespace_name": "api"
                }]
            }),
        );
        let auth_module = module(
            "ent_namespace_auth",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./functions",
                    "bindings": [{"imported": "login", "exported": "login"}]
                }]
            }),
        );
        let functions_module = module(
            "ent_namespace_functions",
            "src/functions.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_namespace_login",
            "src/functions.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let refresh = declaration(
            "ent_namespace_refresh",
            "src/index.ts",
            "Session.refresh",
            "refresh",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_namespace_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([
                call_site("client.login", "login", 5),
                call_site("Session.refresh", "refresh", 6)
            ]),
        );
        let entities = vec![
            app_module,
            index_module,
            auth_module,
            functions_module,
            login.clone(),
            refresh.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().any(|relation| {
            relation.from == run.id
                && relation.to == login.id
                && relation.payload["resolution"] == "imported_reexport_chain"
        }));
        assert!(calls.iter().any(|relation| {
            relation.from == run.id
                && relation.to == refresh.id
                && relation.payload["resolution"] == "imported_symbol_name"
        }));
    }

    #[tokio::test]
    async fn resolves_namespace_members_through_forwarded_namespace_aliases() {
        let app_module = module(
            "ent_fwd_app",
            "src/fwd-app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["client"],
                    "named_bindings": [{"imported": "client", "local": "client"}]
                }]
            }),
        );
        let index_module = module(
            "ent_fwd_index",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./auth",
                    "bindings": [{"imported": "api", "exported": "client"}],
                    "star": false
                }]
            }),
        );
        let auth_module = module(
            "ent_fwd_auth",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./functions",
                    "bindings": [],
                    "star": false,
                    "namespace_name": "api"
                }]
            }),
        );
        let functions_module = module(
            "ent_fwd_functions",
            "src/functions.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_fwd_login",
            "src/functions.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_fwd_run",
            "src/fwd-app.ts",
            "run",
            "run",
            false,
            json!([call_site("client.login", "login", 5)]),
        );
        let entities = vec![
            app_module,
            index_module,
            auth_module,
            functions_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn resolves_namespace_members_through_same_name_forwarded_aliases() {
        let app_module = module(
            "ent_same_app",
            "src/same-app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["api"],
                    "named_bindings": [{"imported": "api", "local": "api"}]
                }]
            }),
        );
        let index_module = module(
            "ent_same_index",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./auth",
                    "bindings": [{"imported": "api", "exported": "api"}],
                    "star": false
                }]
            }),
        );
        let auth_module = module(
            "ent_same_auth",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./functions",
                    "bindings": [],
                    "star": false,
                    "namespace_name": "api"
                }]
            }),
        );
        let functions_module = module(
            "ent_same_functions",
            "src/functions.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_same_login",
            "src/functions.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_same_run",
            "src/same-app.ts",
            "run",
            "run",
            false,
            json!([call_site("api.login", "login", 5)]),
        );
        let entities = vec![
            app_module,
            index_module,
            auth_module,
            functions_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, run.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(calls[0].payload["resolution"], "imported_reexport_chain");
    }

    #[tokio::test]
    async fn ambiguous_forwarded_namespace_aliases_stay_unresolved() {
        let app_module = module(
            "ent_amb_app",
            "src/amb-app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["client"],
                    "named_bindings": [{"imported": "client", "local": "client"}]
                }]
            }),
        );
        let index_module = module(
            "ent_amb_index",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [
                    {
                        "source": "./auth",
                        "bindings": [{"imported": "api", "exported": "client"}],
                        "star": false
                    },
                    {
                        "source": "./other",
                        "bindings": [{"imported": "other", "exported": "client"}],
                        "star": false
                    }
                ]
            }),
        );
        let auth_module = module(
            "ent_amb_auth",
            "src/auth.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./functions",
                    "bindings": [],
                    "star": false,
                    "namespace_name": "api"
                }]
            }),
        );
        let other_module = module("ent_amb_other", "src/other.ts", json!({"imports": []}));
        let functions_module = module(
            "ent_amb_functions",
            "src/functions.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_amb_login",
            "src/functions.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_amb_run",
            "src/amb-app.ts",
            "run",
            "run",
            false,
            json!([call_site("client.login", "login", 5)]),
        );
        let entities = vec![
            app_module,
            index_module,
            auth_module,
            other_module,
            functions_module,
            login.clone(),
            run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls),
            "an ambiguous forwarded namespace alias must stay unresolved"
        );
    }

    #[tokio::test]
    async fn namespace_import_members_require_unique_direct_mappings_and_one_segment() {
        let app_module = module(
            "ent_namespace_guard_app",
            "src/guard-app.ts",
            json!({
                "imports": [{
                    "source": "./index",
                    "named": ["absent", "sourceMissing", "ambiguous", "nested"],
                    "named_bindings": [
                        {"imported": "missing", "local": "absent"},
                        {"imported": "unresolved", "local": "sourceMissing"},
                        {"imported": "duplicate", "local": "ambiguous"},
                        {"imported": "api", "local": "nested"}
                    ]
                }]
            }),
        );
        let index_module = module(
            "ent_namespace_guard_index",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [
                    {
                        "source": "./missing-source",
                        "bindings": [],
                        "star": false,
                        "namespace_name": "unresolved"
                    },
                    {
                        "source": "./auth",
                        "bindings": [],
                        "star": false,
                        "namespace_name": "duplicate"
                    },
                    {
                        "source": "./other",
                        "bindings": [],
                        "star": false,
                        "namespace_name": "duplicate"
                    },
                    {
                        "source": "./auth",
                        "bindings": [],
                        "star": false,
                        "namespace_name": "api"
                    }
                ]
            }),
        );
        let auth_module = module(
            "ent_namespace_guard_auth",
            "src/auth.ts",
            json!({"imports": []}),
        );
        let other_module = module(
            "ent_namespace_guard_other",
            "src/other.ts",
            json!({"imports": []}),
        );
        let index_login = declaration(
            "ent_namespace_guard_index_login",
            "src/index.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let login = declaration(
            "ent_namespace_guard_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_namespace_guard_run",
            "src/guard-app.ts",
            "run",
            "run",
            false,
            json!([
                call_site("absent.login", "login", 4),
                call_site("sourceMissing.login", "login", 5),
                call_site("ambiguous.login", "login", 6),
                call_site("nested.auth.login", "login", 7)
            ]),
        );
        let entities = vec![
            app_module,
            index_module,
            auth_module,
            other_module,
            index_login,
            login,
            run,
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn namespace_import_member_cycles_and_over_limit_chains_stay_unresolved() {
        let app_module = module(
            "ent_namespace_depth_app",
            "src/depth-app.ts",
            json!({
                "imports": [
                    {
                        "source": "./cycle-index",
                        "named": ["cycle"],
                        "named_bindings": [{"imported": "cycle", "local": "cycle"}]
                    },
                    {
                        "source": "./deep-index",
                        "named": ["deep"],
                        "named_bindings": [{"imported": "deep", "local": "deep"}]
                    }
                ]
            }),
        );
        let cycle_index = module(
            "ent_namespace_cycle_index",
            "src/cycle-index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./cycle-a",
                    "bindings": [],
                    "star": false,
                    "namespace_name": "cycle"
                }]
            }),
        );
        let cycle_a = module(
            "ent_namespace_cycle_a",
            "src/cycle-a.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./cycle-index", "bindings": [], "star": true}]
            }),
        );
        let deep_index = module(
            "ent_namespace_deep_index",
            "src/deep-index.ts",
            json!({
                "imports": [],
                "exports": [{
                    "source": "./chain-0",
                    "bindings": [],
                    "star": false,
                    "namespace_name": "deep"
                }]
            }),
        );
        let mut entities = vec![app_module, cycle_index, cycle_a, deep_index];
        for index in 0..=33 {
            let exports = if index < 33 {
                json!([{
                    "source": format!("./chain-{}", index + 1),
                    "bindings": [],
                    "star": true
                }])
            } else {
                json!([])
            };
            entities.push(module(
                &format!("ent_namespace_chain_{index}"),
                &format!("src/chain-{index}.ts"),
                json!({"imports": [], "exports": exports}),
            ));
        }
        let login = declaration(
            "ent_namespace_chain_login",
            "src/chain-33.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_namespace_depth_run",
            "src/depth-app.ts",
            "run",
            "run",
            false,
            json!([
                call_site("cycle.login", "login", 4),
                call_site("deep.login", "login", 5)
            ]),
        );
        entities.extend([login, run]);

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn resolves_unique_targets_across_multiple_wildcard_sources() {
        let unique_app = module(
            "ent_multi_unique_app",
            "src/unique-app.ts",
            json!({
                "imports": [{
                    "source": "./unique-index",
                    "named": ["uniqueLogin"],
                    "named_bindings": [{"imported": "login", "local": "uniqueLogin"}]
                }]
            }),
        );
        let shared_app = module(
            "ent_multi_shared_app",
            "src/shared-app.ts",
            json!({
                "imports": [{
                    "source": "./shared-index",
                    "named": ["sharedLogin"],
                    "named_bindings": [{"imported": "login", "local": "sharedLogin"}]
                }]
            }),
        );
        let unique_index = module(
            "ent_multi_unique_index",
            "src/unique-index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./auth", "bindings": [], "star": true},
                    {"source": "./other", "bindings": [], "star": true}
                ]
            }),
        );
        let shared_index = module(
            "ent_multi_shared_index",
            "src/shared-index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./left", "bindings": [], "star": true},
                    {"source": "./right", "bindings": [], "star": true}
                ]
            }),
        );
        let left = module(
            "ent_multi_left",
            "src/left.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./auth", "bindings": [], "star": true}]
            }),
        );
        let right = module(
            "ent_multi_right",
            "src/right.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./auth", "bindings": [], "star": true}]
            }),
        );
        let auth = module("ent_multi_auth", "src/auth.ts", json!({"imports": []}));
        let other = module("ent_multi_other", "src/other.ts", json!({"imports": []}));
        let login = declaration(
            "ent_multi_auth_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let unique_run = declaration(
            "ent_multi_unique_run",
            "src/unique-app.ts",
            "run",
            "run",
            false,
            json!([call_site("uniqueLogin", "uniqueLogin", 4)]),
        );
        let shared_run = declaration(
            "ent_multi_shared_run",
            "src/shared-app.ts",
            "run",
            "run",
            false,
            json!([call_site("sharedLogin", "sharedLogin", 4)]),
        );
        let entities = vec![
            unique_app,
            shared_app,
            unique_index,
            shared_index,
            left,
            right,
            auth,
            other,
            login.clone(),
            unique_run.clone(),
            shared_run.clone(),
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(
            calls
                .iter()
                .any(|relation| relation.from == unique_run.id && relation.to == login.id)
        );
        assert!(
            calls
                .iter()
                .any(|relation| relation.from == shared_run.id && relation.to == login.id)
        );
        assert!(
            calls
                .iter()
                .all(|relation| relation.payload["resolution"] == "imported_reexport_chain")
        );
    }

    #[tokio::test]
    async fn unresolved_branch_blocks_multiple_wildcard_resolution() {
        let app_module = module(
            "ent_multi_unresolved_app",
            "src/unresolved-app.ts",
            json!({"imports": [{"source": "./index", "named": ["login"]}]}),
        );
        let index_module = module(
            "ent_multi_unresolved_index",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./auth", "bindings": [], "star": true},
                    {"source": "./missing", "bindings": [], "star": true}
                ]
            }),
        );
        let auth_module = module(
            "ent_multi_unresolved_auth",
            "src/auth.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_multi_unresolved_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_multi_unresolved_run",
            "src/unresolved-app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 4)]),
        );
        let entities = vec![app_module, index_module, auth_module, login, run];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn multiple_wildcard_branches_respect_the_resolution_step_bound() {
        let app_module = module(
            "ent_multi_budget_app",
            "src/budget-app.ts",
            json!({"imports": [{"source": "./index", "named": ["login"]}]}),
        );
        let exports = (0..MAX_REEXPORT_RESOLUTION_STEPS)
            .map(|_| json!({"source": "./auth", "bindings": [], "star": true}))
            .collect::<Vec<_>>();
        let index_module = module(
            "ent_multi_budget_index",
            "src/index.ts",
            json!({"imports": [], "exports": exports}),
        );
        let auth_module = module(
            "ent_multi_budget_auth",
            "src/auth.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_multi_budget_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_multi_budget_run",
            "src/budget-app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 4)]),
        );
        let entities = vec![app_module, index_module, auth_module, login, run];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn ambiguous_multiple_wildcard_reexports_remain_unresolved() {
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({"imports": [{"source": "./index", "named": ["login"]}]}),
        );
        let index_module = module(
            "ent_index_module",
            "src/index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./auth", "bindings": [], "star": true},
                    {"source": "./other", "bindings": [], "star": true}
                ]
            }),
        );
        let auth_module = module("ent_auth_module", "src/auth.ts", json!({"imports": []}));
        let other_module = module("ent_other_module", "src/other.ts", json!({"imports": []}));
        let login = declaration(
            "ent_auth_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let other_login = declaration(
            "ent_other_login",
            "src/other.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 4)]),
        );
        let entities = vec![
            app_module,
            index_module,
            auth_module,
            other_module,
            login,
            other_login,
            run,
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn wildcard_cycles_and_chains_over_the_depth_bound_remain_unresolved() {
        let cycle_app = module(
            "ent_cycle_app",
            "src/cycle-app.ts",
            json!({"imports": [{"source": "./cycle-index", "named": ["login"]}]}),
        );
        let deep_app = module(
            "ent_deep_app",
            "src/deep-app.ts",
            json!({"imports": [{"source": "./deep-index", "named": ["login"]}]}),
        );
        let cycle_index = module(
            "ent_cycle_index",
            "src/cycle-index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./auth", "bindings": [], "star": true},
                    {"source": "./cycle-a", "bindings": [], "star": true}
                ]
            }),
        );
        let deep_index = module(
            "ent_deep_index",
            "src/deep-index.ts",
            json!({
                "imports": [],
                "exports": [
                    {"source": "./auth", "bindings": [], "star": true},
                    {"source": "./chain-0", "bindings": [], "star": true}
                ]
            }),
        );
        let auth_module = module("ent_cycle_auth", "src/auth.ts", json!({"imports": []}));
        let cycle_a = module(
            "ent_cycle_a",
            "src/cycle-a.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./cycle-b", "bindings": [], "star": true}]
            }),
        );
        let cycle_b = module(
            "ent_cycle_b",
            "src/cycle-b.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./cycle-a", "bindings": [], "star": true}]
            }),
        );
        let mut entities = vec![
            cycle_app,
            deep_app,
            cycle_index,
            deep_index,
            auth_module,
            cycle_a,
            cycle_b,
        ];
        for index in 0..=33 {
            let exports = if index < 33 {
                json!([{
                    "source": format!("./chain-{}", index + 1),
                    "bindings": [],
                    "star": true
                }])
            } else {
                json!([])
            };
            entities.push(module(
                &format!("ent_chain_{index}"),
                &format!("src/chain-{index}.ts"),
                json!({"imports": [], "exports": exports}),
            ));
        }
        let cycle_run = declaration(
            "ent_cycle_run",
            "src/cycle-app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 3)]),
        );
        let deep_run = declaration(
            "ent_deep_run",
            "src/deep-app.ts",
            "run",
            "run",
            false,
            json!([call_site("login", "login", 3)]),
        );
        let auth_login = declaration(
            "ent_cycle_auth_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let over_limit_login = declaration(
            "ent_chain_login",
            "src/chain-33.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        entities.extend([cycle_run, deep_run, auth_login, over_limit_login]);

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn unresolved_imported_exports_do_not_fall_through_to_a_global_guess() {
        let barrel_module = module(
            "ent_barrel_module",
            "src/barrel.ts",
            json!({
                "imports": [],
                "exports": [
                    {
                        "source": "./missing",
                        "bindings": [{"imported": "dispatch", "exported": "dispatch"}]
                    },
                    {"source": "./auth", "bindings": [], "star": true}
                ]
            }),
        );
        let app_module = module(
            "ent_app_module",
            "src/app.ts",
            json!({
                "imports": [{"source": "./barrel", "named": ["dispatch"]}]
            }),
        );
        let missing_star_module = module(
            "ent_missing_star_module",
            "src/missing-star.ts",
            json!({
                "imports": [],
                "exports": [{"source": "./not-found", "bindings": [], "star": true}]
            }),
        );
        let missing_star_app = module(
            "ent_missing_star_app",
            "src/missing-star-app.ts",
            json!({
                "imports": [{"source": "./missing-star", "named": ["dispatch"]}]
            }),
        );
        let auth_module = module("ent_auth_module", "src/auth.ts", json!({"imports": []}));
        let auth_dispatch = declaration(
            "ent_auth_dispatch",
            "src/auth.ts",
            "dispatch",
            "dispatch",
            true,
            json!([]),
        );
        let decoy = declaration(
            "ent_decoy_dispatch",
            "src/decoy.ts",
            "dispatch",
            "dispatch",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_app_run",
            "src/app.ts",
            "run",
            "run",
            false,
            json!([call_site("dispatch", "dispatch", 4)]),
        );
        let missing_star_run = declaration(
            "ent_missing_star_run",
            "src/missing-star-app.ts",
            "run",
            "run",
            false,
            json!([call_site("dispatch", "dispatch", 4)]),
        );
        let entities = vec![
            barrel_module,
            app_module,
            missing_star_module,
            missing_star_app,
            auth_module,
            auth_dispatch,
            decoy,
            run,
            missing_star_run,
        ];

        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            !relations
                .iter()
                .any(|relation| relation.kind == RelationKind::Calls)
        );
    }

    #[tokio::test]
    async fn resolves_this_member_and_unique_exported_names() {
        let module_entity = module("ent_module", "src/session.ts", json!({"imports": []}));
        let store = declaration(
            "ent_store",
            "src/session.ts",
            "Session.store",
            "store",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_run",
            "src/session.ts",
            "Session.run",
            "run",
            true,
            json!([
                call_site("this.store", "store", 9),
                call_site("orphan", "orphan", 10)
            ]),
        );
        let orphan = declaration(
            "ent_orphan",
            "src/other.ts",
            "orphan",
            "orphan",
            true,
            json!([]),
        );
        let ambiguous_one = declaration(
            "ent_ambiguous_one",
            "src/one.ts",
            "shared",
            "shared",
            true,
            json!([]),
        );
        let ambiguous_two = declaration(
            "ent_ambiguous_two",
            "src/two.ts",
            "shared",
            "shared",
            true,
            json!([]),
        );
        let ambiguous_caller = declaration(
            "ent_ambiguous_caller",
            "src/other.ts",
            "callShared",
            "callShared",
            false,
            json!([call_site("shared", "shared", 3)]),
        );

        let entities = vec![
            module_entity,
            store.clone(),
            run.clone(),
            orphan.clone(),
            ambiguous_one,
            ambiguous_two,
            ambiguous_caller,
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().any(|relation| {
            relation.to == store.id
                && relation.payload["resolution"] == "same_module_this_method"
                && relation.from == run.id
        }));
        assert!(calls.iter().any(|relation| {
            relation.to == orphan.id && relation.payload["resolution"] == "unique_exported_name"
        }));
    }

    #[tokio::test]
    async fn emits_tested_by_for_test_file_calls_into_production_code() {
        let module_entity = module("ent_module", "src/auth.ts", json!({"imports": []}));
        let test_module = module(
            "ent_test_module",
            "src/auth.test.ts",
            json!({"imports": []}),
        );
        let login = declaration(
            "ent_login",
            "src/auth.ts",
            "login",
            "login",
            true,
            json!([]),
        );
        let test = declaration(
            "ent_test_login",
            "src/auth.test.ts",
            "coversLogin",
            "coversLogin",
            false,
            json!([call_site("login", "login", 3)]),
        );

        let entities = vec![module_entity, test_module, login.clone(), test.clone()];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        let tested_by = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::TestedBy)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, test.id);
        assert_eq!(calls[0].to, login.id);
        assert_eq!(tested_by.len(), 1);
        assert_eq!(tested_by[0].from, login.id);
        assert_eq!(tested_by[0].to, test.id);
        assert_eq!(tested_by[0].payload["reason"], "js_ts_test_call");
    }

    #[tokio::test]
    async fn bridges_callback_arguments_to_their_declarations() {
        let filters = module("ent_filters", "src/filters.ts", json!({"imports": []}));
        let pipeline = module(
            "ent_pipeline",
            "src/pipeline.ts",
            json!({
                "imports": [{
                    "source": "./filters",
                    "named_bindings": [{"imported": "filterActive", "local": "filterActive"}]
                }]
            }),
        );
        let filter_active = declaration(
            "ent_filter_active",
            "src/filters.ts",
            "filterActive",
            "filterActive",
            true,
            json!([]),
        );
        let handle_ok = declaration(
            "ent_handle_ok",
            "src/pipeline.ts",
            "handleOk",
            "handleOk",
            false,
            json!([]),
        );
        let on_change = declaration(
            "ent_on_change",
            "src/pipeline.ts",
            "Pipeline.onChange",
            "onChange",
            true,
            json!([]),
        );
        let run = declaration(
            "ent_run",
            "src/pipeline.ts",
            "Pipeline.run",
            "run",
            true,
            json!([
                {
                    "callee": "items.map",
                    "name": "map",
                    "kind": "call",
                    "line": 3,
                    "column": 3,
                    "callback_arguments": ["filterActive"]
                },
                {
                    "callee": "promise.then",
                    "name": "then",
                    "kind": "call",
                    "line": 4,
                    "column": 3,
                    "callback_arguments": ["handleOk"]
                },
                {
                    "callee": "emitter.addEventListener",
                    "name": "addEventListener",
                    "kind": "call",
                    "line": 5,
                    "column": 3,
                    "callback_arguments": ["this.onChange"]
                }
            ]),
        );

        let entities = vec![
            filters,
            pipeline,
            filter_active.clone(),
            handle_ok.clone(),
            on_change.clone(),
            run.clone(),
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        assert!(calls.iter().all(|relation| {
            relation.payload["reason"] == "js_ts_callback_bridge"
                && relation.payload["callee_kind"] == "callback"
                && relation.status == RelationStatus::Inferred
        }));
        assert!(calls.iter().any(|relation| {
            relation.from == run.id
                && relation.to == filter_active.id
                && relation.payload["callee"] == "filterActive"
                && relation.payload["host_callee"] == "items.map"
                && relation.payload["resolution"] == "imported_exact_name"
                && relation.payload["line"].as_u64() == Some(3)
        }));
        assert!(calls.iter().any(|relation| {
            relation.from == run.id
                && relation.to == handle_ok.id
                && relation.payload["callee"] == "handleOk"
                && relation.payload["host_callee"] == "promise.then"
                && relation.payload["resolution"] == "same_module_exact_name"
        }));
        assert!(calls.iter().any(|relation| {
            relation.from == run.id
                && relation.to == on_change.id
                && relation.payload["callee"] == "this.onChange"
                && relation.payload["host_callee"] == "emitter.addEventListener"
                && relation.payload["resolution"] == "same_module_this_method"
        }));
    }

    #[tokio::test]
    async fn bridges_test_callbacks_into_tested_by_edges() {
        let session = module("ent_session", "src/session.ts", json!({"imports": []}));
        let test_module = module(
            "ent_test_module",
            "src/auth.test.ts",
            json!({
                "imports": [{
                    "source": "./session",
                    "named_bindings": [{"imported": "checkSession", "local": "checkSession"}]
                }]
            }),
        );
        let check_session = declaration(
            "ent_check_session",
            "src/session.ts",
            "checkSession",
            "checkSession",
            true,
            json!([]),
        );
        let verify = declaration(
            "ent_verify",
            "src/auth.test.ts",
            "verifySession",
            "verifySession",
            false,
            json!([
                {
                    "callee": "promise.then",
                    "name": "then",
                    "kind": "call",
                    "line": 2,
                    "column": 3,
                    "callback_arguments": ["checkSession"]
                }
            ]),
        );

        let entities = vec![session, test_module, check_session.clone(), verify.clone()];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        let calls = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::Calls)
            .collect::<Vec<_>>();
        let tested_by = relations
            .iter()
            .filter(|relation| relation.kind == RelationKind::TestedBy)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, verify.id);
        assert_eq!(calls[0].to, check_session.id);
        assert_eq!(calls[0].payload["reason"], "js_ts_callback_bridge");
        assert_eq!(tested_by.len(), 1);
        assert_eq!(tested_by[0].from, check_session.id);
        assert_eq!(tested_by[0].to, verify.id);
        assert_eq!(tested_by[0].payload["reason"], "js_ts_test_call");
    }

    #[tokio::test]
    async fn leaves_unresolvable_and_ambiguous_callbacks_unresolved() {
        let first = module("ent_first", "src/first.ts", json!({"imports": []}));
        let second = module("ent_second", "src/second.ts", json!({"imports": []}));
        let caller_module = module("ent_caller_module", "src/caller.ts", json!({"imports": []}));
        let duplicate_one = declaration(
            "ent_duplicate_one",
            "src/first.ts",
            "duplicate",
            "duplicate",
            true,
            json!([]),
        );
        let duplicate_two = declaration(
            "ent_duplicate_two",
            "src/second.ts",
            "duplicate",
            "duplicate",
            true,
            json!([]),
        );
        let caller = declaration(
            "ent_caller",
            "src/caller.ts",
            "caller",
            "caller",
            true,
            json!([
                {
                    "callee": "items.map",
                    "name": "map",
                    "kind": "call",
                    "line": 2,
                    "column": 3,
                    "callback_arguments": ["missingHandler", "duplicate"]
                }
            ]),
        );

        let entities = vec![
            first,
            second,
            caller_module,
            duplicate_one,
            duplicate_two,
            caller,
        ];
        let relations = JsTsCallLinker
            .link(LinkInput {
                snapshot: SnapshotId("snap_test".to_string()),
                entities: Arc::new(entities.clone()),
                facts: Arc::new(Vec::new()),
                affected: AffectedSubset::from_extracted(entities, Vec::new()),
            })
            .await
            .unwrap();

        assert!(
            relations
                .iter()
                .all(|relation| relation.payload["reason"] != "js_ts_callback_bridge"),
            "unresolvable and ambiguous callbacks must stay unresolved"
        );
    }

    fn call_site(callee: &str, name: &str, line: u64) -> serde_json::Value {
        json!({
            "callee": callee,
            "name": name,
            "kind": "call",
            "line": line,
            "column": 3,
        })
    }

    fn module(id: &str, path: &str, payload: serde_json::Value) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("{MODULE_PREFIX}{path}")),
            kind: EntityKind::Module,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload,
        }
    }

    fn declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
        calls: serde_json::Value,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("{SYMBOL_PREFIX}{path}#{qualified}")),
            kind: EntityKind::Function,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(2),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({
                "qualified_name": qualified,
                "exported": exported,
                "calls": calls,
            }),
        }
    }

    fn subclass(id: &str, path: &str, name: &str, exported: bool, extends: &[&str]) -> Entity {
        let mut entity = class_declaration(id, path, name, name, exported);
        entity.payload["extends"] = json!(extends);
        entity
    }

    fn type_alias_entity(
        id: &str,
        path: &str,
        name: &str,
        exported: bool,
        aliased_type: Option<&str>,
    ) -> Entity {
        let mut payload = json!({
            "symbol_kind": "type_alias",
            "qualified_name": name,
            "exported": exported,
            "calls": json!([]),
        });
        if let Some(aliased) = aliased_type {
            payload["aliased_type"] = json!(aliased);
        }
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("{SYMBOL_PREFIX}{path}#{name}")),
            kind: EntityKind::Symbol,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(2),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload,
        }
    }

    fn class_declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("{SYMBOL_PREFIX}{path}#{qualified}")),
            kind: EntityKind::Class,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(2),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({
                "symbol_kind": "class",
                "qualified_name": qualified,
                "exported": exported,
                "calls": json!([]),
            }),
        }
    }

    fn factory_declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
        return_type: Option<&str>,
    ) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("{SYMBOL_PREFIX}{path}#{qualified}")),
            kind: EntityKind::Function,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(2),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: path.to_string(),
            }],
            payload: json!({
                "qualified_name": qualified,
                "exported": exported,
                "calls": json!([]),
                "return_type": return_type,
            }),
        }
    }

    /// A factory that returns a promise: the declared return type names nothing
    /// a receiver can hold, and the awaited type names what `await` yields.
    fn promise_factory_declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
        awaited_return_type: Option<&str>,
    ) -> Entity {
        let mut entity = factory_declaration(id, path, qualified, name, exported, None);
        entity.payload["awaited_return_type"] = json!(awaited_return_type);
        entity
    }

    /// A factory that declares no return type but constructs the value it
    /// returns.
    fn returned_factory_declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
        returned_type: Option<&str>,
    ) -> Entity {
        let mut entity = factory_declaration(id, path, qualified, name, exported, None);
        entity.payload["returned_type"] = json!(returned_type);
        entity
    }

    /// A wrapper that constructs nothing and returns a call to another factory.
    fn forwarded_factory_declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
        returned_callee: Option<&str>,
    ) -> Entity {
        let mut entity = factory_declaration(id, path, qualified, name, exported, None);
        entity.payload["returned_callee"] = json!(returned_callee);
        entity
    }

    /// A factory that asserts the type of the value it returns.
    fn asserted_factory_declaration(
        id: &str,
        path: &str,
        qualified: &str,
        name: &str,
        exported: bool,
        asserted_type: Option<&str>,
    ) -> Entity {
        let mut entity = factory_declaration(id, path, qualified, name, exported, None);
        entity.payload["asserted_type"] = json!(asserted_type);
        entity
    }

    fn construction_site(callee: &str, name: &str, line: u64) -> serde_json::Value {
        json!({
            "callee": callee,
            "name": name,
            "kind": "new",
            "line": line,
            "column": 3,
        })
    }
}
