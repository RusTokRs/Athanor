//! Call-graph linking for JavaScript and TypeScript declarations.
//!
//! The JS/TS extractor records statically named call sites per declaration.
//! This linker resolves those call sites to canonical declarations inside the
//! same snapshot and emits `Calls` relations, plus `TestedBy` relations when a
//! test-file declaration calls one from production code.
//!
//! Every relation carries the matched callee, its resolution reason, and the
//! call-site line, so a query can always be traced back to the source. Dynamic
//! call shapes (computed members, call results, reflective dispatch) stay
//! unresolved instead of being guessed, and name-based fallbacks require a
//! unique candidate so ambiguity never silently becomes a fact.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use athanor_core::{CoreResult, InvalidationPolicy, InvalidationScope, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityKind, Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use crate::paths::{normalize_path, resolve_local_import};

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
        for entity in input.entities.iter() {
            if is_js_ts_module(entity) {
                if let Some(path) = entity_path(entity) {
                    modules.insert(path, entity);
                }
            } else if is_js_ts_declaration(entity) {
                declarations.push(entity);
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
        for candidates in declarations_by_path.values_mut() {
            candidates.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        }
        for candidates in declarations_by_name.values_mut() {
            candidates.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        }

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
                let Some((target, resolution, confidence)) = resolve_call(
                    callee,
                    caller,
                    caller_module,
                    &modules,
                    &declarations_by_path,
                    &declarations_by_name,
                ) else {
                    continue;
                };
                if target.id == caller.id {
                    continue;
                }
                if !affected_ids.contains(&caller.id) && !affected_ids.contains(&target.id) {
                    continue;
                }
                let line = call.get("line").and_then(Value::as_u64);
                let column = call.get("column").and_then(Value::as_u64);
                let call_kind = call.get("kind").and_then(Value::as_str).unwrap_or("call");
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

        relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        relations.dedup_by(|left, right| left.id == right.id);
        relations.truncate(MAX_CALL_RELATIONS);
        Ok(relations)
    }
}

fn is_js_ts_module(entity: &Entity) -> bool {
    entity.kind == EntityKind::Module && entity.stable_key.0.starts_with(MODULE_PREFIX)
}

fn is_js_ts_declaration(entity: &Entity) -> bool {
    entity.kind == EntityKind::Function && entity.stable_key.0.starts_with(SYMBOL_PREFIX)
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
fn resolve_call<'a>(
    callee: &str,
    caller: &'a Entity,
    caller_module: &'a Entity,
    modules: &HashMap<String, &'a Entity>,
    declarations_by_path: &HashMap<String, Vec<&'a Entity>>,
    declarations_by_name: &HashMap<String, Vec<&'a Entity>>,
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
            return Some((target, "same_module_this_method", SAME_MODULE_THIS_CONFIDENCE));
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
    if let Some((head, tail)) = head_and_tail(callee)
        && let Some(binding) = import_binding(caller_module, head)
        && let Some(target_module) = resolve_local_import(caller_module, &binding, modules)
        && let Some(target_path) = entity_path(target_module)
        && let Some(candidates) = declarations_by_path.get(&target_path)
    {
        let wanted = tail.unwrap_or(head);
        if let Some(target) = candidates
            .iter()
            .copied()
            .find(|candidate| qualified_name(candidate) == Some(wanted))
        {
            return Some((target, "imported_exact_name", IMPORTED_EXACT_CONFIDENCE));
        }
        if let Some(target) = candidates
            .iter()
            .copied()
            .find(|candidate| candidate.name == wanted)
        {
            return Some((target, "imported_symbol_name", IMPORTED_NAME_CONFIDENCE));
        }
    }

    let name = callee.rsplit('.').next().unwrap_or(callee);
    if callee.contains('.') {
        // 4. Member call with an unknown receiver: accept a unique member name
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
        // 5. Bare call without a local or imported target: accept a unique
        //    exported declaration elsewhere in the snapshot.
        if let Some(target) = unique_match(declarations_by_name.get(name), |candidate| {
            candidate.id != caller.id && is_exported(candidate)
        }) {
            return Some((target, "unique_exported_name", UNIQUE_NAME_CONFIDENCE));
        }
    }

    None
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

/// Returns the module specifier of the import statement binding `local`.
fn import_binding(module: &Entity, local: &str) -> Option<String> {
    let imports = module.payload.get("imports").and_then(Value::as_array)?;
    for import in imports {
        let Some(specifier) = import.get("source").and_then(Value::as_str) else {
            continue;
        };
        let default_matches = import.get("default_name").and_then(Value::as_str) == Some(local);
        let namespace_matches = import.get("namespace_name").and_then(Value::as_str) == Some(local);
        let named_matches = import
            .get("named")
            .and_then(Value::as_array)
            .is_some_and(|named| named.iter().any(|entry| entry.as_str() == Some(local)));
        if default_matches || namespace_matches || named_matches {
            return Some(specifier.to_string());
        }
    }
    None
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
            client, session_module, auth, login, session, refresh, run.clone(),
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
}
