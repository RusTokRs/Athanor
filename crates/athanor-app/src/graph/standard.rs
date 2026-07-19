use std::collections::HashMap;

use anyhow::{Result, bail};
use athanor_core::{CanonicalSnapshot, OperationContext};
use athanor_domain::{Entity, Relation};

use super::model::{
    GRAPH_EXPORT_SCHEMA, GRAPH_HUBS_SCHEMA, GraphCycles, GraphEdge, GraphExport, GraphHub, GraphHubs,
    GraphNode, GraphOmitted, GraphPageRank, GraphPath, GraphRelated,
};

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

pub fn graph_export_to_graphml(export: &GraphExport) -> String {
    let mut output = String::new();
    output.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    output.push('\n');
    output.push_str(r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns">"#);
    output.push('\n');
    output.push_str(
        r#"  <key id="stable_key" for="node" attr.name="stable_key" attr.type="string"/>"#,
    );
    output.push('\n');
    output.push_str(r#"  <key id="kind" for="all" attr.name="kind" attr.type="string"/>"#);
    output.push('\n');
    output.push_str(r#"  <key id="name" for="node" attr.name="name" attr.type="string"/>"#);
    output.push('\n');
    output.push_str(r#"  <key id="source" for="node" attr.name="source" attr.type="string"/>"#);
    output.push('\n');
    output.push_str(r#"  <key id="degree" for="node" attr.name="degree" attr.type="int"/>"#);
    output.push('\n');
    output.push_str(r#"  <key id="status" for="edge" attr.name="status" attr.type="string"/>"#);
    output.push('\n');
    output.push_str(
        r#"  <key id="confidence" for="edge" attr.name="confidence" attr.type="double"/>"#,
    );
    output.push('\n');
    output.push_str(r#"  <key id="evidence" for="edge" attr.name="evidence" attr.type="string"/>"#);
    output.push('\n');
    output.push_str(&format!(
        r#"  <graph id="{}" edgedefault="directed">"#,
        xml_escape(&export.snapshot)
    ));
    output.push('\n');
    for node in &export.nodes {
        output.push_str(&format!(r#"    <node id="{}">"#, xml_escape(&node.id)));
        output.push('\n');
        graphml_data(&mut output, "stable_key", &node.stable_key);
        graphml_data(&mut output, "kind", &node.kind);
        graphml_data(&mut output, "name", &node.name);
        if let Some(source) = &node.source {
            graphml_data(&mut output, "source", source);
        }
        graphml_data(&mut output, "degree", &node.degree.to_string());
        output.push_str("    </node>\n");
    }
    for edge in &export.edges {
        output.push_str(&format!(
            r#"    <edge id="{}" source="{}" target="{}">"#,
            xml_escape(&edge.id),
            xml_escape(&edge.from),
            xml_escape(&edge.to)
        ));
        output.push('\n');
        graphml_data(&mut output, "kind", &edge.kind);
        graphml_data(&mut output, "status", &edge.status);
        graphml_data(&mut output, "confidence", &edge.confidence.to_string());
        if !edge.evidence.is_empty() {
            graphml_data(&mut output, "evidence", &edge.evidence.join(";"));
        }
        output.push_str("    </edge>\n");
    }
    output.push_str("  </graph>\n");
    output.push_str("</graphml>\n");
    output
}

pub fn build_related_graph(
    snapshot: &CanonicalSnapshot,
    stable_key: &str,
    depth: usize,
    max_entities: usize,
    max_relations: usize,
) -> Result<GraphRelated> {
    crate::graph_cooperative::build_related_graph_with_operation_context(
        snapshot,
        stable_key,
        depth,
        max_entities,
        max_relations,
        &pure_operation("graph-related"),
    )
}

pub fn build_shortest_graph_path(
    snapshot: &CanonicalSnapshot,
    from_stable_key: &str,
    to_stable_key: &str,
    max_depth: usize,
    max_visited: usize,
) -> Result<GraphPath> {
    crate::graph_cooperative::build_shortest_graph_path_with_operation_context(
        snapshot,
        from_stable_key,
        to_stable_key,
        max_depth,
        max_visited,
        &pure_operation("graph-path"),
    )
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

#[allow(clippy::too_many_arguments)]
pub fn build_graph_pagerank(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    kind: Option<&str>,
    damping: f64,
    max_iterations: usize,
    tolerance: f64,
    max_relation_ids: usize,
) -> Result<GraphPageRank> {
    crate::graph_cooperative::build_graph_pagerank_with_operation_context(
        snapshot,
        limit,
        kind,
        damping,
        max_iterations,
        tolerance,
        max_relation_ids,
        &pure_operation("graph-pagerank"),
    )
}

pub fn build_graph_cycles(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    max_depth: usize,
    max_starts: usize,
) -> Result<GraphCycles> {
    crate::graph_cooperative::build_graph_cycles_with_operation_context(
        snapshot,
        limit,
        max_depth,
        max_starts,
        &pure_operation("graph-cycles"),
    )
}

fn pure_operation(name: &str) -> OperationContext {
    OperationContext::new(name)
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

fn graphml_data(output: &mut String, key: &str, value: &str) {
    output.push_str(&format!(
        r#"      <data key="{}">{}</data>"#,
        xml_escape(key),
        xml_escape(value)
    ));
    output.push('\n');
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn serialized_name(value: &impl serde::Serialize) -> String {
    let Ok(value) = serde_json::to_value(value) else {
        return "unknown".to_string();
    };
    if let Some(name) = value.as_str() {
        return name.to_string();
    }
    value
        .get("other")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}
