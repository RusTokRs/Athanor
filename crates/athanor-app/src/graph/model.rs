use std::path::PathBuf;

use athanor_domain::Diagnostic;
use serde::Serialize;

pub const GRAPH_EXPORT_SCHEMA: &str = "athanor.graph_export.v1";
pub const GRAPH_CYCLES_SCHEMA: &str = "athanor.graph_cycles.v1";
pub const GRAPH_HUBS_SCHEMA: &str = "athanor.graph_hubs.v1";
pub const GRAPH_PAGERANK_SCHEMA: &str = "athanor.graph_pagerank.v1";
pub const GRAPH_PATH_SCHEMA: &str = "athanor.graph_path.v1";
pub const GRAPH_RELATED_SCHEMA: &str = "athanor.graph_related.v1";
pub const RUSTOK_FFA_AUDIT_SCHEMA: &str = "athanor.rustok_ffa_audit.v1";
pub const RUSTOK_FFA_SURFACE_GRAPH_SCHEMA: &str = "athanor.rustok_ffa_surface_graph.v1";
pub const RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA: &str = "athanor.rustok_ffa_violations_graph.v1";
pub const RUSTOK_FBA_AUDIT_SCHEMA: &str = "athanor.rustok_fba_audit.v1";
pub const RUSTOK_FBA_MODULE_GRAPH_SCHEMA: &str = "athanor.rustok_fba_module_graph.v1";
pub const RUSTOK_FBA_PORT_GRAPH_SCHEMA: &str = "athanor.rustok_fba_port_graph.v1";
pub const RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA: &str = "athanor.rustok_fba_dependencies_graph.v1";
pub const RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA: &str = "athanor.rustok_fba_violations_graph.v1";
pub const RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA: &str = "athanor.rustok_page_builder_audit.v1";
pub const RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA: &str =
    "athanor.rustok_page_builder_provider_graph.v1";
pub const RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA: &str =
    "athanor.rustok_page_builder_consumer_graph.v1";
pub const RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA: &str =
    "athanor.rustok_page_builder_violations_graph.v1";

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
pub struct GraphPageRankOptions {
    pub root: PathBuf,
    pub limit: usize,
    pub kind: Option<String>,
    pub damping: f64,
    pub max_iterations: usize,
    pub tolerance: f64,
    pub max_relation_ids: usize,
}

#[derive(Debug, Clone)]
pub struct GraphCyclesOptions {
    pub root: PathBuf,
    pub limit: usize,
    pub max_depth: usize,
    pub max_starts: usize,
}

#[derive(Debug, Clone)]
pub struct RustokFfaAuditOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RustokFbaAuditOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RustokPageBuilderAuditOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GraphFfaSurfaceOptions {
    pub root: PathBuf,
    pub module: String,
    pub surface: String,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphFfaViolationsOptions {
    pub root: PathBuf,
    pub module: Option<String>,
    pub surface: Option<String>,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphFbaModuleOptions {
    pub root: PathBuf,
    pub module: String,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphFbaPortOptions {
    pub root: PathBuf,
    pub module: String,
    pub port: String,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphFbaDependenciesOptions {
    pub root: PathBuf,
    pub module: String,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphFbaViolationsOptions {
    pub root: PathBuf,
    pub module: Option<String>,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphPageBuilderConsumerOptions {
    pub root: PathBuf,
    pub module: String,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphPageBuilderProviderOptions {
    pub root: PathBuf,
    pub max_nodes: usize,
    pub max_edges: usize,
}

#[derive(Debug, Clone)]
pub struct GraphPageBuilderViolationsOptions {
    pub root: PathBuf,
    pub module: Option<String>,
    pub max_nodes: usize,
    pub max_edges: usize,
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
pub struct GraphPageRank {
    pub schema: String,
    pub snapshot: String,
    pub kind: Option<String>,
    pub damping: f64,
    pub iterations: usize,
    pub converged: bool,
    pub entity_count: usize,
    pub relation_count: usize,
    pub ranks: Vec<GraphPageRankEntry>,
    pub omitted: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphPageRankEntry {
    pub rank: usize,
    #[serde(flatten)]
    pub entity: GraphNode,
    pub score: f64,
    pub incoming_relations: Vec<GraphPageRankRelationTrace>,
    pub omitted_incoming_relations: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GraphPageRankRelationTrace {
    pub id: String,
    pub kind: String,
    pub from_entity_id: String,
    pub evidence: Vec<String>,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFfaAudit {
    pub schema: String,
    pub snapshot: String,
    pub summary: RustokFfaAuditSummary,
    pub surfaces: Vec<RustokFfaAuditSurface>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFfaAuditSummary {
    pub observed_surfaces: usize,
    pub surfaces_total: usize,
    pub core_transport_ui: usize,
    pub incomplete: usize,
    pub requirements_met: usize,
    pub requirements_total: usize,
    pub completion_percent: Option<u8>,
    pub missing_core: usize,
    pub missing_transport: usize,
    pub missing_ui_adapter: usize,
    pub scaffold_surfaces: usize,
    pub host_wiring_surfaces: usize,
    pub diagnostics_open: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFfaAuditSurface {
    pub id: String,
    pub module: String,
    pub surface: String,
    pub shape: String,
    pub actionable: bool,
    pub requirements_met: usize,
    pub requirements_total: usize,
    pub completion_percent: Option<u8>,
    pub core_present: bool,
    pub transport_present: bool,
    pub ui_adapter_present: bool,
    pub host_wiring_present: bool,
    pub diagnostics_open: usize,
    pub layers: Vec<String>,
    pub files: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RustokFfaGraph {
    pub schema: String,
    pub snapshot: String,
    pub surface: Option<String>,
    pub nodes: Vec<RustokFfaGraphNode>,
    pub edges: Vec<RustokFfaGraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: GraphOmitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFfaGraphNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFfaGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFbaAudit {
    pub schema: String,
    pub snapshot: String,
    pub summary: RustokFbaAuditSummary,
    pub modules: Vec<RustokFbaAuditModule>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFbaAuditSummary {
    pub modules_total: usize,
    pub registered_modules: usize,
    pub dependency_only_modules: usize,
    pub in_progress_modules: usize,
    pub status_unknown_modules: usize,
    pub requirements_met: usize,
    pub requirements_total: usize,
    pub completion_percent: Option<u8>,
    pub modules_with_port_code: usize,
    pub modules_with_complete_operations: usize,
    pub modules_with_evidence: usize,
    pub dependency_edges_resolved: usize,
    pub dependency_edges_total: usize,
    pub provider_modules: usize,
    pub consumer_modules: usize,
    pub ports_total: usize,
    pub operations_total: usize,
    pub diagnostics_open: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFbaAuditModule {
    pub id: String,
    pub module: String,
    pub role: Option<String>,
    pub status: Option<String>,
    pub registry_present: bool,
    pub requirements_met: usize,
    pub requirements_total: usize,
    pub completion_percent: Option<u8>,
    pub port_code_present: Option<bool>,
    pub port_traits_present: Option<bool>,
    pub operations_implemented: Option<bool>,
    pub context_present: Option<bool>,
    pub error_present: Option<bool>,
    pub policy_present: Option<bool>,
    pub evidence_present: Option<bool>,
    pub contract_tests_present: Option<bool>,
    pub write_idempotency_present: Option<bool>,
    pub dependencies_resolved: Option<bool>,
    pub contract_version: Option<String>,
    pub ports: Vec<String>,
    pub operations: Vec<String>,
    pub dependencies: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RustokFbaGraph {
    pub schema: String,
    pub snapshot: String,
    pub root: Option<String>,
    pub nodes: Vec<RustokFbaGraphNode>,
    pub edges: Vec<RustokFbaGraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: GraphOmitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFbaGraphNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFbaGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokPageBuilderAudit {
    pub schema: String,
    pub snapshot: String,
    pub summary: RustokPageBuilderAuditSummary,
    pub providers: Vec<String>,
    pub consumers: Vec<RustokPageBuilderAuditConsumer>,
    pub contracts: Vec<String>,
    pub capabilities: Vec<String>,
    pub fallback_profiles: Vec<String>,
    pub wave_evidence: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokPageBuilderAuditSummary {
    pub providers_total: usize,
    pub consumers_total: usize,
    pub contracts_total: usize,
    pub capabilities_total: usize,
    pub fallback_profiles_total: usize,
    pub wave_evidence_total: usize,
    pub diagnostics_open: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokPageBuilderAuditConsumer {
    pub id: String,
    pub module: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RustokPageBuilderGraph {
    pub schema: String,
    pub snapshot: String,
    pub root: Option<String>,
    pub nodes: Vec<RustokPageBuilderGraphNode>,
    pub edges: Vec<RustokPageBuilderGraphEdge>,
    pub diagnostics: Vec<Diagnostic>,
    pub omitted: GraphOmitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokPageBuilderGraphNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokPageBuilderGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub evidence: Vec<String>,
}
