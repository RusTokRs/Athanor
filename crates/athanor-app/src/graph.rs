use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Entity, Relation};
use serde::Serialize;

pub const GRAPH_EXPORT_SCHEMA: &str = "athanor.graph_export.v1";
pub const GRAPH_CYCLES_SCHEMA: &str = "athanor.graph_cycles.v1";
pub const GRAPH_HUBS_SCHEMA: &str = "athanor.graph_hubs.v1";
pub const GRAPH_PATH_SCHEMA: &str = "athanor.graph_path.v1";
pub const GRAPH_RELATED_SCHEMA: &str = "athanor.graph_related.v1";

#[derive(Debug, Clone)]
pub struct GraphExportOptions {
    pub root: PathBuf,
    pub max_entities: usize,
    pub max_relations: usize,
}

#[derive(Debug, Clone)]
pub struct GraphRelatedOptions {
    pub root: PathBuf,
    pub stable_key: String,
    pub depth: usize,
    pub max_entities: usize,
    pub max_relations: usize,
}

#[derive(Debug, Clone)]
pub struct GraphPathOptions {
    pub root: PathBuf,
    pub from_stable_key: String,
    pub to_stable_key: String,
    pub max_depth: usize,
    pub max_visited: usize,
}

#[derive(Debug, Clone)]
pub struct GraphHubsOptions {
    pub root: PathBuf,
    pub limit: usize,
    pub kind: Option<String>,
    pub max_relation_ids: usize,
}

#[derive(Debug, Clone)]
pub struct GraphCyclesOptions {
    pub root: PathBuf,
    pub limit: usize,
    pub max_depth: usize,
    pub max_starts: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphExport {
    pub schema: String,
    pub snapshot: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub omitted: GraphOmitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
    pub degree: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub status: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphOmitted {
    pub nodes: usize,
    pub edges: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphRelated {
    pub schema: String,
    pub snapshot: String,
    pub root: GraphRelatedNode,
    pub nodes: Vec<GraphRelatedNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphRelatedNode {
    #[serde(flatten)]
    pub entity: GraphNode,
    pub distance: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphPath {
    pub schema: String,
    pub snapshot: String,
    pub from: GraphNode,
    pub to: GraphNode,
    pub found: bool,
    pub hops: Option<usize>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub visited: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphHubs {
    pub schema: String,
    pub snapshot: String,
    pub kind: Option<String>,
    pub hubs: Vec<GraphHub>,
    pub omitted: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphHub {
    #[serde(flatten)]
    pub entity: GraphNode,
    pub incoming_degree: usize,
    pub outgoing_degree: usize,
    pub incoming_relation_ids: Vec<String>,
    pub outgoing_relation_ids: Vec<String>,
    pub omitted_incoming_relation_ids: usize,
    pub omitted_outgoing_relation_ids: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphCycles {
    pub schema: String,
    pub snapshot: String,
    pub cycles: Vec<GraphCycle>,
    pub start_entities: usize,
    pub omitted_start_entities: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphCycle {
    pub length: usize,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub async fn export_graph(options: GraphExportOptions) -> Result<GraphExport> {
    if options.max_entities == 0 || options.max_relations == 0 {
        bail!("graph export entity and relation limits must be greater than zero");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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

    Ok(build_graph_export(
        &snapshot,
        options.max_entities,
        options.max_relations,
    ))
}

pub async fn related_graph(options: GraphRelatedOptions) -> Result<GraphRelated> {
    if options.max_entities == 0 || options.max_relations == 0 {
        bail!("graph related entity and relation limits must be greater than zero");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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

    build_related_graph(
        &snapshot,
        &options.stable_key,
        options.depth,
        options.max_entities,
        options.max_relations,
    )
}

pub async fn shortest_graph_path(options: GraphPathOptions) -> Result<GraphPath> {
    if options.max_visited == 0 {
        bail!("graph path max visited limit must be greater than zero");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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

    build_shortest_graph_path(
        &snapshot,
        &options.from_stable_key,
        &options.to_stable_key,
        options.max_depth,
        options.max_visited,
    )
}

pub async fn graph_hubs(options: GraphHubsOptions) -> Result<GraphHubs> {
    if options.limit == 0 || options.max_relation_ids == 0 {
        bail!("graph hubs limits must be greater than zero");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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

    build_graph_hubs(
        &snapshot,
        options.limit,
        options.kind.as_deref(),
        options.max_relation_ids,
    )
}

pub async fn graph_cycles(options: GraphCyclesOptions) -> Result<GraphCycles> {
    if options.limit == 0 || options.max_depth == 0 || options.max_starts == 0 {
        bail!("graph cycle limits must be greater than zero");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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

    build_graph_cycles(
        &snapshot,
        options.limit,
        options.max_depth,
        options.max_starts,
    )
}

pub fn build_graph_export(
    snapshot: &CanonicalSnapshot,
    max_entities: usize,
    max_relations: usize,
) -> GraphExport {
    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone());
    let degree_by_id = degree_by_id(snapshot);

    let mut entities = snapshot.entities.iter().collect::<Vec<_>>();
    entities.sort_by(|left, right| {
        degree_by_id
            .get(&right.id.0)
            .unwrap_or(&0)
            .cmp(degree_by_id.get(&left.id.0).unwrap_or(&0))
            .then_with(|| left.stable_key.0.cmp(&right.stable_key.0))
    });
    entities.truncate(max_entities);

    let selected_ids = entities
        .iter()
        .map(|entity| entity.id.0.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let nodes = entities
        .iter()
        .map(|entity| graph_node(entity, *degree_by_id.get(&entity.id.0).unwrap_or(&0)))
        .collect::<Vec<_>>();

    let mut relations = snapshot
        .relations
        .iter()
        .filter(|relation| {
            selected_ids.contains(&relation.from.0) && selected_ids.contains(&relation.to.0)
        })
        .collect::<Vec<_>>();
    relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    relations.truncate(max_relations);
    let edges = relations
        .iter()
        .map(|relation| graph_edge(relation))
        .collect::<Vec<_>>();
    let emitted_edges = edges.len();

    GraphExport {
        schema: GRAPH_EXPORT_SCHEMA.to_string(),
        snapshot: snapshot_id,
        nodes,
        edges,
        omitted: GraphOmitted {
            nodes: snapshot.entities.len().saturating_sub(selected_ids.len()),
            edges: snapshot.relations.len().saturating_sub(emitted_edges),
            reason: "graph_export_limits".to_string(),
        },
    }
}

pub fn build_related_graph(
    snapshot: &CanonicalSnapshot,
    stable_key: &str,
    depth: usize,
    max_entities: usize,
    max_relations: usize,
) -> Result<GraphRelated> {
    if max_entities == 0 || max_relations == 0 {
        bail!("graph related entity and relation limits must be greater than zero");
    }

    let entity_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let root_entity = snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == stable_key)
        .ok_or_else(|| {
            anyhow::anyhow!("canonical entity not found for stable key `{stable_key}`")
        })?;
    let degree_by_id = degree_by_id(snapshot);
    let mut adjacency = HashMap::<&str, Vec<(&Relation, &Entity)>>::new();
    for relation in &snapshot.relations {
        if let Some(entity) = entity_by_id.get(relation.to.0.as_str()) {
            adjacency
                .entry(relation.from.0.as_str())
                .or_default()
                .push((relation, entity));
        }
        if let Some(entity) = entity_by_id.get(relation.from.0.as_str()) {
            adjacency
                .entry(relation.to.0.as_str())
                .or_default()
                .push((relation, entity));
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(
            |(left_relation, left_entity), (right_relation, right_entity)| {
                left_entity
                    .stable_key
                    .0
                    .cmp(&right_entity.stable_key.0)
                    .then_with(|| left_relation.id.0.cmp(&right_relation.id.0))
            },
        );
    }

    let mut distances = HashMap::<String, usize>::new();
    distances.insert(root_entity.id.0.clone(), 0);
    let mut queue = VecDeque::from([(root_entity.id.0.clone(), 0)]);
    let mut truncated = false;

    while let Some((entity_id, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        for (_, neighbor) in adjacency
            .get(entity_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if distances.contains_key(&neighbor.id.0) {
                continue;
            }
            if distances.len() >= max_entities {
                truncated = true;
                continue;
            }
            let neighbor_depth = current_depth + 1;
            distances.insert(neighbor.id.0.clone(), neighbor_depth);
            queue.push_back((neighbor.id.0.clone(), neighbor_depth));
        }
    }

    let selected_ids = distances.keys().cloned().collect::<BTreeSet<_>>();
    let mut nodes = selected_ids
        .iter()
        .filter_map(|id| {
            let entity = entity_by_id.get(id.as_str())?;
            Some(GraphRelatedNode {
                entity: graph_node(entity, *degree_by_id.get(id).unwrap_or(&0)),
                distance: *distances.get(id).unwrap_or(&0),
            })
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        left.distance
            .cmp(&right.distance)
            .then_with(|| left.entity.stable_key.cmp(&right.entity.stable_key))
    });

    let mut relations = snapshot
        .relations
        .iter()
        .filter(|relation| {
            selected_ids.contains(&relation.from.0) && selected_ids.contains(&relation.to.0)
        })
        .collect::<Vec<_>>();
    relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    if relations.len() > max_relations {
        truncated = true;
        relations.truncate(max_relations);
    }
    let edges = relations.into_iter().map(graph_edge).collect::<Vec<_>>();
    let root = nodes
        .iter()
        .find(|node| node.entity.id == root_entity.id.0)
        .cloned()
        .expect("root entity must be selected");

    Ok(GraphRelated {
        schema: GRAPH_RELATED_SCHEMA.to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        root,
        nodes,
        edges,
        truncated,
    })
}

pub fn build_shortest_graph_path(
    snapshot: &CanonicalSnapshot,
    from_stable_key: &str,
    to_stable_key: &str,
    max_depth: usize,
    max_visited: usize,
) -> Result<GraphPath> {
    if max_visited == 0 {
        bail!("graph path max visited limit must be greater than zero");
    }

    let entity_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let from_entity = entity_by_stable_key(snapshot, from_stable_key, "source")?;
    let to_entity = entity_by_stable_key(snapshot, to_stable_key, "target")?;
    let degree_by_id = degree_by_id(snapshot);
    let mut adjacency = HashMap::<&str, Vec<(&Relation, &Entity)>>::new();
    for relation in &snapshot.relations {
        if let Some(entity) = entity_by_id.get(relation.to.0.as_str()) {
            adjacency
                .entry(relation.from.0.as_str())
                .or_default()
                .push((relation, entity));
        }
        if let Some(entity) = entity_by_id.get(relation.from.0.as_str()) {
            adjacency
                .entry(relation.to.0.as_str())
                .or_default()
                .push((relation, entity));
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(
            |(left_relation, left_entity), (right_relation, right_entity)| {
                left_entity
                    .stable_key
                    .0
                    .cmp(&right_entity.stable_key.0)
                    .then_with(|| left_relation.id.0.cmp(&right_relation.id.0))
            },
        );
    }

    let mut queue = VecDeque::from([(from_entity.id.0.clone(), 0)]);
    let mut visited = BTreeSet::from([from_entity.id.0.clone()]);
    let mut parent = HashMap::<String, (String, &Relation)>::new();
    let mut found = from_entity.id == to_entity.id;
    let mut truncated = false;

    while !found {
        let Some((entity_id, depth)) = queue.pop_front() else {
            break;
        };
        if depth >= max_depth {
            if adjacency.get(entity_id.as_str()).is_some_and(|neighbors| {
                neighbors
                    .iter()
                    .any(|(_, neighbor)| !visited.contains(&neighbor.id.0))
            }) {
                truncated = true;
            }
            continue;
        }

        for (relation, neighbor) in adjacency
            .get(entity_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if visited.contains(&neighbor.id.0) {
                continue;
            }
            if visited.len() >= max_visited {
                truncated = true;
                queue.clear();
                break;
            }
            visited.insert(neighbor.id.0.clone());
            parent.insert(neighbor.id.0.clone(), (entity_id.clone(), *relation));
            if neighbor.id == to_entity.id {
                found = true;
                break;
            }
            queue.push_back((neighbor.id.0.clone(), depth + 1));
        }
    }

    let mut path_ids = Vec::new();
    let mut path_relations = Vec::new();
    if found {
        let mut current = to_entity.id.0.clone();
        path_ids.push(current.clone());
        while current != from_entity.id.0 {
            let (previous, relation) = parent
                .get(&current)
                .expect("found graph path must have a complete parent chain");
            path_relations.push(*relation);
            current = previous.clone();
            path_ids.push(current.clone());
        }
        path_ids.reverse();
        path_relations.reverse();
    }

    Ok(GraphPath {
        schema: GRAPH_PATH_SCHEMA.to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        from: graph_node(
            from_entity,
            *degree_by_id.get(&from_entity.id.0).unwrap_or(&0),
        ),
        to: graph_node(to_entity, *degree_by_id.get(&to_entity.id.0).unwrap_or(&0)),
        found,
        hops: found.then_some(path_relations.len()),
        nodes: path_ids
            .iter()
            .filter_map(|id| entity_by_id.get(id.as_str()))
            .map(|entity| graph_node(entity, *degree_by_id.get(&entity.id.0).unwrap_or(&0)))
            .collect(),
        edges: path_relations.into_iter().map(graph_edge).collect(),
        visited: visited.len(),
        truncated,
    })
}

pub fn build_graph_hubs(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    kind: Option<&str>,
    max_relation_ids: usize,
) -> Result<GraphHubs> {
    if limit == 0 || max_relation_ids == 0 {
        bail!("graph hubs limits must be greater than zero");
    }

    let mut incoming = HashMap::<String, Vec<String>>::new();
    let mut outgoing = HashMap::<String, Vec<String>>::new();
    for relation in &snapshot.relations {
        outgoing
            .entry(relation.from.0.clone())
            .or_default()
            .push(relation.id.0.clone());
        incoming
            .entry(relation.to.0.clone())
            .or_default()
            .push(relation.id.0.clone());
    }

    let mut hubs = snapshot
        .entities
        .iter()
        .filter(|entity| kind.is_none_or(|kind| serialized_name(&entity.kind) == kind))
        .filter_map(|entity| {
            let mut incoming_ids = incoming.remove(&entity.id.0).unwrap_or_default();
            let mut outgoing_ids = outgoing.remove(&entity.id.0).unwrap_or_default();
            incoming_ids.sort();
            outgoing_ids.sort();
            let incoming_degree = incoming_ids.len();
            let outgoing_degree = outgoing_ids.len();
            let degree = incoming_degree + outgoing_degree;
            if degree == 0 {
                return None;
            }
            incoming_ids.truncate(max_relation_ids);
            outgoing_ids.truncate(max_relation_ids);
            Some(GraphHub {
                entity: graph_node(entity, degree),
                incoming_degree,
                outgoing_degree,
                omitted_incoming_relation_ids: incoming_degree.saturating_sub(incoming_ids.len()),
                omitted_outgoing_relation_ids: outgoing_degree.saturating_sub(outgoing_ids.len()),
                incoming_relation_ids: incoming_ids,
                outgoing_relation_ids: outgoing_ids,
            })
        })
        .collect::<Vec<_>>();
    hubs.sort_by(|left, right| {
        right
            .entity
            .degree
            .cmp(&left.entity.degree)
            .then_with(|| right.incoming_degree.cmp(&left.incoming_degree))
            .then_with(|| right.outgoing_degree.cmp(&left.outgoing_degree))
            .then_with(|| left.entity.stable_key.cmp(&right.entity.stable_key))
    });
    let matched = hubs.len();
    hubs.truncate(limit);

    Ok(GraphHubs {
        schema: GRAPH_HUBS_SCHEMA.to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        kind: kind.map(str::to_string),
        omitted: matched.saturating_sub(hubs.len()),
        hubs,
    })
}

pub fn build_graph_cycles(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    max_depth: usize,
    max_starts: usize,
) -> Result<GraphCycles> {
    if limit == 0 || max_depth == 0 || max_starts == 0 {
        bail!("graph cycle limits must be greater than zero");
    }

    let entity_by_id = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let degree_by_id = degree_by_id(snapshot);
    let mut adjacency = HashMap::<&str, Vec<&Relation>>::new();
    for relation in &snapshot.relations {
        if entity_by_id.contains_key(relation.from.0.as_str())
            && entity_by_id.contains_key(relation.to.0.as_str())
        {
            adjacency
                .entry(relation.from.0.as_str())
                .or_default()
                .push(relation);
        }
    }
    for relations in adjacency.values_mut() {
        relations.sort_by(|left, right| {
            let left_target = entity_by_id
                .get(left.to.0.as_str())
                .map(|entity| entity.stable_key.0.as_str())
                .unwrap_or_default();
            let right_target = entity_by_id
                .get(right.to.0.as_str())
                .map(|entity| entity.stable_key.0.as_str())
                .unwrap_or_default();
            left_target
                .cmp(right_target)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });
    }

    let mut starts = snapshot
        .entities
        .iter()
        .filter(|entity| adjacency.contains_key(entity.id.0.as_str()))
        .collect::<Vec<_>>();
    starts.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    let total_starts = starts.len();
    starts.truncate(max_starts);

    let mut discovered = BTreeSet::<String>::new();
    let mut raw_cycles = Vec::<(Vec<String>, Vec<&Relation>)>::new();
    let mut truncated = total_starts > starts.len();
    for start in &starts {
        if raw_cycles.len() >= limit {
            truncated = true;
            break;
        }
        let mut path = vec![start.id.0.clone()];
        let mut edges = Vec::new();
        let mut on_path = BTreeSet::from([start.id.0.clone()]);
        search_cycles(
            start.id.0.as_str(),
            start.id.0.as_str(),
            &adjacency,
            &mut path,
            &mut edges,
            &mut on_path,
            max_depth,
            limit,
            &mut discovered,
            &mut raw_cycles,
            &mut truncated,
        );
    }

    let mut cycles = raw_cycles
        .into_iter()
        .map(|(node_ids, relations)| GraphCycle {
            length: relations.len(),
            nodes: node_ids
                .iter()
                .filter_map(|id| entity_by_id.get(id.as_str()))
                .map(|entity| graph_node(entity, *degree_by_id.get(&entity.id.0).unwrap_or(&0)))
                .collect(),
            edges: relations.into_iter().map(graph_edge).collect(),
        })
        .collect::<Vec<_>>();
    cycles.sort_by(|left, right| {
        left.length.cmp(&right.length).then_with(|| {
            cycle_key_from_edges(&left.edges).cmp(&cycle_key_from_edges(&right.edges))
        })
    });
    cycles.truncate(limit);

    Ok(GraphCycles {
        schema: GRAPH_CYCLES_SCHEMA.to_string(),
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
        cycles,
        start_entities: starts.len(),
        omitted_start_entities: total_starts.saturating_sub(starts.len()),
        truncated,
    })
}

#[allow(clippy::too_many_arguments)]
fn search_cycles<'a>(
    start: &str,
    current: &str,
    adjacency: &HashMap<&str, Vec<&'a Relation>>,
    path: &mut Vec<String>,
    edges: &mut Vec<&'a Relation>,
    on_path: &mut BTreeSet<String>,
    max_depth: usize,
    limit: usize,
    discovered: &mut BTreeSet<String>,
    cycles: &mut Vec<(Vec<String>, Vec<&'a Relation>)>,
    truncated: &mut bool,
) {
    if cycles.len() >= limit {
        *truncated = true;
        return;
    }
    let Some(outgoing) = adjacency.get(current) else {
        return;
    };
    if edges.len() >= max_depth {
        if outgoing
            .iter()
            .any(|relation| relation.to.0 == start || !on_path.contains(&relation.to.0))
        {
            *truncated = true;
        }
        return;
    }

    for relation in outgoing {
        if cycles.len() >= limit {
            *truncated = true;
            return;
        }
        let target = relation.to.0.as_str();
        if target == start {
            let mut cycle_edges = edges.clone();
            cycle_edges.push(*relation);
            let key = canonical_cycle_key(&cycle_edges);
            if discovered.insert(key) {
                cycles.push((path.clone(), cycle_edges));
            }
            continue;
        }
        if on_path.contains(target) {
            continue;
        }
        on_path.insert(target.to_string());
        path.push(target.to_string());
        edges.push(*relation);
        search_cycles(
            start, target, adjacency, path, edges, on_path, max_depth, limit, discovered, cycles,
            truncated,
        );
        edges.pop();
        path.pop();
        on_path.remove(target);
    }
}

fn canonical_cycle_key(relations: &[&Relation]) -> String {
    let ids = relations
        .iter()
        .map(|relation| relation.id.0.as_str())
        .collect::<Vec<_>>();
    minimum_rotation(&ids)
}

fn cycle_key_from_edges(edges: &[GraphEdge]) -> String {
    let ids = edges
        .iter()
        .map(|edge| edge.id.as_str())
        .collect::<Vec<_>>();
    minimum_rotation(&ids)
}

fn minimum_rotation(ids: &[&str]) -> String {
    (0..ids.len())
        .map(|offset| {
            (0..ids.len())
                .map(|index| ids[(offset + index) % ids.len()])
                .collect::<Vec<_>>()
                .join("\u{1f}")
        })
        .min()
        .unwrap_or_default()
}

fn entity_by_stable_key<'a>(
    snapshot: &'a CanonicalSnapshot,
    stable_key: &str,
    role: &str,
) -> Result<&'a Entity> {
    snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == stable_key)
        .ok_or_else(|| {
            anyhow::anyhow!("canonical {role} entity not found for stable key `{stable_key}`")
        })
}

fn degree_by_id(snapshot: &CanonicalSnapshot) -> HashMap<String, usize> {
    let mut degree_by_id = HashMap::new();
    for relation in &snapshot.relations {
        *degree_by_id.entry(relation.from.0.clone()).or_default() += 1;
        *degree_by_id.entry(relation.to.0.clone()).or_default() += 1;
    }
    degree_by_id
}

fn graph_node(entity: &Entity, degree: usize) -> GraphNode {
    GraphNode {
        id: entity.id.0.clone(),
        stable_key: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity.source.as_ref().map(|source| {
            source.line_start.map_or_else(
                || source.path.clone(),
                |line| format!("{}:{line}", source.path),
            )
        }),
        degree,
    }
}

fn graph_edge(relation: &Relation) -> GraphEdge {
    GraphEdge {
        id: relation.id.0.clone(),
        kind: serialized_name(&relation.kind),
        from: relation.from.0.clone(),
        to: relation.to.0.clone(),
        status: serialized_name(&relation.status),
        confidence: relation.confidence,
        evidence: relation
            .evidence
            .iter()
            .filter_map(|evidence| {
                evidence.source_file.as_ref().map(|path| {
                    evidence
                        .line_start
                        .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
                })
            })
            .collect(),
    }
}

fn serialized_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use athanor_core::CanonicalSnapshot;
    use athanor_domain::{
        EntityId, EntityKind, Evidence, EvidenceStatus, RelationId, RelationKind, RelationStatus,
        SnapshotId, SourceLocation, StableKey,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn exports_bounded_graph_by_degree_then_stable_key() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let doc = entity(
            "ent_doc",
            "doc://docs/api/health.md",
            EntityKind::DocumentationPage,
            "Health API",
        );
        let orphan = entity("ent_orphan", "file://orphan", EntityKind::File, "orphan");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![handler.clone(), orphan, doc.clone(), endpoint.clone()],
            relations: vec![
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
            ],
            ..CanonicalSnapshot::default()
        };

        let export = build_graph_export(&snapshot, 3, 1);

        assert_eq!(export.schema, GRAPH_EXPORT_SCHEMA);
        assert_eq!(export.snapshot, "snap_test");
        assert_eq!(
            export
                .nodes
                .iter()
                .map(|node| node.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "api://GET:/health",
                "doc://docs/api/health.md",
                "rust://src/lib.rs#health"
            ]
        );
        assert_eq!(export.edges.len(), 1);
        assert_eq!(export.edges[0].id, "rel_docs");
        assert_eq!(export.edges[0].evidence, vec!["docs/api/health.md:1"]);
        assert_eq!(export.omitted.nodes, 1);
        assert_eq!(export.omitted.edges, 1);
    }

    #[test]
    fn explores_related_entities_by_bounded_distance() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let doc = entity(
            "ent_doc",
            "doc://docs/api/health.md",
            EntityKind::DocumentationPage,
            "Health API",
        );
        let schema = entity(
            "ent_schema",
            "api-schema://Health",
            EntityKind::ApiSchema,
            "Health",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![
                handler.clone(),
                schema.clone(),
                doc.clone(),
                endpoint.clone(),
            ],
            relations: vec![
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
                relation(
                    "rel_schema",
                    RelationKind::SchemaForResponse,
                    &endpoint,
                    &schema,
                ),
            ],
            ..CanonicalSnapshot::default()
        };

        let related = build_related_graph(&snapshot, "api://GET:/health", 1, 3, 10).unwrap();

        assert_eq!(related.schema, GRAPH_RELATED_SCHEMA);
        assert_eq!(related.root.entity.stable_key, "api://GET:/health");
        assert_eq!(
            related
                .nodes
                .iter()
                .map(|node| (node.distance, node.entity.stable_key.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (0, "api://GET:/health"),
                (1, "api-schema://Health"),
                (1, "doc://docs/api/health.md"),
            ]
        );
        assert_eq!(
            related
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            vec!["rel_docs", "rel_schema"]
        );
        assert!(related.truncated);
    }

    #[test]
    fn reports_missing_related_graph_root() {
        let error =
            build_related_graph(&CanonicalSnapshot::default(), "missing://entity", 1, 10, 10)
                .unwrap_err();

        assert!(error.to_string().contains("missing://entity"));
    }

    #[test]
    fn finds_deterministic_shortest_graph_path() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let doc = entity(
            "ent_doc",
            "doc://docs/api/health.md",
            EntityKind::DocumentationPage,
            "Health API",
        );
        let schema = entity(
            "ent_schema",
            "api-schema://Health",
            EntityKind::ApiSchema,
            "Health",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![
                schema.clone(),
                handler.clone(),
                endpoint.clone(),
                doc.clone(),
            ],
            relations: vec![
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
                relation(
                    "rel_schema",
                    RelationKind::SchemaForResponse,
                    &endpoint,
                    &schema,
                ),
            ],
            ..CanonicalSnapshot::default()
        };

        let path = build_shortest_graph_path(
            &snapshot,
            "doc://docs/api/health.md",
            "rust://src/lib.rs#health",
            3,
            10,
        )
        .unwrap();

        assert_eq!(path.schema, GRAPH_PATH_SCHEMA);
        assert!(path.found);
        assert_eq!(path.hops, Some(2));
        assert_eq!(
            path.nodes
                .iter()
                .map(|node| node.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "doc://docs/api/health.md",
                "api://GET:/health",
                "rust://src/lib.rs#health"
            ]
        );
        assert_eq!(
            path.edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            vec!["rel_docs", "rel_impl"]
        );
        assert!(!path.truncated);
    }

    #[test]
    fn reports_truncated_graph_path_search() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let doc = entity(
            "ent_doc",
            "doc://docs/api/health.md",
            EntityKind::DocumentationPage,
            "Health API",
        );
        let snapshot = CanonicalSnapshot {
            entities: vec![handler.clone(), endpoint.clone(), doc.clone()],
            relations: vec![
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
            ],
            ..CanonicalSnapshot::default()
        };

        let path = build_shortest_graph_path(
            &snapshot,
            "doc://docs/api/health.md",
            "rust://src/lib.rs#health",
            1,
            10,
        )
        .unwrap();

        assert!(!path.found);
        assert_eq!(path.hops, None);
        assert!(path.nodes.is_empty());
        assert!(path.edges.is_empty());
        assert!(path.truncated);
    }

    #[test]
    fn ranks_graph_hubs_and_bounds_relation_ids() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let doc = entity(
            "ent_doc",
            "doc://docs/api/health.md",
            EntityKind::DocumentationPage,
            "Health API",
        );
        let schema = entity(
            "ent_schema",
            "api-schema://Health",
            EntityKind::ApiSchema,
            "Health",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![
                handler.clone(),
                schema.clone(),
                doc.clone(),
                endpoint.clone(),
            ],
            relations: vec![
                relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
                relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
                relation(
                    "rel_schema",
                    RelationKind::SchemaForResponse,
                    &endpoint,
                    &schema,
                ),
            ],
            ..CanonicalSnapshot::default()
        };

        let report = build_graph_hubs(&snapshot, 2, None, 1).unwrap();

        assert_eq!(report.schema, GRAPH_HUBS_SCHEMA);
        assert_eq!(report.hubs.len(), 2);
        assert_eq!(report.omitted, 2);
        assert_eq!(report.hubs[0].entity.stable_key, "api://GET:/health");
        assert_eq!(report.hubs[0].entity.degree, 3);
        assert_eq!(report.hubs[0].incoming_degree, 1);
        assert_eq!(report.hubs[0].outgoing_degree, 2);
        assert_eq!(report.hubs[0].incoming_relation_ids, vec!["rel_docs"]);
        assert_eq!(report.hubs[0].outgoing_relation_ids, vec!["rel_impl"]);
        assert_eq!(report.hubs[0].omitted_outgoing_relation_ids, 1);
    }

    #[test]
    fn filters_graph_hubs_by_serialized_entity_kind() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health",
            EntityKind::ApiEndpoint,
            "health",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let snapshot = CanonicalSnapshot {
            entities: vec![handler.clone(), endpoint.clone()],
            relations: vec![relation(
                "rel_impl",
                RelationKind::ImplementedBy,
                &endpoint,
                &handler,
            )],
            ..CanonicalSnapshot::default()
        };

        let report = build_graph_hubs(&snapshot, 10, Some("function"), 10).unwrap();

        assert_eq!(report.kind.as_deref(), Some("function"));
        assert_eq!(report.hubs.len(), 1);
        assert_eq!(report.hubs[0].entity.stable_key, "rust://src/lib.rs#health");
    }

    #[test]
    fn finds_unique_directed_graph_cycles() {
        let first = entity("ent_first", "symbol://first", EntityKind::Function, "first");
        let second = entity(
            "ent_second",
            "symbol://second",
            EntityKind::Function,
            "second",
        );
        let third = entity("ent_third", "symbol://third", EntityKind::Function, "third");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![third.clone(), first.clone(), second.clone()],
            relations: vec![
                relation("rel_first_second", RelationKind::Calls, &first, &second),
                relation("rel_second_third", RelationKind::Calls, &second, &third),
                relation("rel_third_first", RelationKind::Calls, &third, &first),
            ],
            ..CanonicalSnapshot::default()
        };

        let report = build_graph_cycles(&snapshot, 10, 4, 10).unwrap();

        assert_eq!(report.schema, GRAPH_CYCLES_SCHEMA);
        assert_eq!(report.cycles.len(), 1);
        assert_eq!(report.cycles[0].length, 3);
        assert_eq!(
            report.cycles[0]
                .nodes
                .iter()
                .map(|node| node.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["symbol://first", "symbol://second", "symbol://third"]
        );
        assert_eq!(
            report.cycles[0]
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            vec!["rel_first_second", "rel_second_third", "rel_third_first"]
        );
        assert!(!report.truncated);
    }

    #[test]
    fn marks_cycle_search_truncated_by_depth_and_start_limit() {
        let first = entity("ent_first", "symbol://first", EntityKind::Function, "first");
        let second = entity(
            "ent_second",
            "symbol://second",
            EntityKind::Function,
            "second",
        );
        let third = entity("ent_third", "symbol://third", EntityKind::Function, "third");
        let snapshot = CanonicalSnapshot {
            entities: vec![first.clone(), second.clone(), third.clone()],
            relations: vec![
                relation("rel_first_second", RelationKind::Calls, &first, &second),
                relation("rel_second_third", RelationKind::Calls, &second, &third),
                relation("rel_third_first", RelationKind::Calls, &third, &first),
            ],
            ..CanonicalSnapshot::default()
        };

        let report = build_graph_cycles(&snapshot, 10, 2, 1).unwrap();

        assert!(report.cycles.is_empty());
        assert_eq!(report.start_entities, 1);
        assert_eq!(report.omitted_start_entities, 2);
        assert!(report.truncated);
    }

    fn entity(id: &str, stable_key: &str, kind: EntityKind, name: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: stable_key
                    .strip_prefix("doc://")
                    .unwrap_or("src/lib.rs")
                    .to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind,
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: vec![Evidence {
                source_file: Some("docs/api/health.md".to_string()),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }
    }
}
