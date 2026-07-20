use std::collections::{BTreeSet, HashMap, VecDeque};

use anyhow::{Result, bail};
use athanor_core::{CanonicalSnapshot, OperationContext, OperationContextCancellation};
use athanor_domain::{Entity, Relation};
use serde::Serialize;

use crate::graph::{
    GRAPH_CYCLES_SCHEMA, GRAPH_PAGERANK_SCHEMA, GRAPH_PATH_SCHEMA, GRAPH_RELATED_SCHEMA,
    GraphCycle, GraphCycles, GraphEdge, GraphNode, GraphPageRank, GraphPageRankEntry,
    GraphPageRankRelationTrace, GraphPath, GraphRelated, GraphRelatedNode,
};

const COOPERATIVE_POLL_INTERVAL: usize = 256;

pub fn build_related_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    stable_key: &str,
    depth: usize,
    max_entities: usize,
    max_relations: usize,
    operation: &OperationContext,
) -> Result<GraphRelated> {
    let mut poller = OperationPoller::new(operation, COOPERATIVE_POLL_INTERVAL)?;
    let report = build_related_graph_inner(
        snapshot,
        stable_key,
        depth,
        max_entities,
        max_relations,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(report)
}

pub fn build_shortest_graph_path_with_operation_context(
    snapshot: &CanonicalSnapshot,
    from_stable_key: &str,
    to_stable_key: &str,
    max_depth: usize,
    max_visited: usize,
    operation: &OperationContext,
) -> Result<GraphPath> {
    let mut poller = OperationPoller::new(operation, COOPERATIVE_POLL_INTERVAL)?;
    let report = build_shortest_graph_path_inner(
        snapshot,
        from_stable_key,
        to_stable_key,
        max_depth,
        max_visited,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
pub fn build_graph_pagerank_with_operation_context(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    kind: Option<&str>,
    damping: f64,
    max_iterations: usize,
    tolerance: f64,
    max_relation_ids: usize,
    operation: &OperationContext,
) -> Result<GraphPageRank> {
    let mut poller = OperationPoller::new(operation, COOPERATIVE_POLL_INTERVAL)?;
    let report = build_graph_pagerank_inner(
        snapshot,
        limit,
        kind,
        damping,
        max_iterations,
        tolerance,
        max_relation_ids,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(report)
}

pub fn build_graph_cycles_with_operation_context(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    max_depth: usize,
    max_starts: usize,
    operation: &OperationContext,
) -> Result<GraphCycles> {
    let mut poller = OperationPoller::new(operation, COOPERATIVE_POLL_INTERVAL)?;
    let report = build_graph_cycles_inner(snapshot, limit, max_depth, max_starts, &mut poller)?;
    poller.finish()?;
    Ok(report)
}

struct OperationPoller<'a> {
    operation: &'a OperationContext,
    interval: usize,
    remaining: usize,
}

impl<'a> OperationPoller<'a> {
    fn new(operation: &'a OperationContext, interval: usize) -> Result<Self> {
        operation.check_active().map_err(anyhow::Error::new)?;
        let interval = interval.max(1);
        Ok(Self {
            operation,
            interval,
            remaining: interval,
        })
    }

    fn step(&mut self) -> Result<()> {
        self.remaining -= 1;
        if self.remaining == 0 {
            self.operation.check_active().map_err(anyhow::Error::new)?;
            self.remaining = self.interval;
        }
        Ok(())
    }

    fn finish(&self) -> Result<()> {
        self.operation.check_active().map_err(anyhow::Error::new)
    }
}

fn build_related_graph_inner(
    snapshot: &CanonicalSnapshot,
    stable_key: &str,
    depth: usize,
    max_entities: usize,
    max_relations: usize,
    poller: &mut OperationPoller<'_>,
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
        poller.step()?;
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
        poller.step()?;
        if current_depth >= depth {
            continue;
        }
        for (_, neighbor) in adjacency
            .get(entity_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            poller.step()?;
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
        snapshot: snapshot_id(snapshot),
        root,
        nodes,
        edges,
        truncated,
    })
}

fn build_shortest_graph_path_inner(
    snapshot: &CanonicalSnapshot,
    from_stable_key: &str,
    to_stable_key: &str,
    max_depth: usize,
    max_visited: usize,
    poller: &mut OperationPoller<'_>,
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
        poller.step()?;
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
        poller.step()?;
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
            poller.step()?;
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
            poller.step()?;
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
        snapshot: snapshot_id(snapshot),
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

#[allow(clippy::too_many_arguments)]
fn build_graph_pagerank_inner(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    kind: Option<&str>,
    damping: f64,
    max_iterations: usize,
    tolerance: f64,
    max_relation_ids: usize,
    poller: &mut OperationPoller<'_>,
) -> Result<GraphPageRank> {
    if limit == 0
        || max_iterations == 0
        || max_relation_ids == 0
        || !(0.0..1.0).contains(&damping)
        || tolerance <= 0.0
        || !tolerance.is_finite()
    {
        bail!(
            "graph pagerank requires positive limits and tolerance, with damping between zero and one"
        );
    }

    let mut entities = snapshot.entities.iter().collect::<Vec<_>>();
    entities.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    let entity_count = entities.len();
    if entity_count == 0 {
        return Ok(GraphPageRank {
            schema: GRAPH_PAGERANK_SCHEMA.to_string(),
            snapshot: snapshot_id(snapshot),
            kind: kind.map(str::to_string),
            damping,
            iterations: 0,
            converged: true,
            entity_count: 0,
            relation_count: 0,
            ranks: Vec::new(),
            omitted: 0,
        });
    }

    let index_by_id = entities
        .iter()
        .enumerate()
        .map(|(index, entity)| (entity.id.0.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut outgoing = vec![Vec::<usize>::new(); entity_count];
    let mut incoming_relations = vec![Vec::<&Relation>::new(); entity_count];
    let mut relation_count = 0;
    for relation in &snapshot.relations {
        poller.step()?;
        let (Some(&from), Some(&to)) = (
            index_by_id.get(relation.from.0.as_str()),
            index_by_id.get(relation.to.0.as_str()),
        ) else {
            continue;
        };
        outgoing[from].push(to);
        incoming_relations[to].push(relation);
        relation_count += 1;
    }
    for targets in &mut outgoing {
        targets.sort_unstable();
    }
    for relations in &mut incoming_relations {
        relations.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    }

    let initial = 1.0 / entity_count as f64;
    let mut scores = vec![initial; entity_count];
    let mut iterations = 0;
    let mut converged = false;
    for iteration in 1..=max_iterations {
        poller.step()?;
        let mut dangling = 0.0;
        for (index, score) in scores.iter().enumerate() {
            poller.step()?;
            if outgoing[index].is_empty() {
                dangling += *score;
            }
        }
        let base = (1.0 - damping) / entity_count as f64 + damping * dangling / entity_count as f64;
        let mut next = vec![base; entity_count];
        for (from, targets) in outgoing.iter().enumerate() {
            poller.step()?;
            if targets.is_empty() {
                continue;
            }
            let contribution = damping * scores[from] / targets.len() as f64;
            for &to in targets {
                poller.step()?;
                next[to] += contribution;
            }
        }
        let mut delta = 0.0;
        for (previous, current) in scores.iter().zip(&next) {
            poller.step()?;
            delta += (previous - current).abs();
        }
        scores = next;
        iterations = iteration;
        if delta <= tolerance {
            converged = true;
            break;
        }
    }

    let degrees = degree_by_id(snapshot);
    let mut ranked = entities
        .iter()
        .enumerate()
        .filter(|(_, entity)| kind.is_none_or(|kind| serialized_name(&entity.kind) == kind))
        .map(|(index, entity)| {
            let incoming_count = incoming_relations[index].len();
            let relation_traces = incoming_relations[index]
                .iter()
                .take(max_relation_ids)
                .map(|relation| GraphPageRankRelationTrace {
                    id: relation.id.0.clone(),
                    kind: serialized_name(&relation.kind),
                    from_entity_id: relation.from.0.clone(),
                    evidence: graph_edge(relation).evidence,
                })
                .collect::<Vec<_>>();
            GraphPageRankEntry {
                rank: 0,
                entity: graph_node(entity, *degrees.get(&entity.id.0).unwrap_or(&0)),
                score: scores[index],
                incoming_relations: relation_traces,
                omitted_incoming_relations: incoming_count.saturating_sub(max_relation_ids),
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.entity.stable_key.cmp(&right.entity.stable_key))
    });
    let matched = ranked.len();
    ranked.truncate(limit);
    for (index, entry) in ranked.iter_mut().enumerate() {
        entry.rank = index + 1;
    }

    Ok(GraphPageRank {
        schema: GRAPH_PAGERANK_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        kind: kind.map(str::to_string),
        damping,
        iterations,
        converged,
        entity_count,
        relation_count,
        omitted: matched.saturating_sub(ranked.len()),
        ranks: ranked,
    })
}

fn build_graph_cycles_inner(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    max_depth: usize,
    max_starts: usize,
    poller: &mut OperationPoller<'_>,
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
        poller.step()?;
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
        poller.step()?;
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
            poller,
        )?;
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
        snapshot: snapshot_id(snapshot),
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
    poller: &mut OperationPoller<'_>,
) -> Result<()> {
    poller.step()?;
    if cycles.len() >= limit {
        *truncated = true;
        return Ok(());
    }
    let Some(outgoing) = adjacency.get(current) else {
        return Ok(());
    };
    if edges.len() >= max_depth {
        if outgoing
            .iter()
            .any(|relation| relation.to.0 == start || !on_path.contains(&relation.to.0))
        {
            *truncated = true;
        }
        return Ok(());
    }

    for relation in outgoing {
        poller.step()?;
        if cycles.len() >= limit {
            *truncated = true;
            return Ok(());
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
            truncated, poller,
        )?;
        edges.pop();
        path.pop();
        on_path.remove(target);
    }
    Ok(())
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

fn snapshot_id(snapshot: &CanonicalSnapshot) -> String {
    snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
}

fn serialized_name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
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

#[cfg(test)]
mod tests {
    use athanor_core::CoreError;
    use athanor_domain::{
        EntityId, EntityKind, RelationId, RelationKind, RelationStatus, SnapshotId, StableKey,
    };
    use serde_json::Value;

    use super::*;

    #[test]
    fn pre_cancelled_pagerank_fails_before_work() {
        let operation = OperationContext::new("graph-pagerank-pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();

        let error = build_graph_pagerank_with_operation_context(
            &CanonicalSnapshot::default(),
            1,
            None,
            0.85,
            10,
            1e-8,
            1,
            &operation,
        )
        .expect_err("cancelled PageRank must fail before graph work");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[test]
    fn cycle_search_observes_cooperative_cancellation() {
        let operation = OperationContext::new("graph-cycle-mid-cancel");
        let cancellation = operation.cancellation_handle().unwrap();
        let snapshot = cycle_snapshot();
        let mut poller = OperationPoller::new(&operation, 1).unwrap();
        cancellation.cancel();

        let error = build_graph_cycles_inner(&snapshot, 10, 8, 10, &mut poller)
            .expect_err("cycle traversal must observe cancellation checkpoint");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[test]
    fn cooperative_path_preserves_successful_result() {
        let operation = OperationContext::new("graph-path-success");
        let snapshot = cycle_snapshot();

        let report = build_shortest_graph_path_with_operation_context(
            &snapshot,
            "entity://a",
            "entity://c",
            4,
            16,
            &operation,
        )
        .unwrap();

        assert!(report.found);
        assert_eq!(report.hops, Some(1));
    }

    fn cycle_snapshot() -> CanonicalSnapshot {
        let entities = ["a", "b", "c"]
            .into_iter()
            .map(|name| Entity {
                id: EntityId(format!("ent_{name}")),
                stable_key: StableKey(format!("entity://{name}")),
                kind: EntityKind::Module,
                name: name.to_string(),
                title: None,
                source: None,
                language: None,
                aliases: Vec::new(),
                ownership: Vec::new(),
                payload: Value::Null,
            })
            .collect::<Vec<_>>();
        let snapshot_id = SnapshotId("snap_graph_cooperative".to_string());
        let relations = [
            ("ab", "a", "b"),
            ("bc", "b", "c"),
            ("ca", "c", "a"),
            ("ac", "a", "c"),
        ]
        .into_iter()
        .map(|(id, from, to)| Relation {
            id: RelationId(format!("rel_{id}")),
            kind: RelationKind::Calls,
            from: EntityId(format!("ent_{from}")),
            to: EntityId(format!("ent_{to}")),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: snapshot_id.clone(),
            payload: Value::Null,
        })
        .collect();

        CanonicalSnapshot {
            snapshot: Some(snapshot_id),
            entities,
            relations,
            ..CanonicalSnapshot::default()
        }
    }
}
