use std::collections::{BTreeSet, HashMap};

use anyhow::{Result, bail};
use athanor_core::{CanonicalSnapshot, OperationContext, OperationContextCancellation};
use athanor_domain::{Diagnostic, DiagnosticKind, Entity, Relation, RelationKind};

use crate::graph::{
    GraphOmitted, RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA, RUSTOK_FBA_MODULE_GRAPH_SCHEMA,
    RUSTOK_FBA_PORT_GRAPH_SCHEMA, RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA,
    RUSTOK_FFA_SURFACE_GRAPH_SCHEMA, RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA,
    RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA, RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA,
    RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA, RustokFbaGraph, RustokFbaGraphEdge,
    RustokFbaGraphNode, RustokFfaGraph, RustokFfaGraphEdge, RustokFfaGraphNode,
    RustokPageBuilderGraph, RustokPageBuilderGraphEdge, RustokPageBuilderGraphNode,
};

const RUSTOK_GRAPH_POLL_INTERVAL: usize = 64;

pub(crate) fn build_rustok_ffa_surface_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: &str,
    surface: &str,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokFfaGraph> {
    build_ffa_surface_with_checkpoint(
        snapshot,
        module,
        surface,
        max_nodes,
        max_edges,
        || operation.check_active().map_err(anyhow::Error::new),
    )
}

pub(crate) fn build_rustok_ffa_violations_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokFfaGraph> {
    build_ffa_violations_with_checkpoint(
        snapshot,
        module,
        surface,
        max_nodes,
        max_edges,
        || operation.check_active().map_err(anyhow::Error::new),
    )
}

pub(crate) fn build_rustok_fba_module_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    build_fba_module_with_checkpoint(snapshot, module, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_fba_port_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: &str,
    port: &str,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    build_fba_port_with_checkpoint(snapshot, module, port, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_fba_dependencies_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    build_fba_dependencies_with_checkpoint(snapshot, module, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_fba_violations_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    build_fba_violations_with_checkpoint(snapshot, module, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_page_builder_provider_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokPageBuilderGraph> {
    build_page_builder_provider_with_checkpoint(snapshot, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_page_builder_consumer_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokPageBuilderGraph> {
    build_page_builder_consumer_with_checkpoint(snapshot, module, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

pub(crate) fn build_rustok_page_builder_violations_graph_with_operation_context(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
    operation: &OperationContext,
) -> Result<RustokPageBuilderGraph> {
    build_page_builder_violations_with_checkpoint(snapshot, module, max_nodes, max_edges, || {
        operation.check_active().map_err(anyhow::Error::new)
    })
}

fn build_ffa_surface_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: &str,
    surface: &str,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFfaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FFA graph limits must be greater than zero");
    }
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let surface_key = format!("ffa_surface://{module}/{surface}");
    let entity_by_id = entity_by_id(snapshot, &mut poller)?;
    let degree_by_id = degree_by_id(snapshot, &mut poller)?;
    let surface_entity = find_entity_by_stable_key(snapshot, &surface_key, &mut poller)?
        .ok_or_else(|| anyhow::anyhow!("FFA surface not found for `{surface_key}`"))?;

    let mut selected_ids = BTreeSet::from([surface_entity.id.0.clone()]);
    let mut edges = Vec::new();
    for relation in &snapshot.relations {
        poller.step()?;
        if relation.from == surface_entity.id
            && matches!(relation.kind, RelationKind::Contains)
            && let Some(layer) = entity_by_id.get(relation.to.0.as_str())
            && layer.stable_key.0.starts_with("ffa_layer://")
        {
            selected_ids.insert(layer.id.0.clone());
            push_ffa_edge(&mut edges, relation, &entity_by_id, &mut poller)?;
            for file_relation in &snapshot.relations {
                poller.step()?;
                if file_relation.from == layer.id
                    && matches!(file_relation.kind, RelationKind::ImplementedBy)
                {
                    selected_ids.insert(file_relation.to.0.clone());
                    push_ffa_edge(
                        &mut edges,
                        file_relation,
                        &entity_by_id,
                        &mut poller,
                    )?;
                }
            }
        }
    }

    let mut nodes = Vec::new();
    for id in &selected_ids {
        poller.step()?;
        if let Some(entity) = entity_by_id.get(id.as_str()) {
            nodes.push(ffa_graph_node(
                entity,
                *degree_by_id.get(&entity.id.0).unwrap_or(&0),
            ));
        }
    }
    poller.checkpoint()?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    edges.sort_by(|left, right| {
        (&left.from, &left.to, &left.kind).cmp(&(&right.from, &right.to, &right.kind))
    });
    let total_nodes = nodes.len();
    let total_edges = edges.len();
    nodes.truncate(max_nodes);
    let retained = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    edges.retain(|edge| {
        retained.contains(edge.from.as_str()) && retained.contains(edge.to.as_str())
    });
    edges.truncate(max_edges);
    let diagnostics = ffa_diagnostics(snapshot, Some(module), Some(surface), &mut poller)?;
    poller.finish()?;

    Ok(RustokFfaGraph {
        schema: RUSTOK_FFA_SURFACE_GRAPH_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        surface: Some(surface_key),
        nodes,
        edges,
        diagnostics,
        omitted: GraphOmitted {
            nodes: total_nodes.saturating_sub(max_nodes.min(total_nodes)),
            edges: total_edges.saturating_sub(max_edges.min(total_edges)),
            reason: "rustok_ffa_graph_limits".to_string(),
        },
    })
}

fn build_ffa_violations_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFfaGraph> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let diagnostics = ffa_diagnostics(snapshot, module, surface, &mut poller)?;
    let entity_by_stable = entity_by_stable(snapshot, &mut poller)?;
    let degree_by_id = degree_by_id(snapshot, &mut poller)?;
    let mut node_keys = BTreeSet::new();
    let mut edges = Vec::new();

    for diagnostic in &diagnostics {
        poller.step()?;
        let Some(diag_module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(diag_surface) = diagnostic
            .payload
            .get("surface")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let surface_key = format!("ffa_surface://{diag_module}/{diag_surface}");
        node_keys.insert(surface_key.clone());
        if let Some(role) = diagnostic
            .payload
            .get("role")
            .and_then(serde_json::Value::as_str)
        {
            let layer_key = format!("ffa_layer://{diag_module}/{diag_surface}/{role}");
            node_keys.insert(layer_key.clone());
            edges.push(RustokFfaGraphEdge {
                from: surface_key,
                to: layer_key.clone(),
                kind: "violates".to_string(),
                evidence: diagnostic_evidence(diagnostic, &mut poller)?,
            });
            if let Some(path) = diagnostic
                .payload
                .get("path")
                .and_then(serde_json::Value::as_str)
            {
                let file_key = format!("file://{path}");
                node_keys.insert(file_key.clone());
                edges.push(RustokFfaGraphEdge {
                    from: layer_key,
                    to: file_key,
                    kind: "evidenced_by".to_string(),
                    evidence: diagnostic_evidence(diagnostic, &mut poller)?,
                });
            }
        }
    }

    let total_nodes = node_keys.len();
    let total_edges = edges.len();
    let mut nodes = Vec::new();
    for stable_key in &node_keys {
        poller.step()?;
        if let Some(entity) = entity_by_stable.get(stable_key.as_str()) {
            nodes.push(ffa_graph_node(
                entity,
                *degree_by_id.get(&entity.id.0).unwrap_or(&0),
            ));
        }
    }
    poller.checkpoint()?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    nodes.truncate(max_nodes);
    let retained = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    edges.retain(|edge| {
        retained.contains(edge.from.as_str()) && retained.contains(edge.to.as_str())
    });
    edges.sort_by(|left, right| {
        (&left.from, &left.to, &left.kind).cmp(&(&right.from, &right.to, &right.kind))
    });
    edges.truncate(max_edges);
    poller.finish()?;

    Ok(RustokFfaGraph {
        schema: RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        surface: module
            .zip(surface)
            .map(|(module, surface)| format!("ffa_surface://{module}/{surface}")),
        nodes,
        edges,
        diagnostics,
        omitted: GraphOmitted {
            nodes: total_nodes.saturating_sub(max_nodes.min(total_nodes)),
            edges: total_edges.saturating_sub(max_edges.min(total_edges)),
            reason: "rustok_ffa_graph_limits".to_string(),
        },
    })
}

fn build_fba_module_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFbaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let module_key = format!("fba_module://{module}");
    let module_segment = format!("://{module}/");
    let entity_by_id = entity_by_id(snapshot, &mut poller)?;
    let module_entity = find_entity_by_stable_key(snapshot, &module_key, &mut poller)?
        .ok_or_else(|| anyhow::anyhow!("FBA module not found for `{module_key}`"))?;
    let mut selected_ids = BTreeSet::from([module_entity.id.0.clone()]);
    let mut edges = Vec::new();

    for relation in &snapshot.relations {
        poller.step()?;
        let from = entity_by_id.get(relation.from.0.as_str());
        let to = entity_by_id.get(relation.to.0.as_str());
        let touches_module = relation.from == module_entity.id
            || relation.to == module_entity.id
            || {
                let from_key = from
                    .map(|entity| entity.stable_key.0.as_str())
                    .unwrap_or("");
                let to_key = to.map(|entity| entity.stable_key.0.as_str()).unwrap_or("");
                from_key.contains(&module_segment)
                    || to_key.contains(&module_segment)
                    || from_key == module_key
                    || to_key == module_key
            };
        if touches_module
            && (from.is_some_and(|entity| is_fba_entity(entity))
                || to.is_some_and(|entity| is_fba_entity(entity)))
        {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_fba_edge(&mut edges, relation, &entity_by_id, &mut poller)?;
        }
    }

    let diagnostics = fba_diagnostics(snapshot, Some(module), &mut poller)?;
    let graph = fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_MODULE_GRAPH_SCHEMA,
        Some(module_key),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn build_fba_port_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: &str,
    port: &str,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFbaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let port_key = format!("fba_port://{module}/{port}");
    let entity_by_id = entity_by_id(snapshot, &mut poller)?;
    let port_entity = find_entity_by_stable_key(snapshot, &port_key, &mut poller)?
        .ok_or_else(|| anyhow::anyhow!("FBA port not found for `{port_key}`"))?;
    let mut selected_ids = BTreeSet::from([port_entity.id.0.clone()]);
    let mut edges = Vec::new();

    for relation in &snapshot.relations {
        poller.step()?;
        if relation.from == port_entity.id || relation.to == port_entity.id {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_fba_edge(&mut edges, relation, &entity_by_id, &mut poller)?;
        }
        if relation.from == port_entity.id
            && let Some(operation) = entity_by_id.get(relation.to.0.as_str())
            && operation.stable_key.0.starts_with("fba_operation://")
        {
            selected_ids.insert(operation.id.0.clone());
        }
    }

    let diagnostics = fba_diagnostics(snapshot, Some(module), &mut poller)?;
    let graph = fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_PORT_GRAPH_SCHEMA,
        Some(port_key),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn build_fba_dependencies_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFbaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let module_key = format!("fba_module://{module}");
    let module_segment = format!("fba_dependency://{module}/");
    let provider_segment = format!("/{module}/");
    let entity_by_id = entity_by_id(snapshot, &mut poller)?;
    let entity_by_stable = entity_by_stable(snapshot, &mut poller)?;
    let mut selected_ids = BTreeSet::new();
    if let Some(entity) = entity_by_stable.get(module_key.as_str()) {
        selected_ids.insert(entity.id.0.clone());
    }
    let mut edges = Vec::new();
    for relation in &snapshot.relations {
        poller.step()?;
        let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
            continue;
        };
        let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
            continue;
        };
        if !is_fba_entity(from) || !is_fba_entity(to) {
            continue;
        }
        if from.stable_key.0.starts_with(&module_segment)
            || from.stable_key.0.contains(&provider_segment)
            || to.stable_key.0 == module_key
        {
            selected_ids.insert(from.id.0.clone());
            selected_ids.insert(to.id.0.clone());
            push_fba_edge(&mut edges, relation, &entity_by_id, &mut poller)?;
        }
    }

    let diagnostics = fba_diagnostics(snapshot, Some(module), &mut poller)?;
    let graph = fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA,
        Some(module_key),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn build_fba_violations_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokFbaGraph> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let diagnostics = fba_diagnostics(snapshot, module, &mut poller)?;
    let entity_by_stable = entity_by_stable(snapshot, &mut poller)?;
    let mut node_keys = BTreeSet::new();
    let mut edges = Vec::new();

    for diagnostic in &diagnostics {
        poller.step()?;
        let Some(diag_module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let module_key = format!("fba_module://{diag_module}");
        node_keys.insert(module_key.clone());
        let evidence_root = if let Some(port) = diagnostic
            .payload
            .get("port")
            .and_then(serde_json::Value::as_str)
        {
            let port_key = format!("fba_port://{diag_module}/{port}");
            node_keys.insert(port_key.clone());
            edges.push(RustokFbaGraphEdge {
                from: module_key,
                to: port_key.clone(),
                kind: "violates".to_string(),
                evidence: diagnostic_evidence(diagnostic, &mut poller)?,
            });
            port_key
        } else {
            module_key
        };
        let mut evidence_paths = BTreeSet::new();
        for evidence in &diagnostic.evidence {
            poller.step()?;
            if let Some(path) = evidence.source_file.as_deref() {
                evidence_paths.insert(path);
            }
        }
        if let Some(path) = diagnostic
            .payload
            .get("path")
            .and_then(serde_json::Value::as_str)
        {
            evidence_paths.insert(path);
        }
        for path in evidence_paths {
            poller.step()?;
            let file_key = format!("file://{path}");
            node_keys.insert(file_key.clone());
            edges.push(RustokFbaGraphEdge {
                from: evidence_root.clone(),
                to: file_key,
                kind: "evidenced_by".to_string(),
                evidence: diagnostic_evidence(diagnostic, &mut poller)?,
            });
        }
    }

    let mut selected_ids = BTreeSet::new();
    for key in node_keys {
        poller.step()?;
        if let Some(entity) = entity_by_stable.get(key.as_str()) {
            selected_ids.insert(entity.id.0.clone());
        }
    }
    let graph = fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA,
        module.map(|module| format!("fba_module://{module}")),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn build_page_builder_provider_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokPageBuilderGraph> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let provider_key = "page_builder_provider://page_builder";
    let entity_by_stable = entity_by_stable(snapshot, &mut poller)?;
    let provider = entity_by_stable
        .get(provider_key)
        .ok_or_else(|| anyhow::anyhow!("Page Builder provider not found for `{provider_key}`"))?;
    let mut selected_ids = BTreeSet::from([provider.id.0.clone()]);
    let mut edges = Vec::new();
    let entity_by_id = entity_by_id(snapshot, &mut poller)?;
    for relation in &snapshot.relations {
        poller.step()?;
        let from = entity_by_id.get(relation.from.0.as_str());
        let to = entity_by_id.get(relation.to.0.as_str());
        let touches = relation.from == provider.id
            || relation.to == provider.id
            || (from.is_some_and(|entity| is_page_builder_entity(entity))
                && to.is_some_and(|entity| is_page_builder_entity(entity))
                && relation
                    .payload
                    .get("schema")
                    .and_then(serde_json::Value::as_str)
                    == Some("rustok.page_builder.relation.v1"));
        if touches {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_page_builder_edge(&mut edges, relation, &entity_by_id, &mut poller)?;
        }
    }
    let diagnostics = page_builder_diagnostics(snapshot, None, &mut poller)?;
    let graph = page_builder_graph_from_selection(
        snapshot,
        RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA,
        Some(provider_key.to_string()),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn build_page_builder_consumer_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokPageBuilderGraph> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let consumer_key = format!("page_builder_consumer://{module}");
    let module_segment = format!("://{module}/");
    let entity_by_id = entity_by_id(snapshot, &mut poller)?;
    let consumer = find_entity_by_stable_key(snapshot, &consumer_key, &mut poller)?
        .ok_or_else(|| anyhow::anyhow!("Page Builder consumer not found for `{consumer_key}`"))?;
    let mut selected_ids = BTreeSet::from([consumer.id.0.clone()]);
    let mut edges = Vec::new();
    for relation in &snapshot.relations {
        poller.step()?;
        let from = entity_by_id.get(relation.from.0.as_str());
        let to = entity_by_id.get(relation.to.0.as_str());
        let touches = relation.from == consumer.id
            || relation.to == consumer.id
            || from.is_some_and(|entity| entity.stable_key.0.contains(&module_segment))
            || to.is_some_and(|entity| entity.stable_key.0.contains(&module_segment));
        if touches
            && (from.is_some_and(|entity| is_page_builder_entity(entity))
                || to.is_some_and(|entity| is_page_builder_entity(entity)))
        {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_page_builder_edge(&mut edges, relation, &entity_by_id, &mut poller)?;
        }
    }
    let diagnostics = page_builder_diagnostics(snapshot, Some(module), &mut poller)?;
    let graph = page_builder_graph_from_selection(
        snapshot,
        RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA,
        Some(consumer_key),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn build_page_builder_violations_with_checkpoint(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
    checkpoint: impl FnMut() -> Result<()>,
) -> Result<RustokPageBuilderGraph> {
    let mut poller = CheckpointPoller::new(checkpoint, RUSTOK_GRAPH_POLL_INTERVAL)?;
    let diagnostics = page_builder_diagnostics(snapshot, module, &mut poller)?;
    let entity_by_stable = entity_by_stable(snapshot, &mut poller)?;
    let mut node_keys = BTreeSet::new();
    let mut edges = Vec::new();
    for diagnostic in &diagnostics {
        poller.step()?;
        let diag_module = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("page_builder");
        let consumer_key = format!("page_builder_consumer://{diag_module}");
        let provider_key = "page_builder_provider://page_builder".to_string();
        let root_key = if entity_by_stable.contains_key(consumer_key.as_str()) {
            consumer_key
        } else {
            provider_key
        };
        node_keys.insert(root_key.clone());
        if let Some(path) = diagnostic
            .payload
            .get("path")
            .and_then(serde_json::Value::as_str)
        {
            let file_key = format!("file://{path}");
            node_keys.insert(file_key.clone());
            edges.push(RustokPageBuilderGraphEdge {
                from: root_key,
                to: file_key,
                kind: "evidenced_by".to_string(),
                evidence: diagnostic_evidence(diagnostic, &mut poller)?,
            });
        }
    }
    let mut selected_ids = BTreeSet::new();
    for key in &node_keys {
        poller.step()?;
        if let Some(entity) = entity_by_stable.get(key.as_str()) {
            selected_ids.insert(entity.id.0.clone());
        }
    }
    let graph = page_builder_graph_from_selection(
        snapshot,
        RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA,
        module.map(|module| format!("page_builder_consumer://{module}")),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
        &mut poller,
    )?;
    poller.finish()?;
    Ok(graph)
}

fn entity_by_id<'a, F>(
    snapshot: &'a CanonicalSnapshot,
    poller: &mut CheckpointPoller<F>,
) -> Result<HashMap<&'a str, &'a Entity>>
where
    F: FnMut() -> Result<()>,
{
    let mut entities = HashMap::with_capacity(snapshot.entities.len());
    for entity in &snapshot.entities {
        poller.step()?;
        entities.insert(entity.id.0.as_str(), entity);
    }
    Ok(entities)
}

fn entity_by_stable<'a, F>(
    snapshot: &'a CanonicalSnapshot,
    poller: &mut CheckpointPoller<F>,
) -> Result<HashMap<&'a str, &'a Entity>>
where
    F: FnMut() -> Result<()>,
{
    let mut entities = HashMap::with_capacity(snapshot.entities.len());
    for entity in &snapshot.entities {
        poller.step()?;
        entities.insert(entity.stable_key.0.as_str(), entity);
    }
    Ok(entities)
}

fn find_entity_by_stable_key<'a, F>(
    snapshot: &'a CanonicalSnapshot,
    stable_key: &str,
    poller: &mut CheckpointPoller<F>,
) -> Result<Option<&'a Entity>>
where
    F: FnMut() -> Result<()>,
{
    for entity in &snapshot.entities {
        poller.step()?;
        if entity.stable_key.0 == stable_key {
            return Ok(Some(entity));
        }
    }
    Ok(None)
}

fn degree_by_id<F>(
    snapshot: &CanonicalSnapshot,
    poller: &mut CheckpointPoller<F>,
) -> Result<HashMap<String, usize>>
where
    F: FnMut() -> Result<()>,
{
    let mut degree_by_id = HashMap::new();
    for relation in &snapshot.relations {
        poller.step()?;
        *degree_by_id.entry(relation.from.0.clone()).or_default() += 1;
        *degree_by_id.entry(relation.to.0.clone()).or_default() += 1;
    }
    Ok(degree_by_id)
}

fn ffa_diagnostics<F>(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<Diagnostic>>
where
    F: FnMut() -> Result<()>,
{
    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        let matches_kind = matches!(
            &diagnostic.kind,
            DiagnosticKind::Other(kind) if kind.starts_with("rustok_ffa_")
        );
        let matches_module = module.is_none_or(|module| {
            diagnostic
                .payload
                .get("module")
                .and_then(serde_json::Value::as_str)
                == Some(module)
        });
        let matches_surface = surface.is_none_or(|surface| {
            diagnostic
                .payload
                .get("surface")
                .and_then(serde_json::Value::as_str)
                == Some(surface)
        });
        if matches_kind && matches_module && matches_surface {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(diagnostics)
}

fn fba_diagnostics<F>(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<Diagnostic>>
where
    F: FnMut() -> Result<()>,
{
    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        let matches_kind = matches!(
            &diagnostic.kind,
            DiagnosticKind::Other(kind) if kind.starts_with("rustok_fba_")
        );
        let matches_module = module.is_none_or(|module| {
            diagnostic
                .payload
                .get("module")
                .and_then(serde_json::Value::as_str)
                == Some(module)
        });
        if matches_kind && matches_module {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(diagnostics)
}

fn page_builder_diagnostics<F>(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<Diagnostic>>
where
    F: FnMut() -> Result<()>,
{
    let mut diagnostics = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        poller.step()?;
        let matches_kind = matches!(
            &diagnostic.kind,
            DiagnosticKind::Other(kind) if kind.starts_with("rustok_page_builder_")
        );
        let matches_module = module.is_none_or(|module| {
            diagnostic
                .payload
                .get("module")
                .and_then(serde_json::Value::as_str)
                == Some(module)
        });
        if matches_kind && matches_module {
            diagnostics.push(diagnostic.clone());
        }
    }
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(diagnostics)
}

#[allow(clippy::too_many_arguments)]
fn fba_graph_from_selection<F>(
    snapshot: &CanonicalSnapshot,
    schema: &str,
    root: Option<String>,
    selected_ids: BTreeSet<String>,
    mut edges: Vec<RustokFbaGraphEdge>,
    diagnostics: Vec<Diagnostic>,
    max_nodes: usize,
    max_edges: usize,
    poller: &mut CheckpointPoller<F>,
) -> Result<RustokFbaGraph>
where
    F: FnMut() -> Result<()>,
{
    let entity_by_id = entity_by_id(snapshot, poller)?;
    let total_nodes = selected_ids.len();
    let total_edges = edges.len();
    let mut nodes = Vec::new();
    for id in &selected_ids {
        poller.step()?;
        if let Some(entity) = entity_by_id.get(id.as_str())
            && (is_fba_entity(entity) || entity.stable_key.0.starts_with("file://"))
        {
            nodes.push(fba_graph_node(entity));
        }
    }
    poller.checkpoint()?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    nodes.truncate(max_nodes);
    let retained = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    edges.retain(|edge| {
        retained.contains(edge.from.as_str()) && retained.contains(edge.to.as_str())
    });
    edges.sort_by(|left, right| {
        (&left.from, &left.to, &left.kind).cmp(&(&right.from, &right.to, &right.kind))
    });
    edges.truncate(max_edges);

    Ok(RustokFbaGraph {
        schema: schema.to_string(),
        snapshot: snapshot_id(snapshot),
        root,
        nodes,
        edges,
        diagnostics,
        omitted: GraphOmitted {
            nodes: total_nodes.saturating_sub(max_nodes.min(total_nodes)),
            edges: total_edges.saturating_sub(max_edges.min(total_edges)),
            reason: "rustok_fba_graph_limits".to_string(),
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn page_builder_graph_from_selection<F>(
    snapshot: &CanonicalSnapshot,
    schema: &str,
    root: Option<String>,
    selected_ids: BTreeSet<String>,
    mut edges: Vec<RustokPageBuilderGraphEdge>,
    diagnostics: Vec<Diagnostic>,
    max_nodes: usize,
    max_edges: usize,
    poller: &mut CheckpointPoller<F>,
) -> Result<RustokPageBuilderGraph>
where
    F: FnMut() -> Result<()>,
{
    let entity_by_id = entity_by_id(snapshot, poller)?;
    let total_nodes = selected_ids.len();
    let total_edges = edges.len();
    let mut nodes = Vec::new();
    for id in &selected_ids {
        poller.step()?;
        if let Some(entity) = entity_by_id.get(id.as_str()) {
            nodes.push(RustokPageBuilderGraphNode {
                id: entity.stable_key.0.clone(),
                kind: serialized_name(&entity.kind),
                name: entity.name.clone(),
                source: entity_source(entity),
            });
        }
    }
    poller.checkpoint()?;
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    nodes.truncate(max_nodes);
    let retained = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    edges.retain(|edge| {
        retained.contains(edge.from.as_str()) && retained.contains(edge.to.as_str())
    });
    edges.sort_by(|left, right| {
        (&left.from, &left.to, &left.kind).cmp(&(&right.from, &right.to, &right.kind))
    });
    edges.truncate(max_edges);

    Ok(RustokPageBuilderGraph {
        schema: schema.to_string(),
        snapshot: snapshot_id(snapshot),
        root,
        nodes,
        edges,
        diagnostics,
        omitted: GraphOmitted {
            nodes: total_nodes.saturating_sub(max_nodes.min(total_nodes)),
            edges: total_edges.saturating_sub(max_edges.min(total_edges)),
            reason: "rustok_page_builder_graph_limits".to_string(),
        },
    })
}

fn push_ffa_edge<F>(
    edges: &mut Vec<RustokFfaGraphEdge>,
    relation: &Relation,
    entity_by_id: &HashMap<&str, &Entity>,
    poller: &mut CheckpointPoller<F>,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
        return Ok(());
    };
    let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
        return Ok(());
    };
    edges.push(RustokFfaGraphEdge {
        from: from.stable_key.0.clone(),
        to: to.stable_key.0.clone(),
        kind: serialized_name(&relation.kind),
        evidence: relation_evidence(relation, poller)?,
    });
    Ok(())
}

fn push_fba_edge<F>(
    edges: &mut Vec<RustokFbaGraphEdge>,
    relation: &Relation,
    entity_by_id: &HashMap<&str, &Entity>,
    poller: &mut CheckpointPoller<F>,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
        return Ok(());
    };
    let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
        return Ok(());
    };
    edges.push(RustokFbaGraphEdge {
        from: from.stable_key.0.clone(),
        to: to.stable_key.0.clone(),
        kind: serialized_name(&relation.kind),
        evidence: relation_evidence(relation, poller)?,
    });
    Ok(())
}

fn push_page_builder_edge<F>(
    edges: &mut Vec<RustokPageBuilderGraphEdge>,
    relation: &Relation,
    entity_by_id: &HashMap<&str, &Entity>,
    poller: &mut CheckpointPoller<F>,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
        return Ok(());
    };
    let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
        return Ok(());
    };
    edges.push(RustokPageBuilderGraphEdge {
        from: from.stable_key.0.clone(),
        to: to.stable_key.0.clone(),
        kind: serialized_name(&relation.kind),
        evidence: relation_evidence(relation, poller)?,
    });
    Ok(())
}

fn relation_evidence<F>(
    relation: &Relation,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<String>>
where
    F: FnMut() -> Result<()>,
{
    let mut rendered = Vec::new();
    for evidence in &relation.evidence {
        poller.step()?;
        if let Some(path) = &evidence.source_file {
            rendered.push(
                evidence
                    .line_start
                    .map_or_else(|| path.clone(), |line| format!("{path}:{line}")),
            );
        }
    }
    Ok(rendered)
}

fn diagnostic_evidence<F>(
    diagnostic: &Diagnostic,
    poller: &mut CheckpointPoller<F>,
) -> Result<Vec<String>>
where
    F: FnMut() -> Result<()>,
{
    let mut rendered = Vec::new();
    for evidence in &diagnostic.evidence {
        poller.step()?;
        if let Some(path) = &evidence.source_file {
            rendered.push(
                evidence
                    .line_start
                    .map_or_else(|| path.clone(), |line| format!("{path}:{line}")),
            );
        }
    }
    Ok(rendered)
}

fn is_page_builder_entity(entity: &Entity) -> bool {
    matches!(
        &entity.kind,
        athanor_domain::EntityKind::Other(kind) if kind.starts_with("rustok_page_builder_")
    )
}

fn is_fba_entity(entity: &Entity) -> bool {
    entity.stable_key.0.starts_with("fba_")
}

fn ffa_graph_node(entity: &Entity, _degree: usize) -> RustokFfaGraphNode {
    RustokFfaGraphNode {
        id: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity_source(entity),
    }
}

fn fba_graph_node(entity: &Entity) -> RustokFbaGraphNode {
    RustokFbaGraphNode {
        id: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity_source(entity),
    }
}

fn entity_source(entity: &Entity) -> Option<String> {
    entity.source.as_ref().map(|source| {
        source.line_start.map_or_else(
            || source.path.clone(),
            |line| format!("{}:{line}", source.path),
        )
    })
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> String {
    snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
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
            self.remaining = self.interval;
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

#[cfg(test)]
mod tests {
    use athanor_core::CoreError;
    use athanor_domain::{
        DiagnosticId, DiagnosticStatus, EntityId, EntityKind, RelationId, RelationStatus,
        Severity, SnapshotId, StableKey,
    };
    use serde_json::json;

    use super::*;
    use crate::graph::{
        build_rustok_fba_dependencies_graph, build_rustok_fba_module_graph,
        build_rustok_fba_port_graph, build_rustok_fba_violations_graph,
        build_rustok_ffa_surface_graph, build_rustok_ffa_violations_graph,
        build_rustok_page_builder_consumer_graph, build_rustok_page_builder_provider_graph,
        build_rustok_page_builder_violations_graph,
    };

    #[test]
    fn cooperative_graphs_match_legacy_outputs() {
        let ffa_surface = entity(
            "ffa-surface",
            "ffa_surface://catalog/product",
            "rustok_ffa_surface",
        );
        let ffa_layer = entity(
            "ffa-layer",
            "ffa_layer://catalog/product/core",
            "rustok_ffa_layer",
        );
        let file = entity("file", "file://src/product.rs", "file");
        let fba_catalog = entity("fba-catalog", "fba_module://catalog", "rustok_fba_module");
        let fba_consumer = entity(
            "fba-consumer",
            "fba_module://consumer",
            "rustok_fba_module",
        );
        let fba_port = entity("fba-port", "fba_port://catalog/read", "rustok_fba_port");
        let fba_operation = entity(
            "fba-operation",
            "fba_operation://catalog/read/get",
            "rustok_fba_operation",
        );
        let fba_dependency = entity(
            "fba-dependency",
            "fba_dependency://consumer/catalog/native",
            "rustok_fba_dependency",
        );
        let page_provider = entity(
            "page-provider",
            "page_builder_provider://page_builder",
            "rustok_page_builder_provider",
        );
        let page_consumer = entity(
            "page-consumer",
            "page_builder_consumer://catalog",
            "rustok_page_builder_consumer",
        );
        let page_contract = entity(
            "page-contract",
            "page_builder_contract://catalog/read",
            "rustok_page_builder_contract",
        );
        let page_capability = entity(
            "page-capability",
            "page_builder_capability://catalog/read",
            "rustok_page_builder_capability",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_graph".to_string())),
            entities: vec![
                ffa_surface.clone(),
                ffa_layer.clone(),
                file.clone(),
                fba_catalog.clone(),
                fba_consumer.clone(),
                fba_port.clone(),
                fba_operation.clone(),
                fba_dependency.clone(),
                page_provider.clone(),
                page_consumer.clone(),
                page_contract.clone(),
                page_capability.clone(),
            ],
            relations: vec![
                relation("ffa-contains", &ffa_surface, &ffa_layer),
                relation("ffa-implemented", &ffa_layer, &file),
                relation("fba-module-port", &fba_catalog, &fba_port),
                relation("fba-port-operation", &fba_port, &fba_operation),
                relation("fba-dependency-provider", &fba_dependency, &fba_catalog),
                relation("page-provider-contract", &page_provider, &page_contract),
                relation("page-consumer-capability", &page_consumer, &page_capability),
            ],
            diagnostics: vec![
                diagnostic(
                    "ffa-diagnostic",
                    "rustok_ffa_layer_missing",
                    json!({
                        "module": "catalog",
                        "surface": "product",
                        "role": "core",
                        "path": "src/product.rs"
                    }),
                ),
                diagnostic(
                    "fba-diagnostic",
                    "rustok_fba_policy_missing",
                    json!({
                        "module": "catalog",
                        "port": "read",
                        "path": "src/product.rs"
                    }),
                ),
                diagnostic(
                    "page-diagnostic",
                    "rustok_page_builder_contract_missing",
                    json!({ "module": "catalog", "path": "src/product.rs" }),
                ),
            ],
            ..CanonicalSnapshot::default()
        };
        let operation = OperationContext::new("rustok-graph-parity");

        assert_eq!(
            build_rustok_ffa_surface_graph_with_operation_context(
                &snapshot,
                "catalog",
                "product",
                100,
                100,
                &operation,
            )
            .unwrap(),
            build_rustok_ffa_surface_graph(&snapshot, "catalog", "product", 100, 100)
                .unwrap()
        );
        assert_eq!(
            build_rustok_ffa_violations_graph_with_operation_context(
                &snapshot,
                Some("catalog"),
                Some("product"),
                100,
                100,
                &operation,
            )
            .unwrap(),
            build_rustok_ffa_violations_graph(
                &snapshot,
                Some("catalog"),
                Some("product"),
                100,
                100,
            )
        );
        assert_eq!(
            build_rustok_fba_module_graph_with_operation_context(
                &snapshot, "catalog", 100, 100, &operation,
            )
            .unwrap(),
            build_rustok_fba_module_graph(&snapshot, "catalog", 100, 100).unwrap()
        );
        assert_eq!(
            build_rustok_fba_port_graph_with_operation_context(
                &snapshot, "catalog", "read", 100, 100, &operation,
            )
            .unwrap(),
            build_rustok_fba_port_graph(&snapshot, "catalog", "read", 100, 100).unwrap()
        );
        assert_eq!(
            build_rustok_fba_dependencies_graph_with_operation_context(
                &snapshot, "consumer", 100, 100, &operation,
            )
            .unwrap(),
            build_rustok_fba_dependencies_graph(&snapshot, "consumer", 100, 100).unwrap()
        );
        assert_eq!(
            build_rustok_fba_violations_graph_with_operation_context(
                &snapshot,
                Some("catalog"),
                100,
                100,
                &operation,
            )
            .unwrap(),
            build_rustok_fba_violations_graph(&snapshot, Some("catalog"), 100, 100)
        );
        assert_eq!(
            build_rustok_page_builder_provider_graph_with_operation_context(
                &snapshot, 100, 100, &operation,
            )
            .unwrap(),
            build_rustok_page_builder_provider_graph(&snapshot, 100, 100).unwrap()
        );
        assert_eq!(
            build_rustok_page_builder_consumer_graph_with_operation_context(
                &snapshot, "catalog", 100, 100, &operation,
            )
            .unwrap(),
            build_rustok_page_builder_consumer_graph(&snapshot, "catalog", 100, 100).unwrap()
        );
        assert_eq!(
            build_rustok_page_builder_violations_graph_with_operation_context(
                &snapshot,
                Some("catalog"),
                100,
                100,
                &operation,
            )
            .unwrap(),
            build_rustok_page_builder_violations_graph(&snapshot, Some("catalog"), 100, 100)
        );
    }

    #[test]
    fn cancellation_after_multiple_edge_batches_stops_graph_builder() {
        let module = entity("module", "fba_module://catalog", "rustok_fba_module");
        let mut entities = vec![module.clone()];
        let mut relations = Vec::new();
        for index in 0..700 {
            let port = entity(
                &format!("port-{index}"),
                &format!("fba_port://catalog/port-{index}"),
                "rustok_fba_port",
            );
            relations.push(relation(&format!("relation-{index}"), &module, &port));
            entities.push(port);
        }
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_large_graph".to_string())),
            entities,
            relations,
            ..CanonicalSnapshot::default()
        };
        let operation = OperationContext::new("rustok-graph-mid-cancel");
        let cancellation = operation.cancellation_handle().unwrap();
        let mut checkpoints = 0;

        let error = build_fba_module_with_checkpoint(&snapshot, "catalog", 800, 800, || {
            checkpoints += 1;
            if checkpoints == 3 {
                cancellation.cancel();
            }
            operation.check_active().map_err(anyhow::Error::new)
        })
        .expect_err("cancelled graph build must stop after bounded edge batches");

        assert!(checkpoints >= 3);
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    fn entity(id: &str, stable_key: &str, kind: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind: if kind == "file" {
                EntityKind::File
            } else {
                EntityKind::Other(kind.to_string())
            },
            name: id.to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }

    fn relation(id: &str, from: &Entity, to: &Entity) -> Relation {
        Relation {
            id: RelationId(id.to_string()),
            kind: if id == "ffa-implemented" {
                RelationKind::ImplementedBy
            } else {
                RelationKind::Contains
            },
            from: from.id.clone(),
            to: to.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_graph".to_string()),
            payload: json!({ "schema": "rustok.page_builder.relation.v1" }),
        }
    }

    fn diagnostic(id: &str, kind: &str, payload: serde_json::Value) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(id.to_string()),
            kind: DiagnosticKind::Other(kind.to_string()),
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: id.to_string(),
            message: id.to_string(),
            entities: Vec::new(),
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_graph".to_string()),
            suggested_fix: None,
            payload,
        }
    }
}
