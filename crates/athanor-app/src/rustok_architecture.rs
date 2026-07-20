use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use athanor_core::CanonicalSnapshot;
use athanor_domain::{DiagnosticStatus, Entity, EntityId, EntityKind, Relation};
use serde::{Deserialize, Serialize};

pub const RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA: &str = "athanor.rustok_architecture_context.v1";

const DEFAULT_MAX_MODULES: usize = 6;
const DEFAULT_MAX_CONTRACTS: usize = 16;
const DEFAULT_MAX_INTERACTIONS: usize = 16;
const DEFAULT_MAX_EVIDENCE: usize = 20;

#[derive(Debug, Clone)]
pub struct RustokArchitectureContextOptions {
    pub root: PathBuf,
    pub intent: String,
    pub module: Option<String>,
    pub max_modules: usize,
    pub max_contracts: usize,
    pub max_interactions: usize,
    pub max_evidence: usize,
}

impl RustokArchitectureContextOptions {
    pub fn bounded(root: PathBuf, intent: String, module: Option<String>) -> Self {
        Self {
            root,
            intent,
            module,
            max_modules: DEFAULT_MAX_MODULES,
            max_contracts: DEFAULT_MAX_CONTRACTS,
            max_interactions: DEFAULT_MAX_INTERACTIONS,
            max_evidence: DEFAULT_MAX_EVIDENCE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RustokArchitectureContext {
    pub schema: String,
    pub snapshot: String,
    pub intent: String,
    pub resolution: RustokArchitectureResolution,
    pub modules: Vec<RustokArchitectureModule>,
    pub contracts: Vec<RustokArchitectureContract>,
    pub interactions: Vec<RustokArchitectureInteraction>,
    pub tests: Vec<RustokArchitectureTest>,
    pub diagnostics: Vec<RustokArchitectureDiagnostic>,
    pub evidence: Vec<RustokArchitectureEvidence>,
    pub guidance: Vec<String>,
    pub omitted: RustokArchitectureOmitted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureResolution {
    pub status: String,
    pub primary_module: Option<String>,
    pub candidates: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureModule {
    pub slug: String,
    pub score: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureContract {
    pub stable_key: String,
    pub kind: String,
    pub module: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureInteraction {
    pub consumer: String,
    pub provider: String,
    pub profile: String,
    pub stable_key: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureTest {
    pub stable_key: String,
    pub name: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureDiagnostic {
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct RustokArchitectureEvidence {
    pub stable_key: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RustokArchitectureOmitted {
    pub modules: usize,
    pub contracts: usize,
    pub interactions: usize,
    pub evidence: usize,
}

pub fn build_rustok_architecture_context(
    snapshot: &CanonicalSnapshot,
    options: &RustokArchitectureContextOptions,
    context_entities: &[EntityId],
) -> RustokArchitectureContext {
    let context_ids = context_entities.iter().cloned().collect::<HashSet<_>>();
    let entities_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), entity))
        .collect::<HashMap<_, _>>();
    let mut module_reasons = BTreeMap::<String, BTreeSet<String>>::new();
    let mut module_scores = BTreeMap::<String, usize>::new();

    for entity in &snapshot.entities {
        let is_context = context_ids.contains(&entity.id);
        for (module, reason) in entity_modules(entity) {
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

    let mut ranked_modules = module_scores.into_iter().collect::<Vec<_>>();
    ranked_modules.sort_by_key(|(module, score)| (Reverse(*score), module.clone()));
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

    let mut contracts = snapshot
        .entities
        .iter()
        .filter_map(|entity| architecture_contract(entity, &selected_modules))
        .collect::<Vec<_>>();
    contracts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let total_contracts = contracts.len();
    contracts.truncate(options.max_contracts);

    let mut interactions = snapshot
        .entities
        .iter()
        .filter_map(|entity| architecture_interaction(entity, &selected_modules))
        .collect::<Vec<_>>();
    interactions.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let total_interactions = interactions.len();
    interactions.truncate(options.max_interactions);

    let relevant_ids = relevant_entity_ids(snapshot, &selected_modules, &context_ids);
    let tests = architecture_tests(snapshot, &entities_by_id, &relevant_ids);
    let diagnostics = architecture_diagnostics(snapshot, &relevant_ids);
    let mut evidence = architecture_evidence(snapshot, &relevant_ids);
    let total_evidence = evidence.len();
    evidence.truncate(options.max_evidence);

    let resolution = architecture_resolution(&modules, options.module.as_deref());
    let guidance = architecture_guidance(&resolution, &contracts, &interactions, &tests);
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    RustokArchitectureContext {
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

fn relevant_entity_ids(
    snapshot: &CanonicalSnapshot,
    selected_modules: &BTreeSet<String>,
    context_ids: &HashSet<EntityId>,
) -> HashSet<EntityId> {
    snapshot
        .entities
        .iter()
        .filter(|entity| {
            context_ids.contains(&entity.id)
                || entity_modules(entity)
                    .iter()
                    .any(|(module, _)| selected_modules.contains(module))
        })
        .map(|entity| entity.id.clone())
        .collect()
}

fn architecture_tests(
    snapshot: &CanonicalSnapshot,
    entities_by_id: &HashMap<EntityId, &Entity>,
    relevant_ids: &HashSet<EntityId>,
) -> Vec<RustokArchitectureTest> {
    let mut test_ids = HashSet::<EntityId>::new();
    for entity in &snapshot.entities {
        if matches!(entity.kind, EntityKind::TestCase) && relevant_ids.contains(&entity.id) {
            test_ids.insert(entity.id.clone());
        }
    }
    for relation in &snapshot.relations {
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
    let mut tests = test_ids
        .into_iter()
        .filter_map(|id| entities_by_id.get(&id).copied())
        .map(|entity| RustokArchitectureTest {
            stable_key: entity.stable_key.0.clone(),
            name: entity.name.clone(),
            source: entity_source(entity),
        })
        .collect::<Vec<_>>();
    tests.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    tests.truncate(16);
    tests
}

fn architecture_diagnostics(
    snapshot: &CanonicalSnapshot,
    relevant_ids: &HashSet<EntityId>,
) -> Vec<RustokArchitectureDiagnostic> {
    snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.status == DiagnosticStatus::Open
                && diagnostic
                    .entities
                    .iter()
                    .any(|entity| relevant_ids.contains(entity))
        })
        .map(|diagnostic| RustokArchitectureDiagnostic {
            kind: serialized_name(&diagnostic.kind),
            severity: serialized_name(&diagnostic.severity),
            message: diagnostic.message.clone(),
            source: diagnostic.evidence.iter().find_map(evidence_source),
        })
        .take(16)
        .collect()
}

fn architecture_evidence(
    snapshot: &CanonicalSnapshot,
    relevant_ids: &HashSet<EntityId>,
) -> Vec<RustokArchitectureEvidence> {
    let entities_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), entity))
        .collect::<HashMap<_, _>>();
    let mut evidence = BTreeSet::new();
    for id in relevant_ids {
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
        if relevant_ids.contains(&relation.from) || relevant_ids.contains(&relation.to) {
            let stable_key = relation_endpoints(relation, &entities_by_id);
            for source in relation.evidence.iter().filter_map(evidence_source) {
                evidence.insert(RustokArchitectureEvidence {
                    stable_key: stable_key.clone(),
                    source,
                });
            }
        }
    }
    evidence.into_iter().collect()
}

fn relation_endpoints(relation: &Relation, entities: &HashMap<EntityId, &Entity>) -> String {
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
    use athanor_domain::{EntityKind, Ownership, SnapshotId, SourceLocation, StableKey};
    use serde_json::json;

    use super::*;

    #[test]
    fn compacts_module_contract_dependency_and_test_evidence() {
        let product = entity(
            "product",
            "fba_module://product",
            EntityKind::Other("rustok_fba_module".to_string()),
            "crates/rustok-product/contracts/product-fba-registry.json",
        );
        let contract = entity(
            "contract",
            "fba_contract://product/product.catalog_read.v1",
            EntityKind::Other("rustok_fba_contract".to_string()),
            "crates/rustok-product/contracts/product-fba-registry.json",
        );
        let dependency = entity(
            "dependency",
            "fba_dependency://search/product/embedded_native",
            EntityKind::Dependency,
            "crates/rustok-search/contracts/search-fba-registry.json",
        );
        let test = entity(
            "test",
            "test://search_uses_product_catalog_contract",
            EntityKind::TestCase,
            "crates/rustok-search/tests/product_contract.rs",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![product.clone(), contract, dependency, test.clone()],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let options = RustokArchitectureContextOptions::bounded(
            PathBuf::from("."),
            "catalog search".to_string(),
            Some("product".to_string()),
        );

        let report = build_rustok_architecture_context(
            &snapshot,
            &options,
            &[product.id.clone(), test.id.clone()],
        );

        assert_eq!(report.schema, RUSTOK_ARCHITECTURE_CONTEXT_SCHEMA);
        assert_eq!(report.resolution.status, "resolved");
        assert_eq!(report.resolution.primary_module.as_deref(), Some("product"));
        assert_eq!(
            report.contracts[0].stable_key,
            "fba_contract://product/product.catalog_read.v1"
        );
        assert_eq!(report.interactions[0].consumer, "search");
        assert_eq!(report.interactions[0].provider, "product");
        assert_eq!(report.tests[0].name, "test");
    }

    #[test]
    fn comparable_module_scores_require_an_ownership_decision() {
        let product = entity(
            "product",
            "fba_module://product",
            EntityKind::Other("rustok_fba_module".to_string()),
            "crates/rustok-product/contracts/product-fba-registry.json",
        );
        let search = entity(
            "search",
            "fba_module://search",
            EntityKind::Other("rustok_fba_module".to_string()),
            "crates/rustok-search/contracts/search-fba-registry.json",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![product.clone(), search.clone()],
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
        };
        let options = RustokArchitectureContextOptions::bounded(
            PathBuf::from("."),
            "catalog search".to_string(),
            None,
        );

        let report = build_rustok_architecture_context(
            &snapshot,
            &options,
            &[product.id.clone(), search.id.clone()],
        );

        assert_eq!(report.resolution.status, "ambiguous");
        assert_eq!(report.resolution.primary_module, None);
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, source: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: id.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: source.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: vec![Ownership {
                source_file: source.to_string(),
            }],
            payload: json!({}),
        }
    }
}
