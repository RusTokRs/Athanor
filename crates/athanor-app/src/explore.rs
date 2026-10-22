//! Bounded single-call code exploration over one exact canonical snapshot.
//!
//! `explore` answers one agent question with ranked entry entities, their portable source anchors,
//! bounded declaration excerpts, citation evidence, the one-hop relation neighbourhood in both
//! directions, bounded multi-hop code-flow chains, and an aggregate blast radius for the same hop
//! budget. Entries are seeded either by bounded retrieval (the same lexical/semantic/hybrid search
//! the `search` tool uses) or by an exact stable key or repository path, so an agent that already
//! knows where to look does not pay a retrieval round-trip first. Graph fields come from the
//! canonical snapshot; excerpts read only the cited line range of each declaration, clipped to a
//! line and byte bound, and a missing, escaped, or non-UTF-8 file is disclosed rather than guessed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, OperationContext};
use athanor_domain::{DiagnosticStatus, Entity, EntityId, Evidence, Relation, SourceLocation};
use serde::{Deserialize, Serialize};

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::impact::resolve_target_entities;
use crate::json_contract::{EXPLORE_SCHEMA_V1, VersionedJsonContract};
use crate::project_path::normalize_canonical_path;
use crate::search::{
    SearchMode, SearchReport, search_snapshot_with_composition_and_operation_context,
};

#[path = "explore/excerpts.rs"]
mod excerpts;
#[path = "explore/paths.rs"]
mod paths;

pub use excerpts::{ExploreExcerpt, MAX_EXCERPT_LINES};
pub use paths::{
    DEFAULT_EXPLORE_DEPTH, DEFAULT_EXPLORE_PATHS, ExploreBlast, ExploreBlastDepth, ExplorePath,
    ExplorePathEndpoint, ExplorePathStep, MAX_EXPLORE_DEPTH, MAX_EXPLORE_PATHS,
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
    /// Exact stable key or repository path used instead of retrieval when present.
    pub target: Option<String>,
    pub entries: usize,
    pub links: usize,
    pub depth: usize,
    pub paths: usize,
    pub mode: SearchMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExploreLimits {
    pub entries: usize,
    pub links: usize,
    pub depth: usize,
    pub paths: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreOmissions {
    pub entries_unresolved: usize,
    pub entries_over_budget: usize,
    pub links_truncated: usize,
    pub paths_truncated: usize,
    /// Cited declaration ranges that could not be read from the working tree.
    pub source_unread: usize,
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
    /// Bounded source slice of the declaration, when the cited range can be read.
    pub excerpt: Option<ExploreExcerpt>,
}

/// One resolved starting point of an exploration, before canonical entity lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreSeed {
    pub stable_key: String,
    pub kind: String,
    pub score: f32,
}

/// One prepared explore request: the seeds to resolve plus the retrieval state that produced them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExploreRequest {
    pub query: String,
    pub mode: SearchMode,
    pub snapshot: String,
    pub anchor: Option<String>,
    pub candidates: usize,
    pub retrieval_truncated: bool,
    pub seeds: Vec<ExploreSeed>,
}

impl ExploreRequest {
    /// Derives the request from one already-resolved retrieval report.
    pub fn from_search(search: &SearchReport) -> Self {
        Self {
            query: search.query.clone(),
            mode: search.mode,
            snapshot: search.snapshot.clone(),
            anchor: None,
            candidates: search.returned,
            retrieval_truncated: search.truncated,
            seeds: search
                .results
                .iter()
                .map(|item| ExploreSeed {
                    stable_key: item.stable_key.clone(),
                    kind: item.kind.clone(),
                    score: item.score,
                })
                .collect(),
        }
    }

    /// Derives the request from entities resolved out of an exact stable key or repository path.
    pub fn from_target(
        snapshot_id: &str,
        query: String,
        mode: SearchMode,
        target: &str,
        entities: &[Entity],
    ) -> Self {
        Self {
            query,
            mode,
            snapshot: snapshot_id.to_string(),
            anchor: Some(target.to_string()),
            candidates: entities.len(),
            retrieval_truncated: false,
            seeds: entities
                .iter()
                .map(|entity| ExploreSeed {
                    stable_key: entity.stable_key.0.clone(),
                    kind: label(&entity.kind),
                    score: 1.0,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreReport {
    pub schema: String,
    pub root: PathBuf,
    pub snapshot: String,
    pub query: String,
    pub mode: SearchMode,
    pub anchor: Option<String>,
    pub limits: ExploreLimits,
    pub entries: Vec<ExploreEntry>,
    pub paths: Vec<ExplorePath>,
    pub blast: ExploreBlast,
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
    let query = options.query.trim().to_string();
    let target = options
        .target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(str::to_string);
    if query.is_empty() && target.is_none() {
        bail!("explore requires a non-empty query or a target stable key or path");
    }
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let limits = ExploreLimits {
        entries: options.entries.clamp(1, MAX_EXPLORE_ENTRIES),
        links: options.links.clamp(1, MAX_EXPLORE_LINKS),
        depth: options.depth.clamp(1, MAX_EXPLORE_DEPTH),
        paths: options.paths.clamp(1, MAX_EXPLORE_PATHS),
    };
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .ok_or_else(|| anyhow!("latest canonical snapshot has no snapshot id"))?;

    let request = match target {
        Some(target) => {
            let resolved = resolve_target_entities(&root, &snapshot.entities, &target)?;
            ExploreRequest::from_target(&snapshot_id, query, options.mode, &target, &resolved)
        }
        None => {
            let search = search_snapshot_with_composition_and_operation_context(
                &root,
                &snapshot,
                query,
                limits.entries,
                options.mode,
                composition,
                operation,
            )
            .await?;
            ExploreRequest::from_search(&search)
        }
    };

    Ok(explore_snapshot(&root, &snapshot, &request, &limits))
}

/// Builds the bounded report from one snapshot and one already-prepared request.
pub fn explore_snapshot(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    request: &ExploreRequest,
    limits: &ExploreLimits,
) -> ExploreReport {
    let ExploreLimits {
        entries,
        links,
        depth,
        paths: path_budget,
    } = *limits;
    let mut built = Vec::new();
    let mut explored = Vec::new();
    let mut unresolved = 0usize;
    let mut links_truncated = 0usize;
    let mut notes = Vec::new();

    for seed in &request.seeds {
        if built.len() >= entries {
            break;
        }
        let Some(entity) = snapshot
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == seed.stable_key)
        else {
            unresolved += 1;
            continue;
        };
        let (outgoing, out_truncated) = links_for(snapshot, entity, Direction::Outgoing, links);
        let (incoming, in_truncated) = links_for(snapshot, entity, Direction::Incoming, links);
        links_truncated += out_truncated + in_truncated;
        explored.push(entity);
        built.push(ExploreEntry {
            stable_key: entity.stable_key.0.clone(),
            entity_id: entity.id.clone(),
            kind: seed.kind.clone(),
            name: entity.name.clone(),
            source: entity.source.clone(),
            score: seed.score,
            citations: citations_for(snapshot, entity),
            outgoing,
            incoming,
            open_diagnostics: snapshot
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.status == DiagnosticStatus::Open
                        && diagnostic.entities.contains(&entity.id)
                })
                .count(),
            excerpt: None,
        });
    }

    let traversal = paths::explore_flow(snapshot, &explored, depth, path_budget);
    let mut paths = traversal.paths;
    let mut reader = excerpts::ExcerptReader::new(root);
    for entry in &mut built {
        entry.excerpt = reader.take(entry.source.as_ref());
    }
    for path in &mut paths {
        path.excerpt = reader.take(path.target.source.as_ref());
    }
    let source_unread = reader.unread();
    let entries_over_budget = request.candidates.saturating_sub(built.len());

    if unresolved > 0 {
        notes.push(format!(
            "{unresolved} ranked results are not canonical entities in this snapshot"
        ));
    }
    if entries_over_budget > 0 {
        notes.push(format!(
            "{entries_over_budget} resolved candidates fall outside the entry budget"
        ));
    }
    if links_truncated > 0 {
        notes.push(format!(
            "{links_truncated} relations fall outside the per-entry link budget"
        ));
    }
    if request.retrieval_truncated {
        notes.push("retrieval returned more candidates than the entry budget".to_string());
    }
    if source_unread > 0 {
        notes.push(format!(
            "{source_unread} source excerpts could not be read from the working tree"
        ));
    }
    notes.extend(traversal.notes);

    ExploreReport {
        schema: EXPLORE_SCHEMA_V1.to_string(),
        root: root.to_path_buf(),
        snapshot: request.snapshot.clone(),
        query: request.query.clone(),
        mode: request.mode,
        anchor: request.anchor.clone(),
        limits: *limits,
        entries: built,
        paths,
        blast: traversal.blast,
        omitted: ExploreOmissions {
            entries_unresolved: unresolved,
            entries_over_budget,
            links_truncated,
            paths_truncated: traversal.paths_truncated,
            source_unread,
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
        Fact, FactId, FactKind, LanguageCode, RelationId, RelationKind, RelationStatus, Severity,
        SnapshotId, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    const LOAD: &str = "symbol://rust:crate::store::Store::load";

    #[test]
    fn explore_returns_ranked_entries_with_one_hop_links_and_citations() {
        let snapshot = snapshot_fixture();
        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 1,
                paths: 16,
            },
        );

        assert_eq!(report.schema, EXPLORE_SCHEMA_V1);
        assert_eq!(report.snapshot, "snap_fixture");
        assert_eq!(report.anchor, None);
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
        assert_eq!(entry.incoming.len(), 2);
        assert_eq!(entry.incoming[0].relation, "calls");
        assert_eq!(entry.incoming[0].counterpart.name, "run");
        assert_eq!(entry.incoming[1].relation, "imports");
        assert_eq!(entry.incoming[1].counterpart.name, "store");
        // A one-hop request still reports the blast radius it actually traversed.
        assert_eq!(report.blast.depth, 1);
        assert_eq!(report.blast.entries, 1);
        assert_eq!(report.blast.reached, 3);
        assert!(report.paths.is_empty());
        assert!(
            report
                .omitted
                .notes
                .iter()
                .any(|note| note.contains("depth below two hops")),
            "shallow traversal must be disclosed: {:?}",
            report.omitted.notes
        );
    }

    #[test]
    fn explore_reports_multi_hop_chains_and_the_blast_radius() {
        let snapshot = snapshot_fixture();
        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 16,
            },
        );

        assert_eq!(report.paths.len(), 2);
        let first = &report.paths[0];
        assert_eq!(first.origin, LOAD);
        assert_eq!(first.depth, 2);
        assert_eq!(first.target.name, "flush");
        assert_eq!(first.steps.len(), 2);
        assert_eq!(first.steps[0].relation, "calls");
        assert_eq!(first.steps[0].direction, "forward");
        assert_eq!(first.steps[0].from.stable_key, LOAD);
        assert_eq!(first.steps[0].to.name, "refresh");
        assert_eq!(first.steps[0].source_file.as_deref(), Some("src/store.rs"));
        assert_eq!(first.steps[1].to.name, "flush");
        let second = &report.paths[1];
        assert_eq!(second.target.name, "writer");
        assert_eq!(second.steps[1].relation, "imports");
        assert_eq!(second.steps[1].status, "inferred");

        assert_eq!(report.blast.reached, 5);
        assert_eq!(report.blast.relations, 5);
        assert_eq!(report.blast.by_depth[0].depth, 1);
        assert_eq!(report.blast.by_depth[0].entities, 3);
        assert_eq!(report.blast.by_depth[1].depth, 2);
        assert_eq!(report.blast.by_depth[1].entities, 2);
        assert_eq!(
            report.blast.files,
            vec!["src/store.rs".to_string(), "src/writer.rs".to_string()]
        );
        assert_eq!(report.blast.files_omitted, 0);
        assert_eq!(report.blast.open_diagnostics, 1);
        // `uses_env` is not code flow: the environment entity is never reached or chained.
        assert_eq!(report.blast.edge_visits, 10);
        assert!(
            !report
                .paths
                .iter()
                .any(|path| path.target.name == "LOG_LEVEL"),
            "diagnostic-only relations must not carry the traversal"
        );
        assert!(report.entries[0].excerpt.is_none());
        assert_eq!(report.omitted.source_unread, 3);
        assert_eq!(report.omitted.notes.len(), 1);
        assert!(
            report.omitted.notes[0].contains("source excerpts could not be read"),
            "a missing working tree must be disclosed: {:?}",
            report.omitted.notes
        );
    }

    #[test]
    fn explore_blast_excludes_the_entry_set_from_reached_entities() {
        let snapshot = snapshot_fixture();
        let report = explore(
            &snapshot,
            &[LOAD, "symbol://rust:crate::store::Store::refresh"],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 16,
            },
        );

        assert_eq!(report.blast.entries, 2);
        assert_eq!(report.blast.reached, 4);
        assert_eq!(report.blast.by_depth[0].entities, 4);
        assert_eq!(
            report
                .blast
                .by_depth
                .iter()
                .map(|depth| depth.entities)
                .sum::<usize>(),
            report.blast.reached,
            "an entity reached from two entries must count only at its shortest depth"
        );
    }

    #[test]
    fn explore_bounds_multi_hop_chains_and_discloses_the_omission() {
        let snapshot = snapshot_fixture();
        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 1,
            },
        );

        assert_eq!(report.paths.len(), 1);
        assert_eq!(report.paths[0].target.name, "flush");
        assert_eq!(report.omitted.paths_truncated, 1);
        assert_eq!(report.limits.paths, 1);
        assert!(
            report
                .omitted
                .notes
                .iter()
                .any(|note| note.contains("outside the per-request path budget")),
            "path truncation must be disclosed: {:?}",
            report.omitted.notes
        );
    }

    #[test]
    fn explore_anchored_on_a_target_reports_the_anchor_and_over_budget_candidates() {
        let snapshot = snapshot_fixture();
        let request = ExploreRequest {
            query: String::new(),
            mode: SearchMode::default(),
            snapshot: "snap_fixture".to_string(),
            anchor: Some("src/store.rs".to_string()),
            candidates: 3,
            retrieval_truncated: false,
            seeds: vec![
                ExploreSeed {
                    stable_key: LOAD.to_string(),
                    kind: "function".to_string(),
                    score: 1.0,
                },
                ExploreSeed {
                    stable_key: "symbol://rust:crate::store::Store::refresh".to_string(),
                    kind: "function".to_string(),
                    score: 1.0,
                },
                ExploreSeed {
                    stable_key: "symbol://rust:crate::missing".to_string(),
                    kind: "function".to_string(),
                    score: 1.0,
                },
            ],
        };
        let limits = ExploreLimits {
            entries: 2,
            links: 8,
            depth: 2,
            paths: 16,
        };

        let report = explore_snapshot(Path::new("repo"), &snapshot, &request, &limits);

        assert_eq!(report.anchor.as_deref(), Some("src/store.rs"));
        assert!(report.query.is_empty());
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.omitted.entries_over_budget, 1);
        assert_eq!(report.omitted.entries_unresolved, 0);
        assert_eq!(report.blast.entries, 2);
        assert!(
            report
                .omitted
                .notes
                .iter()
                .any(|note| note.contains("outside the entry budget")),
            "over-budget candidates must be disclosed: {:?}",
            report.omitted.notes
        );
    }

    #[test]
    fn explore_truncates_links_and_discloses_the_omission() {
        let snapshot = snapshot_fixture();
        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 1,
                depth: 1,
                paths: 16,
            },
        );

        assert_eq!(report.entries[0].outgoing.len(), 1);
        assert_eq!(report.entries[0].incoming.len(), 1);
        assert_eq!(report.omitted.links_truncated, 2);
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
        let report = explore(
            &snapshot,
            &["symbol://rust:crate::missing"],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 16,
            },
        );

        assert!(report.entries.is_empty());
        assert!(report.paths.is_empty());
        assert_eq!(
            report.blast,
            ExploreBlast {
                depth: 2,
                ..ExploreBlast::default()
            }
        );
        assert_eq!(report.omitted.entries_unresolved, 1);
        assert!(report.omitted.notes[0].contains("not canonical entities"));
        assert_eq!(report.omitted.source_unread, 0);
    }

    #[test]
    fn explore_inlines_bounded_declaration_excerpts() {
        let root = std::env::temp_dir().join(format!(
            "athanor-explore-excerpt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/store.rs"),
            "mod store;\nuse crate::store;\npub fn load() {\n    refresh();\n}\nfn extra() {}\nfn more() {}\nfn tail() {}\nfn end() {}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/writer.rs"),
            "pub struct Writer;\nimpl Writer {}\nfn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn f() {}\nfn g() {}\n",
        )
        .unwrap();

        let snapshot = snapshot_fixture();
        let report = explore_snapshot(
            &root,
            &snapshot,
            &ExploreRequest {
                query: "store load".to_string(),
                mode: SearchMode::Lexical,
                snapshot: "snap_fixture".to_string(),
                anchor: None,
                candidates: 1,
                retrieval_truncated: false,
                seeds: vec![ExploreSeed {
                    stable_key: LOAD.to_string(),
                    kind: "function".to_string(),
                    score: 1.5,
                }],
            },
            &ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 16,
            },
        );

        let excerpt = report.entries[0]
            .excerpt
            .as_ref()
            .expect("the cited declaration range must be inlined");
        assert_eq!(excerpt.source_file, "src/store.rs");
        assert_eq!(excerpt.line_start, 1);
        assert_eq!(excerpt.line_end, 9);
        assert!(!excerpt.truncated);
        assert!(excerpt.text.contains("pub fn load() {"));
        assert_eq!(report.omitted.source_unread, 0);
        assert!(
            report.paths.iter().any(|path| path.target.name == "writer"
                && path
                    .excerpt
                    .as_ref()
                    .is_some_and(|excerpt| excerpt.source_file == "src/writer.rs")),
            "a path target must carry its own excerpt"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explore_refuses_an_escaped_source_path() {
        let mut snapshot = snapshot_fixture();
        snapshot.entities[1].source = Some(SourceLocation {
            path: "../secret.rs".to_string(),
            line_start: Some(1),
            line_end: Some(2),
        });
        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 1,
                paths: 16,
            },
        );

        assert!(report.entries[0].excerpt.is_none());
        assert!(report.omitted.source_unread >= 1);
        assert!(
            report
                .omitted
                .notes
                .iter()
                .any(|note| note.contains("source excerpts could not be read")),
            "an escaped citation must be disclosed: {:?}",
            report.omitted.notes
        );
    }

    #[test]
    fn explore_counts_only_open_diagnostics() {
        let mut snapshot = snapshot_fixture();
        snapshot.diagnostics[0].status = DiagnosticStatus::Resolved;
        let mut suppressed = snapshot.diagnostics[0].clone();
        suppressed.id = DiagnosticId("diag_flush_suppressed".to_string());
        suppressed.status = DiagnosticStatus::Suppressed;
        suppressed.entities = vec![EntityId("ent_flush".to_string())];
        snapshot.diagnostics.push(suppressed);

        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 16,
            },
        );

        assert_eq!(report.entries[0].open_diagnostics, 0);
        assert_eq!(report.blast.open_diagnostics, 0);
        assert!(report.paths.iter().all(|path| path.open_diagnostics == 0));
    }

    #[test]
    fn explore_report_satisfies_its_json_contract() {
        let snapshot = snapshot_fixture();
        let report = explore(
            &snapshot,
            &[LOAD],
            ExploreLimits {
                entries: 5,
                links: 8,
                depth: 2,
                paths: 16,
            },
        );

        report.validate_contract().unwrap();
        assert_eq!(ExploreReport::SCHEMA, EXPLORE_SCHEMA_V1);
    }

    fn explore(
        snapshot: &CanonicalSnapshot,
        stable_keys: &[&str],
        limits: ExploreLimits,
    ) -> ExploreReport {
        let request = ExploreRequest {
            query: "store load".to_string(),
            mode: SearchMode::Lexical,
            snapshot: "snap_fixture".to_string(),
            anchor: None,
            candidates: stable_keys.len(),
            retrieval_truncated: false,
            seeds: stable_keys
                .iter()
                .map(|stable_key| ExploreSeed {
                    stable_key: (*stable_key).to_string(),
                    kind: "function".to_string(),
                    score: 1.5,
                })
                .collect(),
        };
        explore_snapshot(Path::new("repo"), snapshot, &request, &limits)
    }

    fn snapshot_fixture() -> CanonicalSnapshot {
        let module = entity(
            "ent_module",
            "module://rust:crate::store",
            EntityKind::Module,
            "store",
            "src/store.rs",
        );
        let load = entity(
            "ent_load",
            LOAD,
            EntityKind::Function,
            "load",
            "src/store.rs",
        );
        let refresh = entity(
            "ent_refresh",
            "symbol://rust:crate::store::Store::refresh",
            EntityKind::Function,
            "refresh",
            "src/store.rs",
        );
        let flush = entity(
            "ent_flush",
            "symbol://rust:crate::store::Store::flush",
            EntityKind::Function,
            "flush",
            "src/store.rs",
        );
        let run = entity(
            "ent_run",
            "symbol://rust:crate::run",
            EntityKind::Function,
            "run",
            "src/store.rs",
        );
        let writer = entity(
            "ent_writer",
            "symbol://rust:crate::writer::Writer",
            EntityKind::Symbol,
            "writer",
            "src/writer.rs",
        );
        let env = entity(
            "ent_env",
            "env://LOG_LEVEL",
            EntityKind::EnvVar,
            "LOG_LEVEL",
            "src/store.rs",
        );
        CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_fixture".to_string())),
            entities: vec![
                module.clone(),
                load.clone(),
                refresh.clone(),
                flush.clone(),
                run.clone(),
                writer.clone(),
                env.clone(),
            ],
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
                    "rel_calls_load",
                    RelationKind::Calls,
                    &load,
                    &refresh,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_calls_refresh_flush",
                    RelationKind::Calls,
                    &refresh,
                    &flush,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_imports_refresh_writer",
                    RelationKind::Imports,
                    &refresh,
                    &writer,
                    RelationStatus::Inferred,
                ),
                relation(
                    "rel_calls_run_load",
                    RelationKind::Calls,
                    &run,
                    &load,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_imports_module_load",
                    RelationKind::Imports,
                    &module,
                    &load,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_imports_load_module",
                    RelationKind::Imports,
                    &load,
                    &module,
                    RelationStatus::Verified,
                ),
                relation(
                    "rel_uses_env",
                    RelationKind::UsesEnv,
                    &refresh,
                    &env,
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

    fn entity(id: &str, stable_key: &str, kind: EntityKind, name: &str, path: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(9),
            }),
            language: Some(LanguageCode("rust".to_string())),
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
                source_file: from.source.as_ref().map(|source| source.path.clone()),
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
