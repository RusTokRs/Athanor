use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{Diagnostic, DiagnosticKind, Entity, Relation, RelationKind};
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
    pub surfaces_total: usize,
    pub core_transport_ui: usize,
    pub incomplete: usize,
    pub diagnostics_open: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RustokFfaAuditSurface {
    pub id: String,
    pub module: String,
    pub surface: String,
    pub shape: String,
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

pub async fn graph_pagerank(options: GraphPageRankOptions) -> Result<GraphPageRank> {
    if options.limit == 0
        || options.max_iterations == 0
        || options.max_relation_ids == 0
        || !(0.0..1.0).contains(&options.damping)
        || options.tolerance <= 0.0
        || !options.tolerance.is_finite()
    {
        bail!(
            "graph pagerank requires positive limits and tolerance, with damping between zero and one"
        );
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

    build_graph_pagerank(
        &snapshot,
        options.limit,
        options.kind.as_deref(),
        options.damping,
        options.max_iterations,
        options.tolerance,
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

pub async fn rustok_ffa_audit(options: RustokFfaAuditOptions) -> Result<RustokFfaAudit> {
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    Ok(build_rustok_ffa_audit(&snapshot))
}

pub async fn rustok_fba_audit(options: RustokFbaAuditOptions) -> Result<RustokFbaAudit> {
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    Ok(build_rustok_fba_audit(&snapshot))
}

pub async fn rustok_page_builder_audit(
    options: RustokPageBuilderAuditOptions,
) -> Result<RustokPageBuilderAudit> {
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    Ok(build_rustok_page_builder_audit(&snapshot))
}

pub async fn graph_ffa_surface(options: GraphFfaSurfaceOptions) -> Result<RustokFfaGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("FFA graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    build_rustok_ffa_surface_graph(
        &snapshot,
        &options.module,
        &options.surface,
        options.max_nodes,
        options.max_edges,
    )
}

pub async fn graph_ffa_violations(options: GraphFfaViolationsOptions) -> Result<RustokFfaGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("FFA graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    Ok(build_rustok_ffa_violations_graph(
        &snapshot,
        options.module.as_deref(),
        options.surface.as_deref(),
        options.max_nodes,
        options.max_edges,
    ))
}

pub async fn graph_fba_module(options: GraphFbaModuleOptions) -> Result<RustokFbaGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    build_rustok_fba_module_graph(
        &snapshot,
        &options.module,
        options.max_nodes,
        options.max_edges,
    )
}

pub async fn graph_fba_port(options: GraphFbaPortOptions) -> Result<RustokFbaGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    build_rustok_fba_port_graph(
        &snapshot,
        &options.module,
        &options.port,
        options.max_nodes,
        options.max_edges,
    )
}

pub async fn graph_fba_dependencies(
    options: GraphFbaDependenciesOptions,
) -> Result<RustokFbaGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    build_rustok_fba_dependencies_graph(
        &snapshot,
        &options.module,
        options.max_nodes,
        options.max_edges,
    )
}

pub async fn graph_fba_violations(options: GraphFbaViolationsOptions) -> Result<RustokFbaGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    Ok(build_rustok_fba_violations_graph(
        &snapshot,
        options.module.as_deref(),
        options.max_nodes,
        options.max_edges,
    ))
}

pub async fn graph_page_builder_provider(
    options: GraphPageBuilderProviderOptions,
) -> Result<RustokPageBuilderGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("Page Builder graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    build_rustok_page_builder_provider_graph(&snapshot, options.max_nodes, options.max_edges)
}

pub async fn graph_page_builder_consumer(
    options: GraphPageBuilderConsumerOptions,
) -> Result<RustokPageBuilderGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("Page Builder graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    build_rustok_page_builder_consumer_graph(
        &snapshot,
        &options.module,
        options.max_nodes,
        options.max_edges,
    )
}

pub async fn graph_page_builder_violations(
    options: GraphPageBuilderViolationsOptions,
) -> Result<RustokPageBuilderGraph> {
    if options.max_nodes == 0 || options.max_edges == 0 {
        bail!("Page Builder graph limits must be greater than zero");
    }
    let snapshot = load_latest_graph_snapshot(options.root).await?;
    Ok(build_rustok_page_builder_violations_graph(
        &snapshot,
        options.module.as_deref(),
        options.max_nodes,
        options.max_edges,
    ))
}

async fn load_latest_graph_snapshot(root: PathBuf) -> Result<CanonicalSnapshot> {
    let root = normalize_canonical_path(
        root.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })
}

pub fn build_rustok_fba_audit(snapshot: &CanonicalSnapshot) -> RustokFbaAudit {
    let snapshot_id = snapshot_id(snapshot);
    let module_index = fba_module_index(snapshot);
    let diagnostics = fba_diagnostics(snapshot, None);
    let diagnostics_by_module = diagnostics_by_module(&diagnostics);
    let mut modules = module_index
        .into_iter()
        .map(|(module, details)| {
            let mut ports = details.ports.into_iter().collect::<Vec<_>>();
            let mut operations = details.operations.into_iter().collect::<Vec<_>>();
            let mut dependencies = details.dependencies.into_iter().collect::<Vec<_>>();
            ports.sort();
            operations.sort();
            dependencies.sort();
            let diagnostics = diagnostics_by_module
                .get(&module)
                .cloned()
                .unwrap_or_default();
            RustokFbaAuditModule {
                id: format!("fba_module://{module}"),
                module,
                role: details.role,
                status: details.status,
                contract_version: details.contract_version,
                ports,
                operations,
                dependencies,
                diagnostics,
            }
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.module.cmp(&right.module));

    let provider_modules = modules
        .iter()
        .filter(|module| module.role.as_deref() == Some("provider"))
        .count();
    let consumer_modules = modules
        .iter()
        .filter(|module| {
            matches!(
                module.role.as_deref(),
                Some("consumer") | Some("orchestrator_consumer") | Some("consumer_support_adapter")
            )
        })
        .count();
    let ports_total = modules.iter().map(|module| module.ports.len()).sum();
    let operations_total = modules.iter().map(|module| module.operations.len()).sum();
    let diagnostics_open = modules
        .iter()
        .map(|module| module.diagnostics.len())
        .sum::<usize>();

    RustokFbaAudit {
        schema: RUSTOK_FBA_AUDIT_SCHEMA.to_string(),
        snapshot: snapshot_id,
        summary: RustokFbaAuditSummary {
            modules_total: modules.len(),
            provider_modules,
            consumer_modules,
            ports_total,
            operations_total,
            diagnostics_open,
        },
        modules,
    }
}

pub fn build_rustok_page_builder_audit(snapshot: &CanonicalSnapshot) -> RustokPageBuilderAudit {
    let diagnostics = page_builder_diagnostics(snapshot, None);
    let diagnostics_by_module = diagnostics_by_module(&diagnostics);
    let mut providers = Vec::new();
    let mut consumers = Vec::new();
    let mut contracts = Vec::new();
    let mut capabilities = Vec::new();
    let mut fallback_profiles = Vec::new();
    let mut wave_evidence = Vec::new();

    for entity in &snapshot.entities {
        if !is_page_builder_entity(entity) {
            continue;
        }
        let stable = entity.stable_key.0.clone();
        match entity.kind {
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_provider" =>
            {
                providers.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_consumer" =>
            {
                let module = stable
                    .strip_prefix("page_builder_consumer://")
                    .unwrap_or(entity.name.as_str())
                    .to_string();
                consumers.push(RustokPageBuilderAuditConsumer {
                    id: stable,
                    module: module.clone(),
                    diagnostics: diagnostics_by_module
                        .get(&module)
                        .cloned()
                        .unwrap_or_default(),
                });
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_contract" =>
            {
                contracts.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_capability" =>
            {
                capabilities.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_fallback_profile" =>
            {
                fallback_profiles.push(stable);
            }
            athanor_domain::EntityKind::Other(ref kind)
                if kind == "rustok_page_builder_wave_evidence" =>
            {
                wave_evidence.push(stable);
            }
            _ => {}
        }
    }

    providers.sort();
    consumers.sort_by(|left, right| left.module.cmp(&right.module));
    contracts.sort();
    capabilities.sort();
    fallback_profiles.sort();
    wave_evidence.sort();
    let diagnostics = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.id.0.clone())
        .collect::<Vec<_>>();

    RustokPageBuilderAudit {
        schema: RUSTOK_PAGE_BUILDER_AUDIT_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        summary: RustokPageBuilderAuditSummary {
            providers_total: providers.len(),
            consumers_total: consumers.len(),
            contracts_total: contracts.len(),
            capabilities_total: capabilities.len(),
            fallback_profiles_total: fallback_profiles.len(),
            wave_evidence_total: wave_evidence.len(),
            diagnostics_open: diagnostics.len(),
        },
        providers,
        consumers,
        contracts,
        capabilities,
        fallback_profiles,
        wave_evidence,
        diagnostics,
    }
}

pub fn build_rustok_page_builder_provider_graph(
    snapshot: &CanonicalSnapshot,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokPageBuilderGraph> {
    let provider_key = "page_builder_provider://page_builder";
    let entity_by_stable = snapshot
        .entities
        .iter()
        .map(|entity| (entity.stable_key.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let provider = entity_by_stable
        .get(provider_key)
        .ok_or_else(|| anyhow::anyhow!("Page Builder provider not found for `{provider_key}`"))?;
    let mut selected_ids = BTreeSet::from([provider.id.0.clone()]);
    let mut edges = Vec::new();
    let entity_by_id = entity_by_id(snapshot);
    for relation in &snapshot.relations {
        let from = entity_by_id.get(relation.from.0.as_str());
        let to = entity_by_id.get(relation.to.0.as_str());
        let touches = relation.from == provider.id
            || relation.to == provider.id
            || from.is_some_and(|entity| is_page_builder_entity(entity))
                && to.is_some_and(|entity| is_page_builder_entity(entity))
                && relation
                    .payload
                    .get("schema")
                    .and_then(serde_json::Value::as_str)
                    == Some("rustok.page_builder.relation.v1");
        if touches {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_page_builder_edge(&mut edges, relation, &entity_by_id);
        }
    }
    Ok(page_builder_graph_from_selection(
        snapshot,
        RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA,
        Some(provider_key.to_string()),
        selected_ids,
        edges,
        page_builder_diagnostics(snapshot, None),
        max_nodes,
        max_edges,
    ))
}

pub fn build_rustok_page_builder_consumer_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokPageBuilderGraph> {
    let consumer_key = format!("page_builder_consumer://{module}");
    let entity_by_id = entity_by_id(snapshot);
    let consumer = snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == consumer_key)
        .ok_or_else(|| anyhow::anyhow!("Page Builder consumer not found for `{consumer_key}`"))?;
    let mut selected_ids = BTreeSet::from([consumer.id.0.clone()]);
    let mut edges = Vec::new();
    for relation in &snapshot.relations {
        let from = entity_by_id.get(relation.from.0.as_str());
        let to = entity_by_id.get(relation.to.0.as_str());
        let touches = relation.from == consumer.id
            || relation.to == consumer.id
            || from.is_some_and(|entity| entity.stable_key.0.contains(&format!("://{module}/")))
            || to.is_some_and(|entity| entity.stable_key.0.contains(&format!("://{module}/")));
        if touches
            && (from.is_some_and(|entity| is_page_builder_entity(entity))
                || to.is_some_and(|entity| is_page_builder_entity(entity)))
        {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_page_builder_edge(&mut edges, relation, &entity_by_id);
        }
    }
    Ok(page_builder_graph_from_selection(
        snapshot,
        RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA,
        Some(consumer_key),
        selected_ids,
        edges,
        page_builder_diagnostics(snapshot, Some(module)),
        max_nodes,
        max_edges,
    ))
}

pub fn build_rustok_page_builder_violations_graph(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokPageBuilderGraph {
    let diagnostics = page_builder_diagnostics(snapshot, module);
    let entity_by_stable = snapshot
        .entities
        .iter()
        .map(|entity| (entity.stable_key.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let mut node_keys = BTreeSet::new();
    let mut edges = Vec::new();
    for diagnostic in &diagnostics {
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
                evidence: diagnostic_evidence(diagnostic),
            });
        }
    }
    let selected_ids = node_keys
        .iter()
        .filter_map(|key| {
            entity_by_stable
                .get(key.as_str())
                .map(|entity| entity.id.0.clone())
        })
        .collect::<BTreeSet<_>>();
    page_builder_graph_from_selection(
        snapshot,
        RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA,
        module.map(|module| format!("page_builder_consumer://{module}")),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
    )
}

pub fn build_rustok_fba_module_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFbaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let module_key = format!("fba_module://{module}");
    let entity_by_id = entity_by_id(snapshot);
    let module_entity = snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == module_key)
        .ok_or_else(|| anyhow::anyhow!("FBA module not found for `{module_key}`"))?;
    let mut selected_ids = BTreeSet::from([module_entity.id.0.clone()]);
    let mut edges = Vec::new();

    for relation in &snapshot.relations {
        let from = entity_by_id.get(relation.from.0.as_str());
        let to = entity_by_id.get(relation.to.0.as_str());
        let touches_module =
            relation.from == module_entity.id || relation.to == module_entity.id || {
                let from_key = from
                    .map(|entity| entity.stable_key.0.as_str())
                    .unwrap_or("");
                let to_key = to.map(|entity| entity.stable_key.0.as_str()).unwrap_or("");
                from_key.contains(&format!("://{module}/"))
                    || to_key.contains(&format!("://{module}/"))
                    || from_key == module_key
                    || to_key == module_key
            };
        if touches_module
            && (from.is_some_and(|entity| is_fba_entity(entity))
                || to.is_some_and(|entity| is_fba_entity(entity)))
        {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_fba_edge(&mut edges, relation, &entity_by_id);
        }
    }

    Ok(fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_MODULE_GRAPH_SCHEMA,
        Some(module_key),
        selected_ids,
        edges,
        fba_diagnostics(snapshot, Some(module)),
        max_nodes,
        max_edges,
    ))
}

pub fn build_rustok_fba_port_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    port: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFbaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let port_key = format!("fba_port://{module}/{port}");
    let entity_by_id = entity_by_id(snapshot);
    let port_entity = snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == port_key)
        .ok_or_else(|| anyhow::anyhow!("FBA port not found for `{port_key}`"))?;
    let mut selected_ids = BTreeSet::from([port_entity.id.0.clone()]);
    let mut edges = Vec::new();

    for relation in &snapshot.relations {
        if relation.from == port_entity.id || relation.to == port_entity.id {
            selected_ids.insert(relation.from.0.clone());
            selected_ids.insert(relation.to.0.clone());
            push_fba_edge(&mut edges, relation, &entity_by_id);
        }
        if relation.from == port_entity.id
            && let Some(operation) = entity_by_id.get(relation.to.0.as_str())
            && operation.stable_key.0.starts_with("fba_operation://")
        {
            selected_ids.insert(operation.id.0.clone());
        }
    }

    Ok(fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_PORT_GRAPH_SCHEMA,
        Some(port_key),
        selected_ids,
        edges,
        fba_diagnostics(snapshot, Some(module)),
        max_nodes,
        max_edges,
    ))
}

pub fn build_rustok_fba_dependencies_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFbaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FBA graph limits must be greater than zero");
    }
    let module_key = format!("fba_module://{module}");
    let entity_by_id = entity_by_id(snapshot);
    let entity_by_stable = snapshot
        .entities
        .iter()
        .map(|entity| (entity.stable_key.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let mut selected_ids = BTreeSet::new();
    if let Some(entity) = entity_by_stable.get(module_key.as_str()) {
        selected_ids.insert(entity.id.0.clone());
    }
    let mut edges = Vec::new();
    let module_segment = format!("fba_dependency://{module}/");
    let provider_segment = format!("/{module}/");
    for relation in &snapshot.relations {
        let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
            continue;
        };
        let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
            continue;
        };
        if !is_fba_entity(from) || !is_fba_entity(to) {
            continue;
        }
        if from.stable_key.0.starts_with(module_segment.as_str())
            || from.stable_key.0.contains(provider_segment.as_str())
            || to.stable_key.0 == module_key
        {
            selected_ids.insert(from.id.0.clone());
            selected_ids.insert(to.id.0.clone());
            push_fba_edge(&mut edges, relation, &entity_by_id);
        }
    }

    Ok(fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA,
        Some(module_key),
        selected_ids,
        edges,
        fba_diagnostics(snapshot, Some(module)),
        max_nodes,
        max_edges,
    ))
}

pub fn build_rustok_fba_violations_graph(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokFbaGraph {
    let diagnostics = fba_diagnostics(snapshot, module);
    let entity_by_stable = snapshot
        .entities
        .iter()
        .map(|entity| (entity.stable_key.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let mut node_keys = BTreeSet::new();
    let mut edges = Vec::new();
    for diagnostic in &diagnostics {
        let Some(diag_module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let module_key = format!("fba_module://{diag_module}");
        node_keys.insert(module_key.clone());
        if let Some(port) = diagnostic
            .payload
            .get("port")
            .and_then(serde_json::Value::as_str)
        {
            let port_key = format!("fba_port://{diag_module}/{port}");
            node_keys.insert(port_key.clone());
            edges.push(RustokFbaGraphEdge {
                from: module_key.clone(),
                to: port_key.clone(),
                kind: "violates".to_string(),
                evidence: diagnostic_evidence(diagnostic),
            });
            if let Some(path) = diagnostic
                .payload
                .get("path")
                .and_then(serde_json::Value::as_str)
            {
                let file_key = format!("file://{path}");
                node_keys.insert(file_key.clone());
                edges.push(RustokFbaGraphEdge {
                    from: port_key,
                    to: file_key,
                    kind: "evidenced_by".to_string(),
                    evidence: diagnostic_evidence(diagnostic),
                });
            }
        } else if let Some(path) = diagnostic
            .payload
            .get("path")
            .and_then(serde_json::Value::as_str)
        {
            let file_key = format!("file://{path}");
            node_keys.insert(file_key.clone());
            edges.push(RustokFbaGraphEdge {
                from: module_key,
                to: file_key,
                kind: "evidenced_by".to_string(),
                evidence: diagnostic_evidence(diagnostic),
            });
        }
    }

    let mut selected_ids = BTreeSet::new();
    for key in node_keys {
        if let Some(entity) = entity_by_stable.get(key.as_str()) {
            selected_ids.insert(entity.id.0.clone());
        }
    }
    fba_graph_from_selection(
        snapshot,
        RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA,
        module.map(|module| format!("fba_module://{module}")),
        selected_ids,
        edges,
        diagnostics,
        max_nodes,
        max_edges,
    )
}

pub fn build_rustok_ffa_audit(snapshot: &CanonicalSnapshot) -> RustokFfaAudit {
    let snapshot_id = snapshot_id(snapshot);
    let surface_index = ffa_surface_index(snapshot);
    let diagnostics = ffa_diagnostics(snapshot, None, None);
    let diagnostics_by_surface = diagnostics_by_surface(&diagnostics);
    let mut surfaces = surface_index
        .into_iter()
        .map(|((module, surface), details)| {
            let key = (module.clone(), surface.clone());
            let mut layers = details.layers.into_iter().collect::<Vec<_>>();
            let mut files = details.files.into_iter().collect::<Vec<_>>();
            layers.sort();
            files.sort();
            let diagnostics = diagnostics_by_surface
                .get(&key)
                .cloned()
                .unwrap_or_default();
            RustokFfaAuditSurface {
                id: format!("ffa_surface://{module}/{surface}"),
                module,
                surface,
                shape: ffa_shape(&layers),
                layers,
                files,
                diagnostics,
            }
        })
        .collect::<Vec<_>>();
    surfaces
        .sort_by(|left, right| (&left.module, &left.surface).cmp(&(&right.module, &right.surface)));

    let actionable_surfaces = surfaces
        .iter()
        .filter(|surface| !matches!(surface.shape.as_str(), "host_wiring" | "scaffold"))
        .count();
    let core_transport_ui = surfaces
        .iter()
        .filter(|surface| surface.shape == "core_transport_ui")
        .count();
    let diagnostics_open = surfaces
        .iter()
        .map(|surface| surface.diagnostics.len())
        .sum::<usize>();

    RustokFfaAudit {
        schema: RUSTOK_FFA_AUDIT_SCHEMA.to_string(),
        snapshot: snapshot_id,
        summary: RustokFfaAuditSummary {
            surfaces_total: actionable_surfaces,
            core_transport_ui,
            incomplete: actionable_surfaces.saturating_sub(core_transport_ui),
            diagnostics_open,
        },
        surfaces,
    }
}

pub fn build_rustok_ffa_surface_graph(
    snapshot: &CanonicalSnapshot,
    module: &str,
    surface: &str,
    max_nodes: usize,
    max_edges: usize,
) -> Result<RustokFfaGraph> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("FFA graph limits must be greater than zero");
    }
    let surface_key = format!("ffa_surface://{module}/{surface}");
    let entity_by_id = entity_by_id(snapshot);
    let degree_by_id = degree_by_id(snapshot);
    let surface_entity = snapshot
        .entities
        .iter()
        .find(|entity| entity.stable_key.0 == surface_key)
        .ok_or_else(|| anyhow::anyhow!("FFA surface not found for `{surface_key}`"))?;

    let mut selected_ids = BTreeSet::from([surface_entity.id.0.clone()]);
    let mut edges = Vec::new();
    for relation in &snapshot.relations {
        if relation.from == surface_entity.id
            && matches!(relation.kind, RelationKind::Contains)
            && let Some(layer) = entity_by_id.get(relation.to.0.as_str())
            && layer.stable_key.0.starts_with("ffa_layer://")
        {
            selected_ids.insert(layer.id.0.clone());
            push_ffa_edge(&mut edges, relation, &entity_by_id);
            for file_relation in snapshot.relations.iter().filter(|candidate| {
                candidate.from == layer.id && matches!(candidate.kind, RelationKind::ImplementedBy)
            }) {
                selected_ids.insert(file_relation.to.0.clone());
                push_ffa_edge(&mut edges, file_relation, &entity_by_id);
            }
        }
    }

    let mut nodes = selected_ids
        .iter()
        .filter_map(|id| entity_by_id.get(id.as_str()))
        .map(|entity| ffa_graph_node(entity, *degree_by_id.get(&entity.id.0).unwrap_or(&0)))
        .collect::<Vec<_>>();
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

    Ok(RustokFfaGraph {
        schema: RUSTOK_FFA_SURFACE_GRAPH_SCHEMA.to_string(),
        snapshot: snapshot_id(snapshot),
        surface: Some(surface_key),
        nodes,
        edges,
        diagnostics: ffa_diagnostics(snapshot, Some(module), Some(surface)),
        omitted: GraphOmitted {
            nodes: total_nodes.saturating_sub(max_nodes.min(total_nodes)),
            edges: total_edges.saturating_sub(max_edges.min(total_edges)),
            reason: "rustok_ffa_graph_limits".to_string(),
        },
    })
}

pub fn build_rustok_ffa_violations_graph(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokFfaGraph {
    let diagnostics = ffa_diagnostics(snapshot, module, surface);
    let entity_by_stable = snapshot
        .entities
        .iter()
        .map(|entity| (entity.stable_key.0.as_str(), entity))
        .collect::<HashMap<_, _>>();
    let degree_by_id = degree_by_id(snapshot);
    let mut node_keys = BTreeSet::new();
    let mut edges = Vec::new();

    for diagnostic in &diagnostics {
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
                from: surface_key.clone(),
                to: layer_key.clone(),
                kind: "violates".to_string(),
                evidence: diagnostic_evidence(diagnostic),
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
                    evidence: diagnostic_evidence(diagnostic),
                });
            }
        }
    }

    let total_nodes = node_keys.len();
    let total_edges = edges.len();
    let mut nodes = node_keys
        .iter()
        .filter_map(|stable_key| entity_by_stable.get(stable_key.as_str()))
        .map(|entity| ffa_graph_node(entity, *degree_by_id.get(&entity.id.0).unwrap_or(&0)))
        .collect::<Vec<_>>();
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

    RustokFfaGraph {
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
    }
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

pub fn build_graph_pagerank(
    snapshot: &CanonicalSnapshot,
    limit: usize,
    kind: Option<&str>,
    damping: f64,
    max_iterations: usize,
    tolerance: f64,
    max_relation_ids: usize,
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
            snapshot: snapshot
                .snapshot
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
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
        let dangling = scores
            .iter()
            .enumerate()
            .filter(|(index, _)| outgoing[*index].is_empty())
            .map(|(_, score)| *score)
            .sum::<f64>();
        let base = (1.0 - damping) / entity_count as f64 + damping * dangling / entity_count as f64;
        let mut next = vec![base; entity_count];
        for (from, targets) in outgoing.iter().enumerate() {
            if targets.is_empty() {
                continue;
            }
            let contribution = damping * scores[from] / targets.len() as f64;
            for &to in targets {
                next[to] += contribution;
            }
        }
        let delta = scores
            .iter()
            .zip(&next)
            .map(|(previous, current)| (previous - current).abs())
            .sum::<f64>();
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
        snapshot: snapshot
            .snapshot
            .as_ref()
            .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone()),
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

#[derive(Debug, Clone, Default)]
struct FfaSurfaceDetails {
    layers: BTreeSet<String>,
    files: BTreeSet<String>,
}

fn ffa_surface_index(
    snapshot: &CanonicalSnapshot,
) -> BTreeMap<(String, String), FfaSurfaceDetails> {
    let entity_by_id = entity_by_id(snapshot);
    let mut index = BTreeMap::<(String, String), FfaSurfaceDetails>::new();
    for entity in &snapshot.entities {
        if let Some((module, surface)) = parse_ffa_surface_key(&entity.stable_key.0) {
            index
                .entry((module.to_string(), surface.to_string()))
                .or_default();
        }
    }
    for relation in &snapshot.relations {
        let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
            continue;
        };
        let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
            continue;
        };
        if matches!(relation.kind, RelationKind::Contains)
            && let (Some((module, surface)), Some((_, _, role))) = (
                parse_ffa_surface_key(&from.stable_key.0),
                parse_ffa_layer_key(&to.stable_key.0),
            )
        {
            index
                .entry((module.to_string(), surface.to_string()))
                .or_default()
                .layers
                .insert(role.to_string());
        }
        if matches!(relation.kind, RelationKind::ImplementedBy)
            && let Some((module, surface, role)) = parse_ffa_layer_key(&from.stable_key.0)
            && to.stable_key.0.starts_with("file://")
        {
            let details = index
                .entry((module.to_string(), surface.to_string()))
                .or_default();
            details.layers.insert(role.to_string());
            details.files.insert(to.stable_key.0.clone());
        }
    }
    index
}

fn entity_by_id(snapshot: &CanonicalSnapshot) -> HashMap<&str, &Entity> {
    snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.0.as_str(), entity))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn page_builder_graph_from_selection(
    snapshot: &CanonicalSnapshot,
    schema: &str,
    root: Option<String>,
    selected_ids: BTreeSet<String>,
    mut edges: Vec<RustokPageBuilderGraphEdge>,
    diagnostics: Vec<Diagnostic>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokPageBuilderGraph {
    let entity_by_id = entity_by_id(snapshot);
    let total_nodes = selected_ids.len();
    let total_edges = edges.len();
    let mut nodes = selected_ids
        .iter()
        .filter_map(|id| entity_by_id.get(id.as_str()))
        .map(|entity| RustokPageBuilderGraphNode {
            id: entity.stable_key.0.clone(),
            kind: serialized_name(&entity.kind),
            name: entity.name.clone(),
            source: entity.source.as_ref().map(|source| {
                source.line_start.map_or_else(
                    || source.path.clone(),
                    |line| format!("{}:{line}", source.path),
                )
            }),
        })
        .collect::<Vec<_>>();
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

    RustokPageBuilderGraph {
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
    }
}

fn push_page_builder_edge(
    edges: &mut Vec<RustokPageBuilderGraphEdge>,
    relation: &Relation,
    entity_by_id: &HashMap<&str, &Entity>,
) {
    let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
        return;
    };
    let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
        return;
    };
    edges.push(RustokPageBuilderGraphEdge {
        from: from.stable_key.0.clone(),
        to: to.stable_key.0.clone(),
        kind: serialized_name(&relation.kind),
        evidence: relation_evidence(relation),
    });
}

fn is_page_builder_entity(entity: &Entity) -> bool {
    matches!(
        &entity.kind,
        athanor_domain::EntityKind::Other(kind) if kind.starts_with("rustok_page_builder_")
    )
}

fn page_builder_diagnostics(snapshot: &CanonicalSnapshot, module: Option<&str>) -> Vec<Diagnostic> {
    let mut diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                &diagnostic.kind,
                DiagnosticKind::Other(kind) if kind.starts_with("rustok_page_builder_")
            )
        })
        .filter(|diagnostic| {
            module.is_none_or(|module| {
                diagnostic
                    .payload
                    .get("module")
                    .and_then(serde_json::Value::as_str)
                    == Some(module)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    diagnostics
}

#[derive(Debug, Default)]
struct FbaModuleDetails {
    role: Option<String>,
    status: Option<String>,
    contract_version: Option<String>,
    ports: BTreeSet<String>,
    operations: BTreeSet<String>,
    dependencies: BTreeSet<String>,
}

fn fba_module_index(snapshot: &CanonicalSnapshot) -> BTreeMap<String, FbaModuleDetails> {
    let mut index = BTreeMap::<String, FbaModuleDetails>::new();
    for entity in &snapshot.entities {
        if let Some(module) = parse_fba_module_key(&entity.stable_key.0) {
            let details = index.entry(module.to_string()).or_default();
            if details.role.is_none() {
                details.role = entity
                    .payload
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
        }
        if let Some((module, _)) = parse_fba_contract_key(&entity.stable_key.0) {
            let details = index.entry(module.to_string()).or_default();
            details.contract_version = entity
                .payload
                .get("contract_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
        }
        if let Some((module, port)) = parse_fba_port_key(&entity.stable_key.0) {
            index
                .entry(module.to_string())
                .or_default()
                .ports
                .insert(port.to_string());
        }
        if let Some((module, port, operation)) = parse_fba_operation_key(&entity.stable_key.0) {
            index
                .entry(module.to_string())
                .or_default()
                .operations
                .insert(format!("{port}.{operation}"));
        }
        if let Some((consumer, provider, profile)) = parse_fba_dependency_key(&entity.stable_key.0)
        {
            index
                .entry(consumer.to_string())
                .or_default()
                .dependencies
                .insert(format!("{provider}:{profile}"));
        }
    }
    for diagnostic in fba_diagnostics(snapshot, None) {
        if let Some(module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        {
            index.entry(module.to_string()).or_default();
        }
    }
    for fact in &snapshot.facts {
        if matches!(&fact.kind, athanor_domain::FactKind::Other(kind) if kind == "rustok_fba_registry")
            && let Some(module) = fact.value.get("module").and_then(serde_json::Value::as_str)
        {
            let details = index.entry(module.to_string()).or_default();
            details.role = fact
                .value
                .get("role")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or(details.role.take());
            details.status = fact
                .value
                .get("status")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or(details.status.take());
            details.contract_version = fact
                .value
                .get("contract_version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or(details.contract_version.take());
        }
    }
    index
}

fn fba_diagnostics(snapshot: &CanonicalSnapshot, module: Option<&str>) -> Vec<Diagnostic> {
    let mut diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(&diagnostic.kind, DiagnosticKind::Other(kind) if kind.starts_with("rustok_fba_"))
                && module.is_none_or(|module| {
                    diagnostic
                        .payload
                        .get("module")
                        .and_then(serde_json::Value::as_str)
                        == Some(module)
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    diagnostics
}

fn diagnostics_by_module(diagnostics: &[Diagnostic]) -> BTreeMap<String, Vec<String>> {
    let mut by_module = BTreeMap::<String, Vec<String>>::new();
    for diagnostic in diagnostics {
        let Some(module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        by_module
            .entry(module.to_string())
            .or_default()
            .push(serialized_name(&diagnostic.kind));
    }
    by_module
}

#[allow(clippy::too_many_arguments)]
fn fba_graph_from_selection(
    snapshot: &CanonicalSnapshot,
    schema: &str,
    root: Option<String>,
    selected_ids: BTreeSet<String>,
    mut edges: Vec<RustokFbaGraphEdge>,
    diagnostics: Vec<Diagnostic>,
    max_nodes: usize,
    max_edges: usize,
) -> RustokFbaGraph {
    let entity_by_id = entity_by_id(snapshot);
    let total_nodes = selected_ids.len();
    let total_edges = edges.len();
    let mut nodes = selected_ids
        .iter()
        .filter_map(|id| entity_by_id.get(id.as_str()))
        .filter(|entity| is_fba_entity(entity) || entity.stable_key.0.starts_with("file://"))
        .map(|entity| fba_graph_node(entity))
        .collect::<Vec<_>>();
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

    RustokFbaGraph {
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
    }
}

fn is_fba_entity(entity: &Entity) -> bool {
    entity.stable_key.0.starts_with("fba_")
}

fn fba_graph_node(entity: &Entity) -> RustokFbaGraphNode {
    RustokFbaGraphNode {
        id: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity.source.as_ref().map(|source| {
            source.line_start.map_or_else(
                || source.path.clone(),
                |line| format!("{}:{line}", source.path),
            )
        }),
    }
}

fn push_fba_edge(
    edges: &mut Vec<RustokFbaGraphEdge>,
    relation: &Relation,
    entity_by_id: &HashMap<&str, &Entity>,
) {
    let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
        return;
    };
    let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
        return;
    };
    edges.push(RustokFbaGraphEdge {
        from: from.stable_key.0.clone(),
        to: to.stable_key.0.clone(),
        kind: serialized_name(&relation.kind),
        evidence: relation_evidence(relation),
    });
}

fn parse_fba_module_key(stable_key: &str) -> Option<&str> {
    let rest = stable_key.strip_prefix("fba_module://")?;
    (!rest.contains('/')).then_some(rest)
}

fn parse_fba_contract_key(stable_key: &str) -> Option<(&str, &str)> {
    let rest = stable_key.strip_prefix("fba_contract://")?;
    let mut parts = rest.splitn(2, '/');
    Some((parts.next()?, parts.next()?))
}

fn parse_fba_port_key(stable_key: &str) -> Option<(&str, &str)> {
    let rest = stable_key.strip_prefix("fba_port://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let port = parts.next()?;
    parts.next().is_none().then_some((module, port))
}

fn parse_fba_operation_key(stable_key: &str) -> Option<(&str, &str, &str)> {
    let rest = stable_key.strip_prefix("fba_operation://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let port = parts.next()?;
    let operation = parts.next()?;
    parts.next().is_none().then_some((module, port, operation))
}

fn parse_fba_dependency_key(stable_key: &str) -> Option<(&str, &str, &str)> {
    let rest = stable_key.strip_prefix("fba_dependency://")?;
    let mut parts = rest.split('/');
    let consumer = parts.next()?;
    let provider = parts.next()?;
    let profile = parts.next()?;
    parts
        .next()
        .is_none()
        .then_some((consumer, provider, profile))
}

fn ffa_diagnostics(
    snapshot: &CanonicalSnapshot,
    module: Option<&str>,
    surface: Option<&str>,
) -> Vec<Diagnostic> {
    let mut diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(&diagnostic.kind, DiagnosticKind::Other(kind) if kind.starts_with("rustok_ffa_"))
                && module.is_none_or(|module| {
                    diagnostic
                        .payload
                        .get("module")
                        .and_then(serde_json::Value::as_str)
                        == Some(module)
                })
                && surface.is_none_or(|surface| {
                    diagnostic
                        .payload
                        .get("surface")
                        .and_then(serde_json::Value::as_str)
                        == Some(surface)
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    diagnostics
}

fn diagnostics_by_surface(diagnostics: &[Diagnostic]) -> BTreeMap<(String, String), Vec<String>> {
    let mut by_surface = BTreeMap::<(String, String), Vec<String>>::new();
    for diagnostic in diagnostics {
        let Some(module) = diagnostic
            .payload
            .get("module")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(surface) = diagnostic
            .payload
            .get("surface")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let kind = serialized_name(&diagnostic.kind);
        by_surface
            .entry((module.to_string(), surface.to_string()))
            .or_default()
            .push(kind);
    }
    by_surface
}

fn ffa_shape(layers: &[String]) -> String {
    if layers.iter().any(|layer| layer == "host_wiring") {
        return "host_wiring".to_string();
    }
    let has_core = layers.iter().any(|layer| layer == "core");
    let has_transport = layers.iter().any(|layer| layer == "transport");
    let has_ui = layers.iter().any(|layer| layer == "ui_leptos");
    if !has_core
        && !has_transport
        && !has_ui
        && layers
            .iter()
            .all(|layer| matches!(layer.as_str(), "crate_root" | "manifest"))
    {
        return "scaffold".to_string();
    }
    match (has_core, has_transport, has_ui) {
        (true, true, true) => "core_transport_ui",
        (true, true, false) => "core_transport",
        (true, false, true) => "core_ui",
        (false, true, true) => "transport_ui",
        (true, false, false) => "core_only",
        (false, true, false) => "transport_only",
        (false, false, true) => "ui_only",
        (false, false, false) => "none",
    }
    .to_string()
}

fn parse_ffa_surface_key(stable_key: &str) -> Option<(&str, &str)> {
    let rest = stable_key.strip_prefix("ffa_surface://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let surface = parts.next()?;
    parts.next().is_none().then_some((module, surface))
}

fn parse_ffa_layer_key(stable_key: &str) -> Option<(&str, &str, &str)> {
    let rest = stable_key.strip_prefix("ffa_layer://")?;
    let mut parts = rest.split('/');
    let module = parts.next()?;
    let surface = parts.next()?;
    let role = parts.next()?;
    parts.next().is_none().then_some((module, surface, role))
}

fn ffa_graph_node(entity: &Entity, _degree: usize) -> RustokFfaGraphNode {
    RustokFfaGraphNode {
        id: entity.stable_key.0.clone(),
        kind: serialized_name(&entity.kind),
        name: entity.name.clone(),
        source: entity.source.as_ref().map(|source| {
            source.line_start.map_or_else(
                || source.path.clone(),
                |line| format!("{}:{line}", source.path),
            )
        }),
    }
}

fn push_ffa_edge(
    edges: &mut Vec<RustokFfaGraphEdge>,
    relation: &Relation,
    entity_by_id: &HashMap<&str, &Entity>,
) {
    let Some(from) = entity_by_id.get(relation.from.0.as_str()) else {
        return;
    };
    let Some(to) = entity_by_id.get(relation.to.0.as_str()) else {
        return;
    };
    edges.push(RustokFfaGraphEdge {
        from: from.stable_key.0.clone(),
        to: to.stable_key.0.clone(),
        kind: serialized_name(&relation.kind),
        evidence: relation_evidence(relation),
    });
}

fn relation_evidence(relation: &Relation) -> Vec<String> {
    relation
        .evidence
        .iter()
        .filter_map(|evidence| {
            evidence.source_file.as_ref().map(|path| {
                evidence
                    .line_start
                    .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
            })
        })
        .collect()
}

fn diagnostic_evidence(diagnostic: &Diagnostic) -> Vec<String> {
    diagnostic
        .evidence
        .iter()
        .filter_map(|evidence| {
            evidence.source_file.as_ref().map(|path| {
                evidence
                    .line_start
                    .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
            })
        })
        .collect()
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> String {
    snapshot
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
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
    fn renders_graph_export_as_graphml_with_escaped_values() {
        let endpoint = entity(
            "ent_endpoint",
            "api://GET:/health?format=<json>",
            EntityKind::ApiEndpoint,
            "health & status",
        );
        let handler = entity(
            "ent_handler",
            "rust://src/lib.rs#health",
            EntityKind::Function,
            "health",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![handler.clone(), endpoint.clone()],
            relations: vec![relation(
                "rel_impl",
                RelationKind::ImplementedBy,
                &endpoint,
                &handler,
            )],
            ..CanonicalSnapshot::default()
        };

        let graphml = graph_export_to_graphml(&build_graph_export(&snapshot, 10, 10));

        assert!(graphml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(graphml.contains("<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">"));
        assert!(graphml.contains("<graph id=\"snap_test\" edgedefault=\"directed\">"));
        assert!(graphml.contains("<node id=\"ent_endpoint\">"));
        assert!(
            graphml
                .contains("<data key=\"stable_key\">api://GET:/health?format=&lt;json&gt;</data>")
        );
        assert!(graphml.contains("<data key=\"name\">health &amp; status</data>"));
        assert!(
            graphml
                .contains("<edge id=\"rel_impl\" source=\"ent_endpoint\" target=\"ent_handler\">")
        );
        assert!(graphml.ends_with("</graphml>\n"));
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
    fn ranks_directed_graph_with_pagerank_and_relation_trace_ids() {
        let first = entity("ent_first", "symbol://first", EntityKind::Function, "first");
        let second = entity(
            "ent_second",
            "symbol://second",
            EntityKind::Function,
            "second",
        );
        let aggregator = entity(
            "ent_aggregator",
            "symbol://aggregator",
            EntityKind::Function,
            "aggregator",
        );
        let sink = entity("ent_sink", "symbol://sink", EntityKind::Function, "sink");
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![
                sink.clone(),
                first.clone(),
                aggregator.clone(),
                second.clone(),
            ],
            relations: vec![
                relation(
                    "rel_first_aggregator",
                    RelationKind::Calls,
                    &first,
                    &aggregator,
                ),
                relation(
                    "rel_second_aggregator",
                    RelationKind::Calls,
                    &second,
                    &aggregator,
                ),
                relation(
                    "rel_aggregator_sink",
                    RelationKind::Calls,
                    &aggregator,
                    &sink,
                ),
            ],
            ..CanonicalSnapshot::default()
        };

        let report = build_graph_pagerank(&snapshot, 3, None, 0.85, 100, 1e-12, 1).unwrap();

        assert_eq!(report.schema, GRAPH_PAGERANK_SCHEMA);
        assert!(report.converged);
        assert_eq!(report.entity_count, 4);
        assert_eq!(report.relation_count, 3);
        assert_eq!(report.omitted, 1);
        assert_eq!(report.ranks[0].entity.stable_key, "symbol://sink");
        assert_eq!(report.ranks[0].rank, 1);
        assert_eq!(
            report.ranks[0].incoming_relations[0].id,
            "rel_aggregator_sink"
        );
        assert_eq!(
            report.ranks[0].incoming_relations[0].from_entity_id,
            "ent_aggregator"
        );
        assert_eq!(
            report.ranks[0].incoming_relations[0].evidence,
            vec!["docs/api/health.md:1"]
        );
        assert_eq!(report.ranks[1].entity.stable_key, "symbol://aggregator");
        assert_eq!(
            report.ranks[1].incoming_relations[0].id,
            "rel_first_aggregator"
        );
        assert_eq!(report.ranks[1].omitted_incoming_relations, 1);
    }

    #[test]
    fn pagerank_kind_filter_does_not_change_full_graph_scores() {
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

        let full = build_graph_pagerank(&snapshot, 10, None, 0.85, 100, 1e-12, 10).unwrap();
        let filtered =
            build_graph_pagerank(&snapshot, 10, Some("function"), 0.85, 100, 1e-12, 10).unwrap();

        assert_eq!(filtered.ranks.len(), 1);
        assert_eq!(
            filtered.ranks[0].entity.stable_key,
            "rust://src/lib.rs#health"
        );
        let full_handler = full
            .ranks
            .iter()
            .find(|entry| entry.entity.stable_key == "rust://src/lib.rs#health")
            .unwrap();
        assert_eq!(filtered.ranks[0].score, full_handler.score);
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

    #[test]
    fn ffa_surface_graph_includes_surface_layers_files_and_diagnostics() {
        let surface = entity(
            "ent_surface",
            "ffa_surface://blog/admin",
            EntityKind::Other("rustok_ffa_surface".to_string()),
            "blog/admin",
        );
        let core = entity(
            "ent_core",
            "ffa_layer://blog/admin/core",
            EntityKind::Other("rustok_ffa_layer".to_string()),
            "blog/admin/core",
        );
        let file = entity(
            "ent_file",
            "file://crates/rustok-blog/admin/src/core.rs",
            EntityKind::File,
            "crates/rustok-blog/admin/src/core.rs",
        );
        let diagnostic = ffa_diagnostic(
            "diag_core",
            "rustok_ffa_core_depends_on_leptos",
            "blog",
            "admin",
            Some("core"),
            Some("crates/rustok-blog/admin/src/core.rs"),
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![surface.clone(), core.clone(), file.clone()],
            relations: vec![
                relation("rel_surface_core", RelationKind::Contains, &surface, &core),
                relation("rel_core_file", RelationKind::ImplementedBy, &core, &file),
            ],
            diagnostics: vec![diagnostic],
            ..CanonicalSnapshot::default()
        };

        let graph = build_rustok_ffa_surface_graph(&snapshot, "blog", "admin", 80, 160).unwrap();

        assert_eq!(graph.schema, RUSTOK_FFA_SURFACE_GRAPH_SCHEMA);
        assert_eq!(graph.surface.as_deref(), Some("ffa_surface://blog/admin"));
        assert_eq!(
            graph
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ffa_layer://blog/admin/core",
                "ffa_surface://blog/admin",
                "file://crates/rustok-blog/admin/src/core.rs",
            ]
        );
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.diagnostics.len(), 1);
    }

    #[test]
    fn ffa_violations_graph_excludes_clean_edges() {
        let surface = entity(
            "ent_surface",
            "ffa_surface://blog/admin",
            EntityKind::Other("rustok_ffa_surface".to_string()),
            "blog/admin",
        );
        let core = entity(
            "ent_core",
            "ffa_layer://blog/admin/core",
            EntityKind::Other("rustok_ffa_layer".to_string()),
            "blog/admin/core",
        );
        let file = entity(
            "ent_file",
            "file://crates/rustok-blog/admin/src/core.rs",
            EntityKind::File,
            "crates/rustok-blog/admin/src/core.rs",
        );
        let snapshot = CanonicalSnapshot {
            entities: vec![surface.clone(), core.clone(), file.clone()],
            relations: vec![
                relation("rel_surface_core", RelationKind::Contains, &surface, &core),
                relation("rel_core_file", RelationKind::ImplementedBy, &core, &file),
            ],
            diagnostics: vec![ffa_diagnostic(
                "diag_core",
                "rustok_ffa_core_depends_on_leptos",
                "blog",
                "admin",
                Some("core"),
                Some("crates/rustok-blog/admin/src/core.rs"),
            )],
            ..CanonicalSnapshot::default()
        };

        let graph =
            build_rustok_ffa_violations_graph(&snapshot, Some("blog"), Some("admin"), 80, 160);

        assert_eq!(graph.schema, RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA);
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| edge.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["evidenced_by", "violates"]
        );
        assert!(
            graph
                .edges
                .iter()
                .all(|edge| edge.kind != "contains" && edge.kind != "implemented_by")
        );
    }

    #[test]
    fn ffa_surface_graph_reports_node_and_edge_omissions() {
        let surface = entity(
            "ent_surface",
            "ffa_surface://blog/admin",
            EntityKind::Other("rustok_ffa_surface".to_string()),
            "blog/admin",
        );
        let core = entity(
            "ent_core",
            "ffa_layer://blog/admin/core",
            EntityKind::Other("rustok_ffa_layer".to_string()),
            "blog/admin/core",
        );
        let file = entity(
            "ent_file",
            "file://crates/rustok-blog/admin/src/core.rs",
            EntityKind::File,
            "crates/rustok-blog/admin/src/core.rs",
        );
        let snapshot = CanonicalSnapshot {
            entities: vec![surface.clone(), core.clone(), file.clone()],
            relations: vec![
                relation("rel_surface_core", RelationKind::Contains, &surface, &core),
                relation("rel_core_file", RelationKind::ImplementedBy, &core, &file),
            ],
            ..CanonicalSnapshot::default()
        };

        let graph = build_rustok_ffa_surface_graph(&snapshot, "blog", "admin", 1, 1).unwrap();

        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.edges.len(), 0);
        assert_eq!(graph.omitted.nodes, 2);
        assert_eq!(graph.omitted.edges, 1);
    }

    #[test]
    fn fba_module_graph_includes_contract_port_operation_and_diagnostics() {
        let module = entity(
            "ent_fba_module",
            "fba_module://inventory",
            EntityKind::Other("rustok_fba_module".to_string()),
            "inventory",
        );
        let contract = entity(
            "ent_fba_contract",
            "fba_contract://inventory/inventory.reservation.v1",
            EntityKind::Other("rustok_fba_contract".to_string()),
            "inventory/inventory.reservation.v1",
        );
        let port = entity(
            "ent_fba_port",
            "fba_port://inventory/InventoryReservationPort",
            EntityKind::Other("rustok_fba_port".to_string()),
            "inventory/InventoryReservationPort",
        );
        let operation = entity(
            "ent_fba_operation",
            "fba_operation://inventory/InventoryReservationPort/reserve_inventory",
            EntityKind::Other("rustok_fba_operation".to_string()),
            "inventory/InventoryReservationPort/reserve_inventory",
        );
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            entities: vec![
                module.clone(),
                contract.clone(),
                port.clone(),
                operation.clone(),
            ],
            relations: vec![
                relation(
                    "rel_module_contract",
                    RelationKind::Contains,
                    &module,
                    &contract,
                ),
                relation(
                    "rel_contract_port",
                    RelationKind::Contains,
                    &contract,
                    &port,
                ),
                relation(
                    "rel_port_operation",
                    RelationKind::Contains,
                    &port,
                    &operation,
                ),
            ],
            diagnostics: vec![fba_diagnostic(
                "diag_fba",
                "rustok_fba_policy_missing",
                "inventory",
                Some("InventoryReservationPort"),
                Some("crates/rustok-inventory/contracts/inventory-fba-registry.json"),
            )],
            ..CanonicalSnapshot::default()
        };

        let graph = build_rustok_fba_module_graph(&snapshot, "inventory", 80, 160).unwrap();

        assert_eq!(graph.schema, RUSTOK_FBA_MODULE_GRAPH_SCHEMA);
        assert_eq!(graph.root.as_deref(), Some("fba_module://inventory"));
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.edges.len(), 3);
        assert_eq!(graph.diagnostics.len(), 1);
    }

    #[test]
    fn fba_violations_graph_excludes_clean_edges() {
        let module = entity(
            "ent_fba_module",
            "fba_module://inventory",
            EntityKind::Other("rustok_fba_module".to_string()),
            "inventory",
        );
        let port = entity(
            "ent_fba_port",
            "fba_port://inventory/InventoryReservationPort",
            EntityKind::Other("rustok_fba_port".to_string()),
            "inventory/InventoryReservationPort",
        );
        let file = entity(
            "ent_file",
            "file://crates/rustok-inventory/contracts/inventory-fba-registry.json",
            EntityKind::File,
            "crates/rustok-inventory/contracts/inventory-fba-registry.json",
        );
        let snapshot = CanonicalSnapshot {
            entities: vec![module, port, file],
            diagnostics: vec![fba_diagnostic(
                "diag_fba",
                "rustok_fba_policy_missing",
                "inventory",
                Some("InventoryReservationPort"),
                Some("crates/rustok-inventory/contracts/inventory-fba-registry.json"),
            )],
            ..CanonicalSnapshot::default()
        };

        let graph = build_rustok_fba_violations_graph(&snapshot, Some("inventory"), 80, 160);

        assert_eq!(graph.schema, RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA);
        assert_eq!(
            graph
                .edges
                .iter()
                .map(|edge| edge.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["violates", "evidenced_by"]
        );
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

    fn ffa_diagnostic(
        id: &str,
        kind: &str,
        module: &str,
        surface: &str,
        role: Option<&str>,
        path: Option<&str>,
    ) -> Diagnostic {
        Diagnostic {
            id: athanor_domain::DiagnosticId(id.to_string()),
            kind: DiagnosticKind::Other(kind.to_string()),
            severity: athanor_domain::Severity::High,
            status: athanor_domain::DiagnosticStatus::Open,
            title: kind.to_string(),
            message: kind.to_string(),
            entities: Vec::new(),
            evidence: vec![Evidence {
                source_file: path.map(str::to_string),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({
                "schema": "rustok.ffa.diagnostic.v1",
                "module": module,
                "surface": surface,
                "role": role,
                "path": path,
            }),
        }
    }

    fn fba_diagnostic(
        id: &str,
        kind: &str,
        module: &str,
        port: Option<&str>,
        path: Option<&str>,
    ) -> Diagnostic {
        Diagnostic {
            id: athanor_domain::DiagnosticId(id.to_string()),
            kind: DiagnosticKind::Other(kind.to_string()),
            severity: athanor_domain::Severity::High,
            status: athanor_domain::DiagnosticStatus::Open,
            title: kind.to_string(),
            message: kind.to_string(),
            entities: Vec::new(),
            evidence: vec![Evidence {
                source_file: path.map(str::to_string),
                line_start: Some(1),
                line_end: Some(1),
                extractor: Some("test".to_string()),
                commit_hash: None,
                confidence: 1.0,
                status: EvidenceStatus::Verified,
            }],
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({
                "schema": "rustok.fba.diagnostic.v1",
                "module": module,
                "port": port,
                "path": path,
            }),
        }
    }

    #[test]
    fn ffa_shape_separates_host_and_scaffold_entries() {
        assert_eq!(ffa_shape(&["host_wiring".to_string()]), "host_wiring");
        assert_eq!(
            ffa_shape(&["crate_root".to_string(), "manifest".to_string()]),
            "scaffold"
        );
        assert_eq!(
            ffa_shape(&[
                "core".to_string(),
                "transport".to_string(),
                "ui_leptos".to_string(),
            ]),
            "core_transport_ui"
        );
    }
}
