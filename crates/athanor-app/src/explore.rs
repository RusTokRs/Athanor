//! Bounded single-call code exploration over one exact canonical snapshot.
//!
//! `explore` answers one agent question with ranked entry entities, their portable source
//! anchors, citation evidence, and the one-hop relation neighbourhood in both directions. It
//! composes bounded retrieval (the same lexical/semantic/hybrid search the `search` tool uses)
//! with the committed snapshot itself, and it never reads source files: every field is derived
//! from canonical entities, relations, facts, and diagnostics.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, OperationContext};
use athanor_domain::{Entity, EntityId, Evidence, Relation, SourceLocation};
use serde::{Deserialize, Serialize};

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::json_contract::{EXPLORE_SCHEMA_V1, VersionedJsonContract};
use crate::project_path::normalize_canonical_path;
use crate::search::{
    SearchMode, SearchReport, search_snapshot_with_composition_and_operation_context,
};

/// Default number of entry entities returned for one explore request.
pub const DEFAULT_EXPLORE_ENTRIES: usize = 5;
/// Hard bound on entry entities, so one request stays bounded.
pub const MAX_EXPLORE_ENTRIES: usize = 20;
/// Default number of relations returned per entry and direction.
pub const DEFAULT_EXPLORE_LINKS: usize = 8;
/// Hard bound on relations per entry and direction.
pub const MAX_EXPLORE_LINKS: usize = 40;
/// Bounded citation list per entry and link.
const MAX_CITATIONS: usize = 4;

#[derive(Debug, Clone)]
pub struct ExploreOptions {
    pub root: PathBuf,
    pub query: String,
    pub entries: usize,
    pub links: usize,
    pub mode: SearchMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreLimits {
    pub entries: usize,
    pub links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreOmissions {
    pub entries_unresolved: usize,
    pub links_truncated: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreCitation {
    pub source_file: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub extractor: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreCounterpart {
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub source: Option<SourceLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreLink {
    pub relation: String,
    pub status: String,
    pub confidence: f32,
    pub counterpart: ExploreCounterpart,
    pub evidence: Vec<ExploreCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreEntry {
    pub stable_key: String,
    pub entity_id: EntityId,
    pub kind: String,
    pub name: String,
    pub source: Option<SourceLocation>,
    pub score: f32,
    pub citations: Vec<ExploreCitation>,
    pub outgoing: Vec<ExploreLink>,
    pub incoming: Vec<ExploreLink>,
    pub open_diagnostics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreReport {
    pub schema: String,
    pub root: PathBuf,
    pub snapshot: String,
    pub query: String,
    pub mode: SearchMode,
    pub limits: ExploreLimits,
    pub entries: Vec<ExploreEntry>,
    pub omitted: ExploreOmissions,
}

impl VersionedJsonContract for ExploreReport {
    const SCHEMA: &'static str = EXPLORE_SCHEMA_V1;

    fn schema(&self) -> &str {
        &self.schema
    }
}

/// Resolves one bounded explore request against the latest committed snapshot.
pub async fn explore_project_with_composition_and_operation_context(
    options: ExploreOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<ExploreReport> {
    if options.query.trim().is_empty() {
        bail!("explore query must not be empty");
    }
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let entries = options.entries.clamp(1, MAX_EXPLORE_ENTRIES);
    let links = options.links.clamp(1, MAX_EXPLORE_LINKS);
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;

    let search = search_snapshot_with_composition_and_operation_context(
        &root,
        &snapshot,
        options.query.trim().to_string(),
        entries,
        options.mode,
        composition,
        operation,
    )
    .await?;

    Ok(explore_snapshot(&root, &snapshot, &search, entries, links))
}

/// Builds the bounded report from one snapshot and one already-resolved retrieval result.
pub fn explore_snapshot(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    search: &SearchReport,
    entries: usize,
    links: usize,
) -> ExploreReport {
    let mut built = Vec::new();
    let mut unresolved = 0usize;
    let mut truncated = 0usize;
    let mut notes = Vec::new();

    for item in &search.results {
        if built.len() >= entries {
            break;
        }
        let Some(entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == item.stable_key)
        else {
            unresolved += 1;
            continue;
        };
        let (outgoing, out_truncated) = links_for(snapshot, entity, Direction::Outgoing, links);
        let (incoming, in_truncated) = links_for(snapshot, entity, Direction::Incoming, links);
        truncated += out_truncated + in_truncated;
        built.push(ExploreEntry {
            stable_key: entity.stable_key.0.clone(),
            entity_id: entity.id.clone(),
            kind: item.kind.clone(),
            name: entity.name.clone(),
            source: entity.source.clone(),
            score: item.score,
            citations: citations_for(snapshot, entity),
            outgoing,
            incoming,
            open_diagnostics: snapshot
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.entities.contains(&entity.id))
                .count(),
        });
    }

    if unresolved > 0 {
        notes.push(format!(
            "{unresolved} ranked results are not canonical entities in this snapshot"
        ));
    }
    if truncated > 0 {
        notes.push(format!(
            "{truncated} relations fall outside the per-entry link budget"
        ));
    }
    if search.truncated {
        notes.push("retrieval returned more candidates than the entry budget".to_string());
    }

    ExploreReport {
        schema: EXPLORE_SCHEMA_V1.to_string(),
        root: root.to_path_buf(),
        snapshot: search.snapshot.clone(),
        query: search.query.clone(),
        mode: search.mode,
        limits: ExploreLimits { entries, links },
        entries: built,
        omitted: ExploreOmissions {
            entries_unresolved: unresolved,
            links_truncated: truncated,
            notes,
        },
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Outgoing,
    Incoming,
}

fn links_for(
    snapshot: &CanonicalSnapshot,
    entity: &Entity,
    direction: Direction,
    limit: usize,
) -> (Vec<ExploreLink>, usize) {
    let mut relations = snapshot
        .relations
        .iter()
        .filter(|relation| match direction {
            Direction::Outgoing => relation.from == entity.id,
            Direction::Incoming => relation.to == entity.id,
        })
        .collect::<Vec<_>>();
    relations.sort_by(|left, right| {
        link_sort_key(left, direction).cmp(&link_sort_key(right, direction))
    });
    let truncated = relations.len().saturating_sub(limit);
    let links = relations
        .into_iter()
        .take(limit)
        .map(|relation| explore_link(snapshot, relation, direction))
        .collect();
    (links, truncated)
}

fn link_sort_key(relation: &Relation, direction: Direction) -> (String, String) {
    let counterpart = match direction {
        Direction::Outgoing => &relation.to,
        Direction::Incoming => &relation.from,
    };
    (label(&relation.kind), counterpart.0.clone())
}

fn explore_link(
    snapshot: &CanonicalSnapshot,
    relation: &Relation,
    direction: Direction,
) -> ExploreLink {
    let counterpart_id = match direction {
        Direction::Outgoing => &relation.to,
        Direction::Incoming => &relation.from,
    };
    let counterpart = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == *counterpart_id);
    ExploreLink {
        relation: label(&relation.kind),
        status: label(&relation.status),
        confidence: relation.confidence,
        counterpart: match counterpart {
            Some(counterpart) => ExploreCounterpart {
                stable_key: counterpart.stable_key.0.clone(),
                kind: label(&counterpart.kind),
                name: counterpart.name.clone(),
                source: counterpart.source.clone(),
            },
            None => ExploreCounterpart {
                stable_key: counterpart_id.0.clone(),
                kind: "unknown".to_string(),
                name: counterpart_id.0.clone(),
                source: None,
            },
        },
        evidence: citations_from_evidence(&relation.evidence),
    }
}

fn citations_for(snapshot: &CanonicalSnapshot, entity: &Entity) -> Vec<ExploreCitation> {
    let mut citations = Vec::new();
    let mut seen = BTreeSet::new();
    for fact in snapshot.facts.iter().filter(|fact| {
        fact.subject == entity.id || fact.object.as_ref().is_some_and(|id| *id == entity.id)
    }) {
        for evidence in &fact.evidence {
            if citations.len() >= MAX_CITATIONS {
                return citations;
            }
            if seen.insert(evidence_key(evidence)) {
                citations.push(citation(evidence));
            }
        }
    }
    citations
}

fn citations_from_evidence(evidence: &[Evidence]) -> Vec<ExploreCitation> {
    let mut citations = Vec::new();
    let mut seen = BTreeSet::new();
    for evidence in evidence {
        if citations.len() >= MAX_CITATIONS {
            break;
        }
        if seen.insert(evidence_key(evidence)) {
            citations.push(citation(evidence));
        }
    }
    citations
}

fn evidence_key(evidence: &Evidence) -> (String, Option<u32>, Option<u32>) {
    (
        evidence.source_file.clone().unwrap_or_default(),
        evidence.line_start,
        evidence.line_end,
    )
}

fn citation(evidence: &Evidence) -> ExploreCitation {
    ExploreCitation {
        source_file: evidence.source_file.clone(),
        line_start: evidence.line_start,
        line_end: evidence.line_end,
        extractor: evidence.extractor.clone(),
        confidence: evidence.confidence,
    }
}

/// Canonical JSON label of a domain enum, matching the serialized snapshot vocabulary.
fn label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, EntityKind, EvidenceStatus,
        Fact, FactId, FactKind, RelationId, RelationKind, RelationStatus, Severity, SnapshotId,
        StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn explore_returns_ranked_entries_with_one_hop_links_and_citations() {
        let snapshot = snapshot_fixture();
        let report = explore_snapshot(
            Path::new("repo"),
            &snapshot,
            &search_fixture(&["symbol://rust:crate::store::Store::load"]),
            5,
            8,
        );

        assert_eq!(report.schema, EXPLORE_SCHEMA_V1);
        assert_eq!(report.snapshot, "snap_fixture");
        assert_eq!(report.entries.len(), 1);
        let entry = &report.entries[0];
        assert_eq!(entry.name, "load");
        assert_eq!(entry.kind, "function");
        assert_eq!(entry.score, 1.5);
        assert_eq!(entry.source.as_ref().unwrap().path, "src/store.rs");
        assert_eq!(entry.open_diagnostics, 1);
        assert_eq!(entry.citations.len(), 1);
        assert_eq!(entry.citations[0].extractor.as_deref(), Some("rust"));
        assert_eq!(entry.outgoing.len(), 2);
        assert_eq!(entry.outgoing[0].relation, "calls");
        assert_eq!(entry.outgoing[0].status, "verified");
        assert_eq!(entry.outgoing[0].counterpart.name, "refresh");
        assert_eq!(
            entry.outgoing[0].counterpart.stable_key,
            "symbol://rust:crate::store::Store::refresh"
        );
        assert_eq!(entry.outgoing[1].relation, "imports");
        assert_eq!(entry.outgoing[1].counterpart.name, "store");
        assert_eq!(entry.incoming.len(), 1);
        assert_eq!(entry.incoming[0].relation, "imports");
        assert_eq!(entry.incoming[0].counterpart.name, "store");
        assert!(report.omitted.notes.is_empty());
    }

    #[test]
    fn explore_truncates_links_and_discloses_the_omission() {
        let snapshot = snapshot_fixture();
        let report = explore_snapshot(
            Path::new("repo"),
            &snapshot,
            &search_fixture(&["symbol://rust:crate::store::Store::load"]),
            5,
            1,
        );

        assert_eq!(report.entries[0].outgoing.len(), 1);
        assert_eq!(report.entries[0].incoming.len(), 1);
        assert_eq!(report.omitted.links_truncated, 1);
        assert_eq!(report.limits.links, 1);
        assert!(
            report
                .omitted
                .notes
                .iter()
                .any(|note| note.contains("outside the per-entry link budget")),
            "truncation must be disclosed: {:?}",
            report.omitted.notes
        );
    }

    #[test]
    fn explore_discloses_results_that_are_not_canonical_entities() {
        let snapshot = snapshot_fixture();
        let report = explore_snapshot(
            Path::new("repo"),
            &snapshot,
            &search_fixture(&["symbol://rust:crate::missing"]),
            5,
            8,
        );

        assert!(report.entries.is_empty());
        assert_eq!(report.omitted.entries_unresolved, 1);
        assert_eq!(report.omitted.notes.len(), 1);
        assert!(report.omitted.notes[0].contains("not canonical entities"));
    }

    #[test]
    fn explore_report_satisfies_its_json_contract() {
        let report = explore_snapshot(
            Path::new("repo"),
            &snapshot_fixture(),
            &search_fixture(&["symbol://rust:crate::store::Store::load"]),
            5,
            8,
        );

        report.validate_contract().unwrap();
        assert_eq!(ExploreReport::SCHEMA, EXPLORE_SCHEMA_V1);
    }

    fn snapshot_fixture() -> CanonicalSnapshot {
        let module = entity(
            "ent_module",
            "module://rust:crate::store",
            EntityKind::Module,
            "store",
        );
        let load = entity(
            "ent_load",
            "symbol://rust:crate::store::Store::load",
            EntityKind::Function,
            "load",
        );
        let refresh = entity(
            "ent_refresh",
            "symbol://rust:crate::store::Store::refresh",
            EntityKind::Function,
            "refresh",
        );
        CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_fixture".to_string())),
            entities: vec![module.clone(), load.clone(), refresh.clone()],
            facts: vec![Fact {
                id: FactId("fact_load".to_string()),
                kind: FactKind::SymbolDefined,
                subject: load.id.clone(),
                object: None,
                value: json!({}),
                evidence: vec![Evidence {
                    source_file: Some("src/store.rs".to_string()),
                    line_start: Some(12),
                    line_end: Some(20),
                    extractor: Some("rust".to_string()),
                    commit_hash: None,
                    confidence: 1.0,
                    status: EvidenceStatus::Verified,
                }],
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_fixture".to_string()),
                extractor: "rust".to_string(),
                confidence: 1.0,
            }],
            relations: vec![
                relation(
                    "rel_calls",
                    RelationKind::Calls,
                    &load,
                    &refresh,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_imports",
                    RelationKind::Imports,
                    &module,
                    &load,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_module_import",
                    RelationKind::Imports,
                    &load,
                    &module,
                    RelationStatus::Verified,
                ),
            ],
            diagnostics: vec![Diagnostic {
                id: DiagnosticId("diag_load".to_string()),
                kind: DiagnosticKind::UncoveredSymbol,
                severity: Severity::Medium,
                status: DiagnosticStatus::Open,
                title: "Uncovered symbol".to_string(),
                message: "load lacks coverage".to_string(),
                entities: vec![load.id.clone()],
                evidence: Vec::new(),
                ownership: Vec::new(),
                snapshot: SnapshotId("snap_fixture".to_string()),
                suggested_fix: None,
                payload: json!({}),
            }],
        }
    }

    fn search_fixture(stable_keys: &[&str]) -> SearchReport {
        SearchReport {
            schema: crate::json_contract::SEARCH_SCHEMA_V1.to_string(),
            root: PathBuf::from("repo"),
            snapshot: "snap_fixture".to_string(),
            query: "store load".to_string(),
            mode: SearchMode::Lexical,
            limit: 5,
            returned: stable_keys.len(),
            truncated: false,
            omitted: crate::search::SearchOmissions {
                results_lower_bound: stable_keys.len(),
                reason: None,
            },
            results: stable_keys
                .iter()
                .map(|stable_key| crate::search::SearchItem {
                    entity_id: EntityId(format!("ent_{stable_key}")),
                    stable_key: (*stable_key).to_string(),
                    kind: "function".to_string(),
                    name: "load".to_string(),
                    title: None,
                    source: None,
                    ownership: Vec::new(),
                    score: 1.5,
                })
                .collect(),
        }
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, name: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: "src/store.rs".to_string(),
                line_start: Some(1),
                line_end: Some(9),
            }),
            language: Some(athanor_domain::LanguageCode("rust".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn relation(
        id: &str,
        kind: RelationKind,
        from: &Entity,
        to: &Entity,
        status: RelationStatus,
    ) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status,
            confidence: 1.0,
            evidence: vec![Evidence {
                source_file: Some("src/store.rs".to_string()),
                line_start: Some(12),
                line_end: Some(12),
                extractor: Some("rust-linker".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_fixture".to_string()),
            payload: json!({}),
        }
    }
}
