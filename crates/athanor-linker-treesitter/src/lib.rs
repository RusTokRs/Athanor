//! Cross-file resolution for the tree-sitter language packs.
//!
//! `athanor-extractor-treesitter` records one module entity per source file and one entity per
//! declaration, together with the imports and statically named call sites it can read from the
//! syntax. This linker turns those canonical payloads into relations: `Imports` between module
//! entities, `Calls` from a calling declaration to the declaration it reaches, and `TestedBy` from
//! a tested declaration to the test that calls it. Resolution stays pack-driven — the language id
//! on each stable key selects the module layout, and the recorded import `style` selects dotted or
//! package-path semantics — so a new language receives cross-file resolution without a new crate.
//!
//! Every relation is evidence-backed: it records the resolution reason, the confidence of the
//! step that matched it, and both source files as ownership. Name-based steps require exactly one
//! candidate, so ambiguity stays unresolved instead of producing a guess.

mod index;
mod resolve;

pub use resolve::{LanguageCoverage, resolution_coverage};

use std::collections::HashSet;

use async_trait::async_trait;
use athanor_core::{CoreResult, InvalidationPolicy, InvalidationScope, LinkInput, Linker};
use athanor_domain::{
    Entity, EntityId, Evidence, EvidenceStatus, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId,
};
use athanor_extractor_basic::stable_hash;
use serde_json::{Value, json};

use index::{Index, is_test_file, module_key, symbol_key};
use resolve::{call_sites, import_payloads, import_targets, resolve_call};

/// Relations emitted for one snapshot are capped at this bound.
pub const MAX_RELATIONS: usize = 10_000;
/// Modules one import statement may resolve to before the bound is applied.
const MAX_IMPORT_TARGETS: usize = 32;

const SAME_UNIT_OWN_CONFIDENCE: f32 = 0.9;
const SAME_UNIT_EXACT_CONFIDENCE: f32 = 0.9;
const IMPORTED_EXACT_CONFIDENCE: f32 = 0.85;
const IMPORTED_NAME_CONFIDENCE: f32 = 0.7;
const UNIQUE_NAME_CONFIDENCE: f32 = 0.5;
const TESTED_BY_CONFIDENCE: f32 = 0.6;
const EVIDENCE_EXTRACTOR: &str = "treesitter-linker";

/// Resolves cross-file references recorded by the tree-sitter packs.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeSitterLinker;

#[async_trait]
impl Linker for TreeSitterLinker {
    fn name(&self) -> &str {
        "treesitter"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy {
            on_change: InvalidationScope::DependencyClosure,
            on_add: InvalidationScope::DependencyClosure,
            on_remove: InvalidationScope::DependencyClosure,
        }
    }

    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>> {
        let index = Index::build(input.entities.as_slice());
        let affected = input
            .affected
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<HashSet<EntityId>>();
        Ok(resolve_relations(&index, &input.snapshot, Some(&affected)))
    }
}

/// Resolves every cross-file reference in `entities` for one snapshot.
pub fn resolve_entities(entities: &[Entity], snapshot: &SnapshotId) -> Vec<Relation> {
    let index = Index::build(entities);
    resolve_relations(&index, snapshot, None)
}

fn resolve_relations(
    index: &Index<'_>,
    snapshot: &SnapshotId,
    affected: Option<&HashSet<EntityId>>,
) -> Vec<Relation> {
    let mut relations = Vec::new();
    let mut relation_ids = HashSet::new();

    for module in index.modules() {
        if module_key(&module.stable_key.0).is_none() {
            continue;
        }
        for import in import_payloads(module) {
            let mut resolution = import_targets(index, module, import);
            resolution.targets.truncate(MAX_IMPORT_TARGETS);
            let specifier = import
                .get("module")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let line = import.get("line").and_then(Value::as_u64);
            for target in resolution.targets {
                if !affected_by(affected, module, target) {
                    continue;
                }
                push_unique(
                    &mut relations,
                    &mut relation_ids,
                    import_relation(
                        snapshot,
                        module,
                        target,
                        specifier,
                        resolution.reason,
                        resolution.status,
                        resolution.confidence,
                        line,
                    ),
                );
            }
        }
    }

    for declaration in index.declarations() {
        let Some((language, path, _)) = symbol_key(&declaration.stable_key.0) else {
            continue;
        };
        let Some(module) = index.module(language, path) else {
            continue;
        };
        for call in call_sites(declaration) {
            let Some(callee) = call.get("callee").and_then(Value::as_str) else {
                continue;
            };
            let Some((target, resolution, confidence)) =
                resolve_call(index, declaration, module, callee)
            else {
                continue;
            };
            if target.id == declaration.id || !affected_by(affected, declaration, target) {
                continue;
            }
            let line = call.get("line").and_then(Value::as_u64);
            let column = call.get("column").and_then(Value::as_u64);
            let call_kind = call.get("kind").and_then(Value::as_str).unwrap_or("call");
            push_unique(
                &mut relations,
                &mut relation_ids,
                call_relation(
                    snapshot,
                    declaration,
                    target,
                    callee,
                    call_kind,
                    resolution,
                    confidence,
                    line,
                    column,
                ),
            );
            let target_is_test = declaration_path(target).is_some_and(|target_path| {
                is_test_file(language, target_path)
            });
            if is_test_file(language, path) && !target_is_test {
                push_unique(
                    &mut relations,
                    &mut relation_ids,
                    tested_by_relation(snapshot, target, declaration, callee, line),
                );
            }
        }
    }

    relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    relations.dedup_by(|left, right| left.id == right.id);
    relations.truncate(MAX_RELATIONS);
    relations
}

fn affected_by(affected: Option<&HashSet<EntityId>>, from: &Entity, to: &Entity) -> bool {
    affected.is_none_or(|ids| ids.contains(&from.id) || ids.contains(&to.id))
}

fn declaration_path(entity: &Entity) -> Option<&str> {
    symbol_key(&entity.stable_key.0).map(|(_, path, _)| path)
}

#[allow(clippy::too_many_arguments)]
fn import_relation(
    snapshot: &SnapshotId,
    source: &Entity,
    target: &Entity,
    specifier: &str,
    resolution: &str,
    status: RelationStatus,
    confidence: f32,
    line: Option<u64>,
) -> Relation {
    let id_material = format!(
        "treesitter_import\0{}\0{}\0{specifier}",
        source.stable_key.0, target.stable_key.0
    );
    Relation {
        id: RelationId(format!(
            "rel_treesitter_import_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Imports,
        from: source.id.clone(),
        to: target.id.clone(),
        status,
        confidence,
        evidence: reference_evidence(source, target, line, confidence, status),
        ownership: combined_ownership(source, target),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": source.stable_key.0,
            "to": target.stable_key.0,
            "specifier": specifier,
            "resolution": resolution,
            "line": line,
            "reason": "treesitter_import",
        }),
    }
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
        "treesitter_call\0{}\0{}\0{callee}",
        caller.stable_key.0, target.stable_key.0
    );
    Relation {
        id: RelationId(format!(
            "rel_treesitter_call_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::Calls,
        from: caller.id.clone(),
        to: target.id.clone(),
        status: RelationStatus::Inferred,
        confidence,
        evidence: reference_evidence(
            caller,
            target,
            line,
            confidence,
            RelationStatus::Inferred,
        ),
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
            "reason": "treesitter_static_call",
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
        "treesitter_tested_by\0{}\0{}\0{callee}",
        tested.stable_key.0, test.stable_key.0
    );
    Relation {
        id: RelationId(format!(
            "rel_treesitter_tested_by_{:016x}",
            stable_hash(id_material.as_bytes())
        )),
        kind: RelationKind::TestedBy,
        from: tested.id.clone(),
        to: test.id.clone(),
        status: RelationStatus::Inferred,
        confidence: TESTED_BY_CONFIDENCE,
        evidence: reference_evidence(
            test,
            tested,
            line,
            TESTED_BY_CONFIDENCE,
            RelationStatus::Inferred,
        ),
        ownership: combined_ownership(tested, test),
        snapshot: snapshot.clone(),
        payload: json!({
            "from": tested.stable_key.0,
            "to": test.stable_key.0,
            "callee": callee,
            "line": line,
            "reason": "treesitter_test_call",
        }),
    }
}

fn reference_evidence(
    source: &Entity,
    target: &Entity,
    line: Option<u64>,
    confidence: f32,
    status: RelationStatus,
) -> Vec<Evidence> {
    let evidence_status = if status == RelationStatus::Verified {
        EvidenceStatus::Verified
    } else {
        EvidenceStatus::Inferred
    };
    let mut evidence = Vec::new();
    if let Some(location) = &source.source {
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: line.and_then(|line| u32::try_from(line).ok()),
            line_end: line.and_then(|line| u32::try_from(line).ok()),
            extractor: Some(EVIDENCE_EXTRACTOR.to_string()),
            commit_hash: None,
            confidence,
            status: evidence_status,
        });
    }
    if let Some(location) = &target.source {
        evidence.push(Evidence {
            source_file: Some(location.path.clone()),
            line_start: location.line_start,
            line_end: location.line_end,
            extractor: Some(EVIDENCE_EXTRACTOR.to_string()),
            commit_hash: None,
            confidence,
            status: evidence_status,
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
