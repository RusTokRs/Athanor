//! Bounded code-flow traversal shared by the `explore` entry points.
//!
//! The one-hop neighbourhood answers what an entry is connected to. This module answers the two
//! questions it cannot: which relation chains lead from an entry to a distant declaration, and how
//! large the blast radius of changing that entry actually is.
//!
//! The traversal walks only relation kinds that carry code influence, indexes the snapshot once for
//! every entry, keeps the shortest chain to each reached entity, and bounds both the hop count and
//! the number of expanded relation edges. Every dropped result is disclosed in the report instead of
//! being silently hidden.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{DiagnosticStatus, Entity, Relation, RelationKind};
use serde::{Deserialize, Serialize};

use super::{ExploreCounterpart, label};

/// Default number of code-flow hops explored from each entry entity.
pub const DEFAULT_EXPLORE_DEPTH: usize = 2;
/// Hard bound on explored hops, so one request stays bounded.
pub const MAX_EXPLORE_DEPTH: usize = 4;
/// Default number of multi-hop chains returned for one explore request.
pub const DEFAULT_EXPLORE_PATHS: usize = 16;
/// Hard bound on multi-hop chains per explore request.
pub const MAX_EXPLORE_PATHS: usize = 64;
/// Hard bound on relation-edge expansions per request, shared by every entry.
const MAX_EXPLORE_VISITS: usize = 4_096;
/// Hard bound on blast-radius files listed per request.
const MAX_EXPLORE_FILES: usize = 24;

/// One endpoint of a traversed relation, named the way an agent reads it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorePathEndpoint {
    /// Canonical entity id of the endpoint.
    pub entity_id: String,
    /// Portable stable key of the endpoint.
    pub stable_key: String,
    /// Declaration name of the endpoint.
    pub name: String,
}

/// One traversed relation inside a multi-hop chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorePathStep {
    /// Serialized relation kind, for example `calls`.
    pub relation: String,
    /// Serialized relation status, for example `verified`.
    pub status: String,
    /// Linker confidence of the traversed relation.
    pub confidence: f32,
    /// `forward` when the chain followed the relation, `backward` when it went against it.
    pub direction: String,
    /// Relation source endpoint.
    pub from: ExplorePathEndpoint,
    /// Relation target endpoint.
    pub to: ExplorePathEndpoint,
    /// First cited source file of the relation evidence, when the linker recorded one.
    pub source_file: Option<String>,
    /// First cited line of the relation evidence, when the linker recorded one.
    pub line_start: Option<u32>,
}

/// One shortest multi-hop chain from an entry entity to a reached entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorePath {
    /// Stable key of the entry entity this chain starts from.
    pub origin: String,
    /// Number of relation hops in the chain.
    pub depth: usize,
    /// Reached entity.
    pub target: ExploreCounterpart,
    /// Ordered relation steps from the entry to the reached entity.
    pub steps: Vec<ExplorePathStep>,
    /// Open diagnostics attached to the reached entity.
    pub open_diagnostics: usize,
    /// Bounded source slice of the reached declaration, when the cited range can be read.
    pub excerpt: Option<super::ExploreExcerpt>,
}

/// Reach count for one hop of the traversal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreBlastDepth {
    /// Hop this row describes, counting from each entry entity.
    pub depth: usize,
    /// Distinct entities first reached at this hop.
    pub entities: usize,
}

/// Aggregate blast radius of the entry entities within the requested hop bound.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploreBlast {
    /// Hop bound the aggregate was computed for.
    pub depth: usize,
    /// Entry entities the traversal started from.
    pub entries: usize,
    /// Distinct entities reached beyond the entry set.
    pub reached: usize,
    /// Distinct relations that carried the traversal to a new entity.
    pub relations: usize,
    /// Relation edges expanded while traversing, disclosed against the visit budget.
    pub edge_visits: usize,
    /// Open diagnostics attached to the entry set or to anything it reaches.
    pub open_diagnostics: usize,
    /// Per-hop reach counts, ordered by hop.
    pub by_depth: Vec<ExploreBlastDepth>,
    /// Files the entry set and everything it reaches are anchored in, sorted.
    pub files: Vec<String>,
    /// Files left out of `files` by the bounded list.
    pub files_omitted: usize,
}

/// Traversal result consumed by the explore report builder.
pub(super) struct ExploreTraversal {
    pub(super) paths: Vec<ExplorePath>,
    pub(super) blast: ExploreBlast,
    pub(super) paths_truncated: usize,
    pub(super) notes: Vec<String>,
}

/// One directed view of a relation, from the node being expanded towards its neighbour.
#[derive(Debug, Clone, Copy)]
struct FlowEdge<'a> {
    relation: &'a Relation,
    next: &'a str,
    forward: bool,
}

/// One multi-hop chain discovered during traversal, before the path budget is applied.
struct FoundPath<'a> {
    origin: String,
    depth: usize,
    target_key: String,
    target: &'a str,
    steps: Vec<ExplorePathStep>,
}

/// Snapshot index the traversal reads instead of scanning relations for every node.
struct FlowGraph<'a> {
    adjacency: HashMap<&'a str, Vec<FlowEdge<'a>>>,
    entities: HashMap<&'a str, &'a Entity>,
    diagnostics: HashMap<&'a str, usize>,
}

impl<'a> FlowGraph<'a> {
    fn new(snapshot: &'a CanonicalSnapshot) -> Self {
        let mut entities: HashMap<&'a str, &'a Entity> = HashMap::new();
        for entity in &snapshot.entities {
            entities.insert(entity.id.0.as_str(), entity);
        }

        let mut diagnostics: HashMap<&'a str, usize> = HashMap::new();
        for diagnostic in &snapshot.diagnostics {
            if diagnostic.status != DiagnosticStatus::Open {
                continue;
            }
            for entity in &diagnostic.entities {
                *diagnostics.entry(entity.0.as_str()).or_default() += 1;
            }
        }

        let mut adjacency: HashMap<&'a str, Vec<FlowEdge<'a>>> = HashMap::new();
        for relation in &snapshot.relations {
            if !code_flow(&relation.kind) {
                continue;
            }
            adjacency
                .entry(relation.from.0.as_str())
                .or_default()
                .push(FlowEdge {
                    relation,
                    next: relation.to.0.as_str(),
                    forward: true,
                });
            adjacency
                .entry(relation.to.0.as_str())
                .or_default()
                .push(FlowEdge {
                    relation,
                    next: relation.from.0.as_str(),
                    forward: false,
                });
        }
        for edges in adjacency.values_mut() {
            edges.sort_by_key(|edge| flow_sort_key(&entities, edge));
        }

        Self {
            adjacency,
            entities,
            diagnostics,
        }
    }

    fn entity(&self, id: &str) -> Option<&'a Entity> {
        self.entities.get(id).copied()
    }

    /// Compact identity of an entity, falling back to the raw id when the snapshot lacks it.
    fn counterpart(&self, id: &str) -> ExploreCounterpart {
        self.entity(id).map_or_else(
            || ExploreCounterpart {
                stable_key: id.to_string(),
                kind: "unknown".to_string(),
                name: id.to_string(),
                source: None,
            },
            |entity| ExploreCounterpart {
                stable_key: entity.stable_key.0.clone(),
                kind: label(&entity.kind),
                name: entity.name.clone(),
                source: entity.source.clone(),
            },
        )
    }

    fn stable_key(&self, id: &str) -> String {
        self.entity(id)
            .map_or_else(|| id.to_string(), |entity| entity.stable_key.0.clone())
    }

    fn step(&self, edge: &FlowEdge<'a>) -> ExplorePathStep {
        let evidence = edge.relation.evidence.first();
        ExplorePathStep {
            relation: label(&edge.relation.kind),
            status: label(&edge.relation.status),
            confidence: edge.relation.confidence,
            direction: if edge.forward { "forward" } else { "backward" }.to_string(),
            from: self.endpoint(edge.relation.from.0.as_str()),
            to: self.endpoint(edge.relation.to.0.as_str()),
            source_file: evidence.and_then(|evidence| evidence.source_file.clone()),
            line_start: evidence.and_then(|evidence| evidence.line_start),
        }
    }

    fn endpoint(&self, id: &str) -> ExplorePathEndpoint {
        self.entity(id).map_or_else(
            || ExplorePathEndpoint {
                entity_id: id.to_string(),
                stable_key: id.to_string(),
                name: id.to_string(),
            },
            |entity| ExplorePathEndpoint {
                entity_id: entity.id.0.clone(),
                stable_key: entity.stable_key.0.clone(),
                name: entity.name.clone(),
            },
        )
    }

    fn open_diagnostics(&self, id: &str) -> usize {
        self.diagnostics.get(id).copied().unwrap_or_default()
    }

    fn files_of(&self, id: &str, files: &mut BTreeSet<String>) {
        let Some(entity) = self.entity(id) else {
            return;
        };
        if let Some(source) = &entity.source {
            files.insert(source.path.clone());
        }
        for ownership in &entity.ownership {
            files.insert(ownership.source_file.clone());
        }
    }
}

/// Deterministic adjacency order: relation kind, then counterpart identity, then relation id.
fn flow_sort_key<'a>(
    entities: &HashMap<&'a str, &'a Entity>,
    edge: &FlowEdge<'a>,
) -> (String, String, String) {
    let counterpart = match entities.get(edge.next) {
        Some(entity) => entity.stable_key.0.clone(),
        None => edge.next.to_string(),
    };
    (
        label(&edge.relation.kind),
        counterpart,
        edge.relation.id.0.clone(),
    )
}

/// Relation kinds that carry code influence from one canonical entity to another.
///
/// Contract-support and diagnostic edges are excluded on purpose: this traversal is a code-flow
/// walk, and `ath impact` stays the authoritative deep blast-radius query.
fn code_flow(kind: &RelationKind) -> bool {
    matches!(
        kind,
        RelationKind::Calls
            | RelationKind::Imports
            | RelationKind::Contains
            | RelationKind::Extends
            | RelationKind::Implements
            | RelationKind::ImplementedBy
            | RelationKind::TestedBy
            | RelationKind::CoveredByTest
            | RelationKind::Documents
    )
}

/// One traversal state: the node to expand, its hop number, and the chain that got there.
struct State<'a> {
    at: &'a str,
    hop: usize,
    steps: Vec<ExplorePathStep>,
}

/// Explores the code-flow closure of `entries` up to `depth` hops.
///
/// Single-hop chains are not repeated here: they are already the one-hop `outgoing` and `incoming`
/// links of each entry. Reported chains start at two hops, so a request with a depth below two
/// returns blast counts and no chains.
pub(super) fn explore_flow<'a>(
    snapshot: &'a CanonicalSnapshot,
    entries: &[&'a Entity],
    depth: usize,
    budget: usize,
) -> ExploreTraversal {
    let graph = FlowGraph::new(snapshot);
    let mut notes = Vec::new();
    let mut found: Vec<FoundPath<'a>> = Vec::new();
    let mut reached: HashSet<&str> = HashSet::new();
    let mut shortest_depths: HashMap<&str, usize> = HashMap::new();
    let mut traversed: HashSet<&str> = HashSet::new();
    let mut visits = 0usize;
    let mut budget_hit = false;

    for entry in entries {
        if budget_hit {
            break;
        }
        let origin = graph.stable_key(entry.id.0.as_str());
        let mut visited: HashSet<&str> = HashSet::new();
        visited.insert(entry.id.0.as_str());
        let mut frontier = VecDeque::new();
        frontier.push_back(State {
            at: entry.id.0.as_str(),
            hop: 1,
            steps: Vec::new(),
        });

        while let Some(state) = frontier.pop_front() {
            if visits >= MAX_EXPLORE_VISITS {
                budget_hit = true;
                break;
            }
            let edges: &[FlowEdge<'a>] = graph
                .adjacency
                .get(state.at)
                .map_or(&[], |edges| edges.as_slice());
            let remaining = MAX_EXPLORE_VISITS - visits;
            let expanded = edges.len().min(remaining);
            visits += expanded;
            for edge in &edges[..expanded] {
                if visited.contains(edge.next) {
                    continue;
                }
                visited.insert(edge.next);
                traversed.insert(edge.relation.id.0.as_str());
                reached.insert(edge.next);
                shortest_depths
                    .entry(edge.next)
                    .and_modify(|shortest| *shortest = (*shortest).min(state.hop))
                    .or_insert(state.hop);

                let mut steps = state.steps.clone();
                steps.push(graph.step(edge));
                if state.hop >= 2 {
                    found.push(FoundPath {
                        origin: origin.clone(),
                        depth: state.hop,
                        target_key: graph.stable_key(edge.next),
                        target: edge.next,
                        steps: steps.clone(),
                    });
                }
                if state.hop < depth {
                    frontier.push_back(State {
                        at: edge.next,
                        hop: state.hop + 1,
                        steps,
                    });
                }
            }
            if expanded < edges.len() {
                budget_hit = true;
                break;
            }
        }
    }

    for entry in entries {
        reached.remove(entry.id.0.as_str());
        shortest_depths.remove(entry.id.0.as_str());
    }
    let mut by_depth: BTreeMap<usize, usize> = BTreeMap::new();
    for depth in shortest_depths.into_values() {
        *by_depth.entry(depth).or_default() += 1;
    }

    found.sort_by_key(|path| (path.origin.clone(), path.depth, path.target_key.clone()));
    let paths_truncated = found.len().saturating_sub(budget);
    let paths = found
        .into_iter()
        .take(budget)
        .map(|found| ExplorePath {
            origin: found.origin,
            depth: found.depth,
            target: graph.counterpart(found.target),
            steps: found.steps,
            open_diagnostics: graph.open_diagnostics(found.target),
            excerpt: None,
        })
        .collect::<Vec<_>>();

    let mut files = BTreeSet::new();
    let mut affected: HashSet<&str> = HashSet::new();
    for entry in entries {
        graph.files_of(entry.id.0.as_str(), &mut files);
        affected.insert(entry.id.0.as_str());
    }
    for id in &reached {
        graph.files_of(id, &mut files);
        affected.insert(*id);
    }
    let open_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.status == DiagnosticStatus::Open
                && diagnostic
                    .entities
                    .iter()
                    .any(|entity| affected.contains(entity.0.as_str()))
        })
        .count();

    let all_files = files.into_iter().collect::<Vec<_>>();
    let files_omitted = all_files.len().saturating_sub(MAX_EXPLORE_FILES);
    if files_omitted > 0 {
        notes.push(format!(
            "{files_omitted} blast-radius files fall outside the bounded file list"
        ));
    }
    if paths_truncated > 0 {
        notes.push(format!(
            "{paths_truncated} multi-hop chains fall outside the per-request path budget"
        ));
    }
    if budget_hit {
        notes.push(format!(
            "traversal stopped after expanding {MAX_EXPLORE_VISITS} relation edges; deeper chains are not reported"
        ));
    }
    if depth < 2 {
        notes.push("a depth below two hops reports the one-hop neighbourhood only".to_string());
    }

    ExploreTraversal {
        paths,
        blast: ExploreBlast {
            depth,
            entries: entries.len(),
            reached: reached.len(),
            relations: traversed.len(),
            edge_visits: visits,
            open_diagnostics,
            by_depth: by_depth
                .into_iter()
                .map(|(depth, entities)| ExploreBlastDepth { depth, entities })
                .collect(),
            files: all_files
                .into_iter()
                .take(MAX_EXPLORE_FILES)
                .collect::<Vec<_>>(),
            files_omitted,
        },
        paths_truncated,
        notes,
    }
}
