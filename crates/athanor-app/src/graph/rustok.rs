use anyhow::Result;
use athanor_core::{CanonicalSnapshot, OperationContext};

use super::model::{
    RustokFbaAudit, RustokFbaGraph, RustokFfaAudit, RustokFfaGraph, RustokPageBuilderAudit,
    RustokPageBuilderGraph,
};

pub fn build_rustok_ffa_audit(snapshot: &CanonicalSnapshot) -> RustokFfaAudit {
    crate::rustok_audit_cooperative::build_rustok_ffa_audit_with_operation_context(
        snapshot,
        &pure_operation("rustok-ffa-audit"),
    )
    .expect("fresh graph operation context must remain active")
}

pub fn build_rustok_fba_audit(snapshot: &CanonicalSnapshot) -> RustokFbaAudit {
    crate::rustok_audit_cooperative::build_rustok_fba_audit_with_operation_context(
        snapshot,
        &pure_operation("rustok-fba-audit"),
    )
    .expect("fresh graph operation context must remain active")
}

pub fn build_rustok_page_builder_audit(snapshot: &CanonicalSnapshot) -> RustokPageBuilderAudit {
    crate::rustok_audit_cooperative::build_rustok_page_builder_audit_with_operation_context(
        snapshot,
        &pure_operation("rustok-page-builder-audit"),
    )
    .expect("fresh graph operation context must remain active")
}

pub fn build_rustok_ffa_surface_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    surface: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFfaGraph> {
    crate::rustok_graph_cooperative::build_rustok_ffa_surface_graph_with_operation_context(
        snapshot,
        module,
        surface,
        max_nodes,
        max_edges,
        &pure_operation("rustok-ffa-surface-graph"),
    )
}

pub fn build_rustok_ffa_violations_graph(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokFfaGraph {
    crate::rustok_graph_cooperative::build_rustok_ffa_violations_graph_with_operation_context(
        snapshot,
        module,
        surface,
        max_nodes,
        max_edges,
        &pure_operation("rustok-ffa-violations-graph"),
    )
    .expect("fresh graph operation context must remain active")
}

pub fn build_rustok_fba_module_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFbaGraph> {
    crate::rustok_graph_cooperative::build_rustok_fba_module_graph_with_operation_context(
        snapshot,
        module,
        max_nodes,
        max_edges,
        &pure_operation("rustok-fba-module-graph"),
    )
}

pub fn build_rustok_fba_port_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    port: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFbaGraph> {
    crate::rustok_graph_cooperative::build_rustok_fba_port_graph_with_operation_context(
        snapshot,
        module,
        port,
        max_nodes,
        max_edges,
        &pure_operation("rustok-fba-port-graph"),
    )
}

pub fn build_rustok_fba_dependencies_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFbaGraph> {
    crate::rustok_graph_cooperative::build_rustok_fba_dependencies_graph_with_operation_context(
        snapshot,
        module,
        max_nodes,
        max_edges,
        &pure_operation("rustok-fba-dependencies-graph"),
    )
}

pub fn build_rustok_fba_violations_graph(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokFbaGraph {
    crate::rustok_graph_cooperative::build_rustok_fba_violations_graph_with_operation_context(
        snapshot,
        module,
        max_nodes,
        max_edges,
        &pure_operation("rustok-fba-violations-graph"),
    )
    .expect("fresh graph operation context must remain active")
}

pub fn build_rustok_page_builder_provider_graph(
    snapshot: &CanonicalSnapshot,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokPageBuilderGraph> {
    crate::rustok_graph_cooperative::build_rustok_page_builder_provider_graph_with_operation_context(
        snapshot,
        max_nodes,
        max_edges,
        &pure_operation("rustok-page-builder-provider-graph"),
    )
}

pub fn build_rustok_page_builder_consumer_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokPageBuilderGraph> {
    crate::rustok_graph_cooperative::build_rustok_page_builder_consumer_graph_with_operation_context(
        snapshot,
        module,
        max_nodes,
        max_edges,
        &pure_operation("rustok-page-builder-consumer-graph"),
    )
}

pub fn build_rustok_page_builder_violations_graph(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokPageBuilderGraph {
    crate::rustok_graph_cooperative::build_rustok_page_builder_violations_graph_with_operation_context(
        snapshot,
        module,
        max_nodes,
        max_edges,
        &pure_operation("rustok-page-builder-violations-graph"),
    )
    .expect("fresh graph operation context must remain active")
}

fn pure_operation(name: &str) -> OperationContext {
    OperationContext::new(name)
}
