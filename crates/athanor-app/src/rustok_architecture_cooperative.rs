use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Result;
use athanor_core::{CanonicalSnapshot, OperationContext, OperationContextCancellation};
use athanor_domain::{DiagnosticStatus, Entity, EntityId, EntityKind, Relation};
use serde::Serialize;

use crate::rustok_architecture::{
    RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA, RustokArchitectureContext,
    RustokArchitectureContextOptions, RustokArchitectureContract, RustokArchitectureDiagnostic,
    RustokArchitectureEvidence, RustokArchitectureInteraction, RustokArchitectureModule,
    RustokArchitectureOmitted, RustokArchitectureResolution, RustokArchitectureTest,
};

const RUSTOK_ARCHITECTURE_POLL_INTERVAL: usize = 64;

pub(crate) fn build_rustok_architecture_context_with_operation_context(
    snapshot: &CanonicalSnapshot,
    options: &RustokArchitectureContextOptions,
    context_entities: &[EntityId],
    operation: &OperationContext,
) -> Result<RustokArchitectureContext> {
    build_with_checkpoint(snapshot, options, context_entities, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

fn build_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    options: &RustokArchitectureContextOptions,
    context_entities: &[EntityId],
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokArchitectureContext> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_ARCHITECTURE_POLL_INTERVAL)?;
    let mut context_ids = HashSet::with_capacity(context_entities.len());
    for id in context_entities {
        poller.step()?;
        context_ids.insert(id.clone());
    }

    let mut entities_by_id = HashMap::with_capacity(snapshot.entities.len());
    for entity in &snapshot.entities {
        poller.step()?;
        entities_by_id.insert(entity.id.clone(), entity);
    }

    let mut module_reasons = BTreeMap::<String, BTreeSet<String>>::new();
    let mut module_scores = BTreeMap::<String, usize>::new();
    for entity in &snapshot.entities {
        poller.step()?;
        let is_context = context_ids.contains(&entity.id);
        for (module, reason) in entity_modules(entity) {
            poller.step()?;
            let score = if is_context { 4 } else { 1 };
            *module_scores.entry(module.clone()).or_default() += score;
            module_reasons.entry(module).or_default().insert(reason);
        }
    }
    if let Some(module) = options.module.as_deref() {
        *module_scores.entry(module.to_string()).or_default() += 100;
        module_reasons
            .entry(module.to_string())
            .or_default()
            .insert("explicit module selector".to_string());
    }

    poller.checkpoint()?;
    let mut ranked_modules = module_scores.into_iter().collect::<Vec<_>>();
    ranked_modules.sort_by_key(|(module, score)| (Reverse(*score), module.clone()));
    poller.checkpoint()?;
    let total_modules = ranked_modules.len();
    let selected_modules = ranked_modules
        .iter()
        .take(options.max_modules)
        .map(|(module, _)| module.clone())
        .collect::<BTreeSet<_>>();
    let modules = ranked_modules
        .into_iter()
        .take(options.max_modules)
        .map(|(slug, score)| RustokArchitectureModule {
            reasons: module_reasons
                .remove(&slug)
                .unwrap_or_default()
                .into_iter()
                .collect(),
            slug,
            score,
        })
        .collect::<Vec<_>>();

    let mut contracts = Vec::new();
    let mut interactions = Vec::new();
    let mut relevant_ids = HashSet::new();
    for entity in &snapshot.entities {
        poller.step()?;
        if let Some(contract) = architecture_contract(entity, &selected_modules) {
            contracts.push(contract);
        }
        if let Some(interaction) = architecture_interaction(entity, &selected_modules) {
            interactions.push(interaction);
        }
        if context_ids.contains(&entity.id)
            || entity_modules(entity)
                .iter()
                .any(|(module, _)| selected_modules.contains(module))
        {
            relevant_ids.insert(entity.id.clone());
        }
    }
    contracts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    interactions.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let total_contracts = contracts.len();
    let total_interactions = interactions.len();
    contracts.truncate(options.max_contracts);
    interactions.truncate(options.max_interactions);

    let mut test_ids = HashSet::<EntityId>::new();
    for entity in &snapshot.entities {
        poller.step()?;
        if matches!(entity.kind, EntityKind::TestCase) && relevant_ids.contains(&entity.id) {
            test_ids.insert(entity.id.clone());
        }
    }
    for relation in &snapshot.relations {
        poller.step()?;
        if relevant_ids.contains(&relation.from)
            && matches!(
                relation.kind,
                athanor_domain::RelationKind::TestedBy
                    | athanor_domain::RelationKind::CoveredByTest
            )
        {
            test_ids.insert(relation.to.clone());
        }
    }
    let mut tests = Vec::new();
    for id in test_ids {
        poller.step()?;
        if let Some(entity) = entities_by_id.get(&id).copied() {
            tests.push(RustokArchitectureTest {
                stable_key: entity.stable_key.0.clone(),
                name: entity.name.clone(),
                source: entity_source(entity),
            });
        }
    }
    tests.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    tests.truncate(16);

    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        if diagnostic.status == DiagnosticStatus::Open
            && diagnostic
                .entities
                .iter()
                .any(|entity| relevant_ids.contains(entity))
        {
            diagnostics.push(RustokArchitectureDiagnostic {
                kind: serialized_name(&diagnostic.kind),
                severity: serialized_name(&diagnostic.severity),
                message: diagnostic.message.clone(),
                source: diagnostic.evidence.iter().find_map(evidence_source),
            });
            if diagnostics.len() == 16 {
                break;
            }
        }
    }

    let mut evidence = BTreeSet::new();
    for id in &relevant_ids {
        poller.step()?;
        if let Some(entity) = entities_by_id.get(id)
            && let Some(source) = entity_source(entity)
        {
            evidence.insert(RustokArchitectureEvidence {
                stable_key: entity.stable_key.0.clone(),
                source,
            });
        }
    }
    for relation in &snapshot.relations {
        poller.step()?;
        if relevant_ids.contains(&relation.from) || relevant_ids.contains(&relation.to) {
            let stable_key = relation_endpoints(relation, &entities_by_id);
            for source in relation.evidence.iter().filter_map(evidence_source) {
                poller.step()?;
                evidence.insert(RustokArchitectureEvidence {
                    stable_key: stable_key.clone(),
                    source,
                });
            }
        }
    }
    let mut evidence = evidence.into_iter().collect::<Vec<_>>();
    let total_evidence = evidence.len();
    evidence.truncate(options.max_evidence);

    poller.finish()?;
    let resolution = architecture_resolution(&modules, options.module.as_deref());
    let guidance = architecture_guidance(&resolution, &contracts, &interactions, &tests);
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(RustokArchitectureContext {
        schema: RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA.to_string(),
        snapshot: snapshot_id,
        intent: options.intent.clone(),
        resolution,
        modules,
        contracts,
        interactions,
        tests,
        diagnostics,
        evidence,
        guidance,
        omitted: RustokArchitectureOmitted {
            modules: total_modules.saturating_sub(options.max_modules),
            contracts: total_contracts.saturating_sub(options.max_contracts),
            interactions: total_interactions.saturating_sub(options.max_interactions),
            evidence: total_evidence.saturating_sub(options.max_evidence),
        },
    })
}

struct CheckpointPoller<F> {
    checkpoint: F,
    interval: usize,
    remaining: usize,
}

impl<F> CheckpointPoller<F>
where
    F: FnMut() -> Result<()>,
{
    fn new(mut checkpoint: F, interval: usize) -> Result<Self> {
        checkpoint()?;
        let interval = interval.max(1);
        Ok(Self {
            checkpoint,
            interval,
            remaining: interval,
        })
    }

    fn step(&mut self) -> Result<()> {
        self.remaining -= 1;
        if self.remaining == 0 {
            self.checkpoint()?;
        }
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<()> {
        (self.checkpoint)()?;
        self.remaining = self.interval;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.checkpoint()
    }
}

fn entity_modules(entity: &Entity) -> Vec<(String, String)> {
    let mut modules = BTreeMap::<String, String>::new();
    if let Some(module) = module_from_stable_key(&entity.stable_key.0) {
        modules.insert(module, "canonical architecture entity".to_string());
    }
    if let Some(source) = &entity.source
        && let Some(module) = module_from_source(&source.path)
    {
        modules.insert(module, "source ownership".to_string());
    }
    for field in ["module", "consumer", "provider"] {
        if let Some(module) = entity
            .payload
            .get(field)
            .and_then(serde_json::Value::as_str)
        {
            modules.insert(module.to_string(), format!("{field} declaration"));
        }
    }
    modules.into_iter().collect()
}

fn module_from_stable_key(stable_key: &str) -> Option<String> {
    for prefix in [
        "fba_module://",
        "fba_contract://",
        "fba_port://",
        "fba_operation://",
        "fba_profile://",
        "ffa_surface://",
        "ffa_layer://",
    ] {
        if let Some(rest) = stable_key.strip_prefix(prefix) {
            return rest.split('/').next().map(str::to_string);
        }
    }
    None
}

fn module_from_source(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let rest = normalized.split("crates/rustok-").nth(1)?;
    rest.split('/').next().map(str::to_string)
}

fn architecture_contract(
    entity: &Entity,
    selected_modules: &BTreeSet<String>,
) -> Option<RustokArchitectureContract> {
    let stable_key = entity.stable_key.0.as_str();
    let is_contract = stable_key.starts_with("fba_contract://")
        || stable_key.starts_with("fba_port://")
        || stable_key.starts_with("fba_operation://")
        || stable_key.starts_with("ffa_surface://")
        || matches!(entity.kind, EntityKind::ApiEndpoint | EntityKind::ApiSchema);
    if !is_contract {
        return None;
    }
    let module = module_from_stable_key(stable_key).or_else(|| {
        entity
            .source
            .as_ref()
            .and_then(|source| module_from_source(&source.path))
    })?;
    selected_modules
        .contains(&module)
        .then(|| RustokArchitectureContract {
            stable_key: stable_key.to_string(),
            kind: serialized_name(&entity.kind),
            module,
            source: entity_source(entity),
        })
}

fn architecture_interaction(
    entity: &Entity,
    selected_modules: &BTreeSet<String>,
) -> Option<RustokArchitectureInteraction> {
    let rest = entity.stable_key.0.strip_prefix("fba_dependency://")?;
    let mut parts = rest.split('/');
    let consumer = parts.next()?.to_string();
    let provider = parts.next()?.to_string();
    let profile = parts.next()?.to_string();
    if parts.next().is_some()
        || (!selected_modules.contains(&consumer) && !selected_modules.contains(&provider))
    {
        return None;
    }
    Some(RustokArchitectureInteraction {
        consumer,
        provider,
        profile,
        stable_key: entity.stable_key.0.clone(),
        source: entity_source(entity),
    })
}

fn relation_endpoints(
    relation: &Relation,
    entities: &HashMap<EntityId, &Entity>,
) -> String {
    let from = entities
        .get(&relation.from)
        .map_or(relation.from.0.as_str(), |entity| {
            entity.stable_key.0.as_str()
        });
    let to = entities
        .get(&relation.to)
        .map_or(relation.to.0.as_str(), |entity| {
            entity.stable_key.0.as_str()
        });
    format!("{from} -> {to}")
}

fn architecture_resolution(
    modules: &[RustokArchitectureModule],
    explicit_module: Option<&str>,
) -> RustokArchitectureResolution {
    let candidates = modules
        .iter()
        .map(|module| module.slug.clone())
        .collect::<Vec<_>>();
    if let Some(module) = explicit_module {
        return RustokArchitectureResolution {
            status: "resolved".to_string(),
            primary_module: Some(module.to_string()),
            candidates,
            summary: format!("Architecture context is anchored to explicit module `{module}`."),
        };
    }
    match modules {
        [] => RustokArchitectureResolution {
            status: "unresolved".to_string(),
            primary_module: None,
            candidates,
            summary: "No RusTok module ownership candidate was found in the indexed graph."
                .to_string(),
        },
        [first, rest @ ..] if rest.first().is_none_or(|second| first.score >= second.score * 2) => {
            RustokArchitectureResolution {
                status: "resolved".to_string(),
                primary_module: Some(first.slug.clone()),
                candidates,
                summary: format!(
                    "`{}` is the strongest ownership candidate from indexed architecture evidence.",
                    first.slug
                ),
            }
        }
        _ => RustokArchitectureResolution {
            status: "ambiguous".to_string(),
            primary_module: None,
            candidates,
            summary: "Multiple modules have comparable evidence; resolve ownership before implementing new domain behavior."
                .to_string(),
        },
    }
}

fn architecture_guidance(
    resolution: &RustokArchitectureResolution,
    contracts: &[RustokArchitectureContract],
    interactions: &[RustokArchitectureInteraction],
    tests: &[RustokArchitectureTest],
) -> Vec<String> {
    let mut guidance = vec![resolution.summary.clone()];
    if contracts.is_empty() {
        guidance.push(
            "No public contract was found; inspect the candidate owner before adding a new local implementation."
                .to_string(),
        );
    } else {
        guidance.push(format!(
            "Reuse the indexed public contract first: {}.",
            contracts[0].stable_key
        ));
    }
    if interactions.is_empty() {
        guidance.push(
            "No declared consumer/provider edge was found; declare the interaction before cross-module wiring."
                .to_string(),
        );
    }
    if tests.is_empty() {
        guidance.push(
            "No linked integration test evidence was found for this context; add or link a boundary scenario."
                .to_string(),
        );
    }
    guidance
}

fn entity_source(entity: &Entity) -> Option<String> {
    entity.source.as_ref().map(|source| {
        source.line_start.map_or_else(
            || source.path.clone(),
            |line| format!("{}:{line}", source.path),
        )
    })
}

fn evidence_source(evidence: &athanor_domain::Evidence) -> Option<String> {
    evidence.source_file.as_ref().map(|source| {
        evidence
            .line_start
            .map_or_else(|| source.clone(), |line| format!("{source}:{line}"))
    })
}

fn serialized_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use athanor_core::CoreError;
    use athanor_domain::{EntityKind, SnapshotId, StableKey};
    use serde_json::json;

    use super::*;
    use crate::rustok_architecture::build_rustok_architecture_context;

    #[test]
    fn cooperative_builder_matches_legacy_output() {
        let product = entity("product", "fba_module://product", EntityKind::Other("rustok_fba_module".to_string()));
        let contract = entity("contract", "fba_contract://product/catalog.v1", EntityKind::Other("rustok_fba_contract".to_string()));
        let dependency = entity("dependency", "fba_dependency://search/product/native", EntityKind::Dependency);
        let test = entity("test", "test://catalog", EntityKind::TestCase);
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![product.clone(), contract, dependency, test.clone()],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let options = RustokArchitectureContextOptions::bounded(
            std::path::PathBuf::from("."),
            "catalog".to_string(),
            Some("product".to_string()),
        );
        let operation = OperationContext::new("rustok-architecture-parity");

        let cooperative = build_rustok_architecture_context_with_operation_context(
            &snapshot,
            &options,
            &[product.id.clone(), test.id.clone()],
            &operation,
        )
        .unwrap();
        let legacy = build_rustok_architecture_context(
            &snapshot,
            &options,
            &[product.id.clone(), test.id.clone()],
        );

        assert_eq!(cooperative, legacy);
    }

    #[test]
    fn cancellation_after_multiple_batches_stops_builder() {
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_large".to_string())),
            entities: (0..600)
                .map(|index| {
                    entity(
                        &format!("entity-{index}"),
                        &format!("fba_contract://module-{}/contract-{index}", index % 8),
                        EntityKind::Other("rustok_fba_contract".to_string()),
                    )
                })
                .collect(),
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let options = RustokArchitectureContextOptions::bounded(
            std::path::PathBuf::from("."),
            "large architecture".to_string(),
            None,
        );
        let operation = OperationContext::new("rustok-architecture-mid-cancel");
        let cancellation = operation.cancellation_handle().unwrap();
        let mut checkpoints = 0;

        let error = build_with_checkpoint(&snapshot, &options, &[], || {
            checkpoints += 1;
            if checkpoints == 3 {
                cancellation.cancel();
            }
            operation.check_active().map_err(anyhow::Error::new)
        })
        .expect_err("cancelled architecture build must stop after bounded batches");

        assert!(checkpoints >= 3);
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: id.to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }
}
