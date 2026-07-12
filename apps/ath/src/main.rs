use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    AdapterTrustListOptions, AdapterTrustOptions, AdapterTrustReport, AffectedCheckOptions,
    AffectedCheckReport, ApiCleanupOptions, ApiCleanupReport, ApiContractDiff, ApiDiffOptions,
    ApiRetentionOverrides, ApiSnapshotOptions, ApiSnapshotReport, BenchmarkOptions,
    BenchmarkReport, BenchmarkSize, CapabilitiesOptions, CapabilitiesReport, ChangeMapOptions,
    ChangeMapReport, ChangedValidationOptions, ContextLimitOverrides, ContextOptions,
    CoverageOptions, CoverageReport, DEFAULT_CAPABILITIES_LIMIT, DEFAULT_CONFIDENCE_THRESHOLD,
    DiagnosticCheckOptions, DiagnosticCheckReport, DiagnosticScope, DocsApplyPatchOptions,
    DocsApplyPatchReport, DocsCheckOptions, DocsCheckReport, DocsDriftOptions, DocsDriftReport,
    DocsProposeFixOptions, DocsProposeFixReport, EntityExplanation, ExplainOptions,
    GenerationOptions, GraphCycles, GraphCyclesOptions, GraphExportOptions,
    GraphFbaDependenciesOptions, GraphFbaModuleOptions, GraphFbaPortOptions,
    GraphFbaViolationsOptions, GraphFfaSurfaceOptions, GraphFfaViolationsOptions, GraphHubs,
    GraphHubsOptions, GraphPageBuilderConsumerOptions, GraphPageBuilderProviderOptions,
    GraphPageBuilderViolationsOptions, GraphPageRank, GraphPageRankOptions, GraphPath,
    GraphPathOptions, GraphRelated, GraphRelatedOptions, HtmlReportOptions, ImpactAnalysis,
    ImpactOptions, IndexOptions, IndexReport, InitOptions, OperationsDocsCheckOptions,
    OperationsDocsCheckReport, OverviewOptions, ProjectRegisterOptions, ProjectRegistration,
    ProjectRegistryOptions, ProjectRegistryReport, ProjectUnregisterOptions, RepairApplyOptions,
    RepairApplyReport, RepairCleanupOptions, RepairCleanupReport, RepairInspectOptions,
    RepairInspectReport, RepairRecoverCanonicalOptions, RepairRecoverCanonicalReport,
    RepairRegenerateOptions, RepairRegenerateReport, RepositoryOverview, RustokArchitectureContext,
    RustokArchitectureContextOptions, RustokFbaAudit, RustokFbaAuditOptions, RustokFbaGraph,
    RustokFfaAudit, RustokFfaAuditOptions, RustokFfaGraph, RustokPageBuilderAudit,
    RustokPageBuilderAuditOptions, RustokPageBuilderGraph, WikiOptions, apply_repair,
    benchmark_index, capabilities_project_with_composition, change_map_project_with_composition,
    check_affected_with_composition, check_docs, check_operations_docs_with_composition,
    check_project_with_composition, cleanup_api_contracts, cleanup_repair,
    context_project_with_composition, coverage_project_with_composition,
    default_adapter_trust_path, default_project_registry_path, diff_api_contracts,
    docs_apply_patch, docs_drift, docs_propose_fix, explain_project_with_composition,
    generate_project_with_composition, graph_fba_dependencies, graph_fba_module, graph_fba_port,
    graph_fba_violations, graph_ffa_surface, graph_ffa_violations, graph_page_builder_consumer,
    graph_page_builder_provider, graph_page_builder_violations, impact_project_with_composition,
    index_project_with_composition, init_project, inspect_repair, list_adapter_plugin_trust,
    list_registered_projects, overview_project_with_composition, project_html_report, project_wiki,
    recover_canonical_repair, regenerate_repair, register_project, resolve_registered_project,
    rustok_architecture_context, rustok_fba_audit, rustok_ffa_audit, rustok_page_builder_audit,
    snapshot_api_contract, trust_adapter_plugin, unregister_project, untrust_adapter_plugin,
    validate_changed,
};
use athanor_domain::ContextLevel;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ContextLevelArg {
    Summary,
    Normal,
    Deep,
    Full,
}

impl From<ContextLevelArg> for ContextLevel {
    fn from(value: ContextLevelArg) -> Self {
        match value {
            ContextLevelArg::Summary => Self::Summary,
            ContextLevelArg::Normal => Self::Normal,
            ContextLevelArg::Deep => Self::Deep,
            ContextLevelArg::Full => Self::Full,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DiagnosticScopeArg {
    Affected,
    Api,
    Docs,
    Env,
    Scripts,
    Deployment,
    Runbooks,
    #[value(name = "rustok-ffa")]
    RustokFfa,
    #[value(name = "rustok-fba")]
    RustokFba,
    #[value(name = "rustok-page-builder")]
    RustokPageBuilder,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphExportFormatArg {
    Json,
    Graphml,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BenchSizeArg {
    Small,
    Medium,
    Large,
}

impl From<BenchSizeArg> for BenchmarkSize {
    fn from(value: BenchSizeArg) -> Self {
        match value {
            BenchSizeArg::Small => Self::Small,
            BenchSizeArg::Medium => Self::Medium,
            BenchSizeArg::Large => Self::Large,
        }
    }
}

impl DiagnosticScopeArg {
    fn diagnostic_scope(self) -> Option<DiagnosticScope> {
        match self {
            Self::Affected => None,
            Self::Api => Some(DiagnosticScope::Api),
            Self::Docs => Some(DiagnosticScope::Docs),
            Self::Env => Some(DiagnosticScope::Env),
            Self::Scripts => Some(DiagnosticScope::Scripts),
            Self::Deployment => Some(DiagnosticScope::Deployment),
            Self::Runbooks => Some(DiagnosticScope::Runbooks),
            Self::RustokFfa => Some(DiagnosticScope::RustokFfa),
            Self::RustokFba => Some(DiagnosticScope::RustokFba),
            Self::RustokPageBuilder => Some(DiagnosticScope::RustokPageBuilder),
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "ath", version, about = "Athanor command line interface")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize Athanor metadata in a project.
    Init {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Index project files and export JSONL read-models.
    Index {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Path to write adapter validation reports when indexing fails validation.
        #[arg(long)]
        validation_report: Option<PathBuf>,
        /// Path to write successful validation-only result JSON.
        #[arg(long)]
        validation_result: Option<PathBuf>,
        /// Validate adapter contracts without writing snapshots, state, or read models.
        #[arg(long)]
        validate_only: bool,
        /// Print the complete index report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run synthetic indexing benchmark fixtures.
    Bench {
        /// Benchmark fixture size.
        #[arg(long, value_enum, default_value_t = BenchSizeArg::Small)]
        size: BenchSizeArg,
        /// Fixture root to recreate. Defaults to a temporary directory.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Keep the generated fixture after the benchmark.
        #[arg(long)]
        keep_fixture: bool,
        /// Print the complete benchmark report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Update the project index from changed files.
    Update {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Only process files changed since the last index state.
        #[arg(long)]
        changed: bool,
        /// Print the complete update report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Validate changed source files through extractors without writing a snapshot.
    ValidateChanged {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Validate a specific source file. Repeat to validate multiple files instead of Git changes.
        #[arg(long = "file")]
        files: Vec<PathBuf>,
        /// Print the complete changed-file validation report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Build a task-focused context pack from the latest canonical snapshot.
    Context {
        /// Task or question used to select relevant project knowledge. Optional with --diff.
        task: Option<String>,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete context pack as JSON.
        #[arg(long)]
        json: bool,
        /// Context detail level and its default limits.
        #[arg(long, value_enum, default_value_t = ContextLevelArg::Normal)]
        level: ContextLevelArg,
        /// Approximate maximum serialized tokens.
        #[arg(long = "budget")]
        max_tokens: Option<usize>,
        /// Maximum number of source files.
        #[arg(long)]
        max_files: Option<usize>,
        /// Maximum number of canonical entities.
        #[arg(long)]
        max_entities: Option<usize>,
        /// Maximum number of diagnostics.
        #[arg(long)]
        max_diagnostics: Option<usize>,
        /// Maximum relation traversal depth.
        #[arg(long)]
        max_depth: Option<usize>,
        /// Build context from files changed since the last index state.
        #[arg(long)]
        diff: bool,
    },
    /// Explain one canonical entity from the latest snapshot.
    Explain {
        /// Exact canonical stable key, for example api://POST:/login.
        stable_key: String,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete explanation as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Summarize the latest canonical snapshot for repository orientation.
    Overview {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print the complete overview as JSON.
        #[arg(long)]
        json: bool,
        /// Maximum number of ranked items per section.
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
    /// Calculate direct and transitive blast radius of changes.
    Impact {
        /// Target entity stable key (e.g. api://POST:/login) or source file path (e.g. src/auth.rs).
        target: Option<String>,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete blast radius analysis as JSON.
        #[arg(long)]
        json: bool,
        /// Analyze the impact of all modified files in the working directory compared to index state.
        #[arg(long)]
        diff: bool,
        /// Maximum relation traversal depth.
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
    },
    /// Build a bounded map of code, contracts, tests, docs, and operations likely affected by a change.
    ChangeMap {
        /// Task description used to find direct canonical matches. Optional with --target or --diff.
        task: Option<String>,
        /// Exact canonical stable key or source path to use as a direct root.
        #[arg(long)]
        target: Option<String>,
        /// Build the map from files changed since the last index state.
        #[arg(long)]
        diff: bool,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Maximum number of canonical entities returned.
        #[arg(long, default_value_t = 30)]
        max_entities: usize,
        /// Maximum number of source files returned.
        #[arg(long, default_value_t = 20)]
        max_files: usize,
        /// Maximum number of open diagnostics returned.
        #[arg(long, default_value_t = 20)]
        max_diagnostics: usize,
        /// Maximum relation traversal depth.
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
        /// Print the complete change map as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show open diagnostics from the latest canonical snapshot.
    Check {
        /// Diagnostic scope to inspect.
        #[arg(value_enum)]
        scope: DiagnosticScopeArg,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete diagnostic report as JSON.
        #[arg(long)]
        json: bool,
        /// Fail on open API diagnostics or breaking contract changes.
        #[arg(long)]
        strict: bool,
    },
    /// Check editable documentation against the configured completeness policy.
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
    },
    /// Validate and inspect the effective project configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Snapshot or compare the public API contract.
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
    /// Build a Markdown wiki from the latest canonical snapshot.
    Wiki {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Wiki output directory. Relative paths are resolved from the project root.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Build generated reports from the latest canonical snapshot.
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    /// Publish JSONL, Markdown, and HTML as one immutable generated generation.
    Generate {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Query and export the canonical entity graph.
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    /// Manage explicit repository identities for future daemon and MCP use.
    Projects {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect and manage trusted adapter plugin manifests.
    Plugins {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Inspect repairable local Athanor artifacts.
    Repair {
        #[command(subcommand)]
        command: RepairCommand,
    },
    /// Search the project's knowledge base.
    Search {
        /// Search query terms.
        query: String,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Maximum number of search results to return.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Print the search results as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Start Model Context Protocol (MCP) stdio server.
    Mcp {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// List registered repositories.
    List {
        /// Override the user-level project registry path.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Print the registry as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Register one repository under an explicit project id.
    Add {
        /// Stable project id used by daemon and agent requests.
        project_id: String,
        /// Repository root to register.
        path: PathBuf,
        /// Override the user-level project registry path.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Print the updated registry as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove one project id from the registry.
    Remove {
        /// Exact registered project id.
        project_id: String,
        /// Override the user-level project registry path.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Print the updated registry as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Resolve one exact project id to its canonical repository root.
    Resolve {
        /// Exact registered project id.
        project_id: String,
        /// Override the user-level project registry path.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Print the registration as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    /// List discovered adapter plugin manifests and trust status.
    List {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Override the user-level adapter trust store path.
        #[arg(long)]
        trust_store: Option<PathBuf>,
        /// Print the trust report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Trust one adapter plugin manifest by path and current content hash.
    Trust {
        /// Path to an adapter manifest, for example .athanor/plugins/example/athanor-adapter.json.
        manifest: PathBuf,
        /// Override the user-level adapter trust store path.
        #[arg(long)]
        trust_store: Option<PathBuf>,
        /// Print the trust report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove trust for one adapter plugin manifest by path.
    Untrust {
        /// Path to an adapter manifest.
        manifest: PathBuf,
        /// Override the user-level adapter trust store path.
        #[arg(long)]
        trust_store: Option<PathBuf>,
        /// Print the trust report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum GraphCommand {
    /// Export a bounded JSON graph from the latest canonical snapshot.
    Export {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Export format.
        #[arg(long, value_enum, default_value_t = GraphExportFormatArg::Json)]
        format: GraphExportFormatArg,
        /// Maximum number of graph nodes to include.
        #[arg(long, default_value_t = 500)]
        max_entities: usize,
        /// Maximum number of graph edges to include.
        #[arg(long, default_value_t = 2_000)]
        max_relations: usize,
    },
    /// Explore entities related to one exact canonical stable key.
    Related {
        /// Exact canonical stable key to use as the graph root.
        stable_key: String,
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Maximum relation distance from the root.
        #[arg(long, default_value_t = 1)]
        depth: usize,
        /// Maximum number of graph nodes to include.
        #[arg(long, default_value_t = 50)]
        max_entities: usize,
        /// Maximum number of graph edges to include.
        #[arg(long, default_value_t = 100)]
        max_relations: usize,
        /// Print the complete related graph as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Find a shortest canonical relation path between two exact stable keys.
    Path {
        /// Exact canonical stable key for the source entity.
        from_stable_key: String,
        /// Exact canonical stable key for the target entity.
        to_stable_key: String,
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Maximum relation distance to search.
        #[arg(long, default_value_t = 6)]
        max_depth: usize,
        /// Maximum number of entities to visit during the search.
        #[arg(long, default_value_t = 10_000)]
        max_visited: usize,
        /// Print the complete path report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Rank canonical graph hubs by degree centrality.
    Hubs {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Maximum number of ranked hubs to return.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Optional serialized entity kind, such as module or api_endpoint.
        #[arg(long)]
        kind: Option<String>,
        /// Maximum incoming and outgoing relation ids retained per hub.
        #[arg(long, default_value_t = 20)]
        max_relation_ids: usize,
        /// Print the complete hub report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Rank canonical entities by directed PageRank centrality.
    Pagerank {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Maximum number of ranked entities to return.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Optional serialized entity kind, such as module or api_endpoint.
        #[arg(long)]
        kind: Option<String>,
        /// PageRank damping factor.
        #[arg(long, default_value_t = 0.85)]
        damping: f64,
        /// Maximum PageRank iterations.
        #[arg(long, default_value_t = 100)]
        max_iterations: usize,
        /// Convergence tolerance for total score delta.
        #[arg(long, default_value_t = 1e-8)]
        tolerance: f64,
        /// Maximum incoming canonical relation ids retained per result.
        #[arg(long, default_value_t = 20)]
        max_relation_ids: usize,
        /// Print the complete PageRank report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Find bounded directed cycles in canonical relations.
    Cycles {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Maximum number of unique cycles to return.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Maximum number of relations in one cycle.
        #[arg(long, default_value_t = 8)]
        max_depth: usize,
        /// Maximum number of graph entities used as cycle search roots.
        #[arg(long, default_value_t = 1_000)]
        max_starts: usize,
        /// Print the complete cycle report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ReportCommand {
    /// Build a self-contained static HTML report.
    Html {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Report output directory. Relative paths are resolved from the project root.
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum RepairCommand {
    /// Inspect generated pointers, snapshots, and orphaned artifacts without changing files.
    Inspect {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print the complete repair inspection as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove orphaned immutable canonical and generated artifacts.
    Cleanup {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print the planned removals without deleting anything.
        #[arg(long)]
        dry_run: bool,
        /// Number of newest orphan canonical snapshots to retain.
        #[arg(long, default_value_t = 0)]
        keep_canonical: usize,
        /// Number of newest orphan generated generations to retain.
        #[arg(long, default_value_t = 0)]
        keep_generated: usize,
        /// Remove only orphan generated generations, leaving canonical snapshots untouched.
        #[arg(long)]
        generated_only: bool,
        /// Print the complete cleanup report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Regenerate stale or missing coordinated generated outputs.
    Regenerate {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Report whether regeneration is needed without writing outputs.
        #[arg(long)]
        dry_run: bool,
        /// Print the complete regeneration report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Recover a missing or invalid canonical latest pointer.
    RecoverCanonical {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Report the selected snapshot without writing latest.json.
        #[arg(long)]
        dry_run: bool,
        /// Print the complete recovery report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Apply all deterministic local artifact repairs.
    Apply {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Report the planned repair stages without writing or deleting artifacts.
        #[arg(long)]
        dry_run: bool,
        /// Number of newest orphan canonical snapshots to retain during cleanup.
        #[arg(long, default_value_t = 0)]
        keep_canonical: usize,
        /// Number of newest orphan generated generations to retain during cleanup.
        #[arg(long, default_value_t = 0)]
        keep_generated: usize,
        /// Remove only orphan generated generations during cleanup.
        #[arg(long)]
        generated_only: bool,
        /// Print the complete apply report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DocsCommand {
    /// Run the editable-documentation completeness gate.
    Check {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete gate report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Report editable documents not verified against the latest snapshot.
    Drift {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete drift report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Write a reviewable patch proposal for documentation policy and drift findings.
    ProposeFix {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Patch proposal output path. Defaults to .athanor/patches/docs/<id>.json.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Print the complete proposal report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Apply a previously generated documentation patch proposal.
    ApplyPatch {
        /// Patch id from .athanor/patches/docs or a project-relative JSON path.
        patch: String,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete apply report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect operational documentation diagnostics.
    Operations {
        #[command(subcommand)]
        command: DocsOperationsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Parse athanor.toml and reject unknown or invalid fields.
    Validate {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the effective configuration as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Report the effective configuration and local compatibility checks.
    Doctor {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DocsOperationsCommand {
    /// Check environment, script, deployment, and runbook documentation diagnostics.
    Check {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete operational documentation report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ApiCommand {
    /// Publish an immutable contract from the latest canonical snapshot.
    Snapshot {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Run API artifact cleanup after the snapshot succeeds.
        #[arg(long, conflicts_with = "no_cleanup")]
        cleanup: bool,
        /// Skip automatic API artifact cleanup for this invocation.
        #[arg(long = "no-cleanup")]
        no_cleanup: bool,
        /// Override retained API contract snapshots when cleanup runs.
        #[arg(long)]
        keep_snapshots: Option<usize>,
        /// Override retained API diff artifacts when cleanup runs.
        #[arg(long)]
        keep_diffs: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Compare two API contract snapshots.
    Diff {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Run API artifact cleanup after the diff succeeds.
        #[arg(long, conflicts_with = "no_cleanup")]
        cleanup: bool,
        /// Skip automatic API artifact cleanup for this invocation.
        #[arg(long = "no-cleanup")]
        no_cleanup: bool,
        /// Override retained API contract snapshots when cleanup runs.
        #[arg(long)]
        keep_snapshots: Option<usize>,
        /// Override retained API diff artifacts when cleanup runs.
        #[arg(long)]
        keep_diffs: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Fail when the selected API diff contains breaking changes.
    BreakingChanges {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show the API registry.
    Registry {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the API registry as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove old API contract snapshots and diff artifacts by retention policy.
    Cleanup {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print planned removals without deleting artifacts.
        #[arg(long)]
        dry_run: bool,
        /// Total API contract snapshots to retain. The latest pointer snapshot is always retained.
        #[arg(long, default_value_t = 2)]
        keep_snapshots: usize,
        /// Diff artifacts to retain when both endpoint snapshots are also retained.
        #[arg(long, default_value_t = 2)]
        keep_diffs: usize,
        /// Print the cleanup report as JSON.
        #[arg(long)]
        json: bool,
    },
}

fn retention_overrides(
    cleanup: bool,
    no_cleanup: bool,
    keep_snapshots: Option<usize>,
    keep_diffs: Option<usize>,
) -> ApiRetentionOverrides {
    ApiRetentionOverrides {
        auto_cleanup: cleanup
            .then_some(true)
            .or_else(|| no_cleanup.then_some(false)),
        keep_snapshots,
        keep_diffs,
    }
}

fn project_registry_path(registry: Option<PathBuf>) -> Result<PathBuf> {
    registry.map_or_else(default_project_registry_path, Ok)
}

fn adapter_trust_path(trust_store: Option<PathBuf>) -> Result<PathBuf> {
    trust_store.map_or_else(default_adapter_trust_path, Ok)
}

fn main() -> Result<()> {
    std::thread::Builder::new()
        .name("ath-cli".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(run_cli)
        .context("failed to start Athanor CLI thread")?
        .join()
        .map_err(|_| anyhow::anyhow!("Athanor CLI thread panicked"))?
}

#[tokio::main]
async fn run_cli() -> Result<()> {
    #[allow(deprecated)]
    {
        athanor_runtime_defaults::install();
    }
    let composition = athanor_runtime_defaults::production();
    init_tracing();
    if handle_manual_coverage_command(&composition).await? {
        return Ok(());
    }
    if handle_manual_capabilities_command(&composition).await? {
        return Ok(());
    }
    if handle_manual_rustok_arch_command().await? {
        return Ok(());
    }
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init { path }) => {
            let report = init_project(InitOptions { root: path })?;
            println!("initialized Athanor project at {}", report.root.display());

            for path in report.created {
                println!("created {}", path.display());
            }
        }
        Some(Command::Index {
            path,
            validation_report,
            validation_result,
            validate_only,
            json,
        }) => {
            let report = index_project_with_composition(
                IndexOptions {
                    root: path,
                    validation_report,
                    validation_result,
                    validate_only,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_index_report(&report, "indexed")?;
            }
        }
        Some(Command::Bench {
            size,
            root,
            keep_fixture,
            json,
        }) => {
            let report = benchmark_index(BenchmarkOptions {
                size: size.into(),
                root,
                keep_fixture,
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_benchmark_report(&report);
            }
        }
        Some(Command::Update {
            path,
            changed,
            json,
        }) => {
            if !changed {
                anyhow::bail!("update requires --changed");
            }
            let report = index_project_with_composition(
                IndexOptions {
                    root: path,
                    validation_report: None,
                    validation_result: None,
                    validate_only: false,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_index_report(&report, "updated")?;
            }
        }
        Some(Command::ValidateChanged { path, files, json }) => {
            let report = validate_changed(ChangedValidationOptions { root: path, files }).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "validated {} changed files through extractors using snapshot {}",
                    report.files_checked, report.snapshot
                );
                println!(
                    "affected files: {} changed, {} removed",
                    report.changed_files, report.removed_files
                );
                println!(
                    "diagnostics: {}, metrics: total {} ms, discovery {} ms, extraction {} ms",
                    report.diagnostics.len(),
                    report.metrics.total_ms,
                    report.metrics.source_discovery_ms,
                    report.metrics.extraction_ms
                );
            }
        }
        Some(Command::Context {
            task,
            path,
            json,
            level,
            max_tokens,
            max_files,
            max_entities,
            max_diagnostics,
            max_depth,
            diff,
        }) => {
            let pack = context_project_with_composition(
                ContextOptions {
                    root: path,
                    task: task.unwrap_or_default(),
                    diff,
                    level: level.into(),
                    limits: ContextLimitOverrides {
                        max_tokens,
                        max_files,
                        max_entities,
                        max_diagnostics,
                        max_depth,
                    },
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&pack)?);
            } else {
                println!("{}", pack.summary);
                for file in &pack.files {
                    println!("file: {file}");
                }
                for scope in &pack.scope {
                    println!("entity: {scope}");
                }
                for diagnostic in &pack.diagnostics {
                    println!("diagnostic: {}", diagnostic.0);
                }
            }
        }
        Some(Command::Explain {
            stable_key,
            path,
            json,
        }) => {
            let explanation = explain_project_with_composition(
                ExplainOptions {
                    root: path,
                    stable_key,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&explanation)?);
            } else {
                print_explanation(&explanation)?;
            }
        }
        Some(Command::Overview { path, json, top }) => {
            let overview = overview_project_with_composition(
                OverviewOptions { root: path, top },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&overview)?);
            } else {
                print_overview(&overview)?;
            }
        }
        Some(Command::Impact {
            target,
            path,
            json,
            diff,
            max_depth,
        }) => {
            let analysis = impact_project_with_composition(
                ImpactOptions {
                    root: path,
                    target,
                    diff,
                    max_depth,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            } else {
                print_impact_analysis(&analysis)?;
            }
        }
        Some(Command::ChangeMap {
            task,
            target,
            diff,
            path,
            max_entities,
            max_files,
            max_diagnostics,
            max_depth,
            json,
        }) => {
            let report = change_map_project_with_composition(
                ChangeMapOptions {
                    root: path,
                    task,
                    target,
                    diff,
                    max_entities,
                    max_files,
                    max_diagnostics,
                    max_depth,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_change_map(&report)?;
            }
        }
        Some(Command::Check {
            scope,
            path,
            json,
            strict,
        }) => {
            if matches!(scope, DiagnosticScopeArg::Affected) {
                if strict {
                    anyhow::bail!("--strict is currently supported only for `ath check api`");
                }
                let report = check_affected_with_composition(
                    AffectedCheckOptions { root: path },
                    &composition,
                )
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_affected_check_report(&report)?;
                }
                if report.counts.total > 0 {
                    anyhow::bail!(
                        "affected check failed with {} open diagnostics",
                        report.counts.total
                    );
                }
                return Ok(());
            }
            let scope = scope
                .diagnostic_scope()
                .expect("non-affected diagnostic scope expected");
            let config = athanor_app::config::load_config(&path)?;
            let is_strict = strict || (scope == DiagnosticScope::Api && config.api.strict);
            let report = check_project_with_composition(
                DiagnosticCheckOptions {
                    root: path.clone(),
                    scope,
                },
                &composition,
            )
            .await?;
            if is_strict {
                if scope != DiagnosticScope::Api {
                    anyhow::bail!("--strict is currently supported only for `ath check api`");
                }
                let diff = diff_api_contracts(ApiDiffOptions {
                    root: path,
                    from: None,
                    to: None,
                    retention: ApiRetentionOverrides {
                        auto_cleanup: Some(false),
                        ..ApiRetentionOverrides::default()
                    },
                })?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "schema": "athanor.api_strict_check.v1",
                            "diagnostics": report,
                            "contract": diff,
                        }))?
                    );
                } else {
                    print_check_report(&report)?;
                    print_api_contract_diff(&diff)?;
                }
                if report.counts.total > 0 || diff.breaking_changes > 0 {
                    anyhow::bail!(
                        "strict API check failed with {} open diagnostics and {} breaking changes",
                        report.counts.total,
                        diff.breaking_changes
                    );
                }
            } else if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_check_report(&report)?;
            }
        }
        Some(Command::Docs {
            command: DocsCommand::Check { path, json },
        }) => {
            let report = check_docs(DocsCheckOptions { root: path }).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_docs_check_report(&report)?;
            }
            if !report.passed {
                anyhow::bail!("documentation completeness gate failed");
            }
        }
        Some(Command::Config {
            command: ConfigCommand::Validate { path, json },
        }) => {
            let config = athanor_app::config::load_config(&path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&config)?);
            } else {
                println!(
                    "configuration at {} is valid",
                    path.join("athanor.toml").display()
                );
                println!("{}", serde_json::to_string_pretty(&config)?);
            }
        }
        Some(Command::Config {
            command: ConfigCommand::Doctor { path, json },
        }) => {
            let config = athanor_app::config::load_config(&path)?;
            let report = serde_json::json!({
                "schema": "athanor.config_doctor.v1",
                "root": path,
                "config": config,
                "checks": [{
                    "name": "storage_backend",
                    "status": "available",
                    "detail": "the configured storage mode is compiled into this build"
                }, {
                    "name": "external_process_adapters",
                    "status": "configured",
                    "detail": "external process adapters require explicit enablement, trust, and allowlisting"
                }]
            });
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("configuration doctor: {}", path.display());
                println!("  storage backend: available");
                println!("  external process adapters: configured");
            }
        }
        Some(Command::Docs {
            command: DocsCommand::Drift { path, json },
        }) => {
            let report = docs_drift(DocsDriftOptions { root: path }).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_docs_drift_report(&report);
            }
        }
        Some(Command::Docs {
            command: DocsCommand::ProposeFix { path, output, json },
        }) => {
            let report = docs_propose_fix(DocsProposeFixOptions { root: path, output }).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_docs_propose_fix_report(&report);
            }
        }
        Some(Command::Docs {
            command: DocsCommand::ApplyPatch { patch, path, json },
        }) => {
            let report = docs_apply_patch(DocsApplyPatchOptions { root: path, patch }).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_docs_apply_patch_report(&report);
            }
        }
        Some(Command::Docs {
            command:
                DocsCommand::Operations {
                    command: DocsOperationsCommand::Check { path, json },
                },
        }) => {
            let report = check_operations_docs_with_composition(
                OperationsDocsCheckOptions { root: path },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_operations_docs_check_report(&report)?;
            }
            if report.counts.total > 0 {
                anyhow::bail!(
                    "operational documentation check failed with {} open diagnostics",
                    report.counts.total
                );
            }
        }
        Some(Command::Api {
            command:
                ApiCommand::Snapshot {
                    path,
                    cleanup,
                    no_cleanup,
                    keep_snapshots,
                    keep_diffs,
                    json,
                },
        }) => {
            let report = snapshot_api_contract(ApiSnapshotOptions {
                root: path,
                retention: retention_overrides(cleanup, no_cleanup, keep_snapshots, keep_diffs),
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_api_snapshot_report(&report);
            }
        }
        Some(Command::Api {
            command:
                ApiCommand::BreakingChanges {
                    from,
                    to,
                    path,
                    json,
                },
        }) => {
            let diff = diff_api_contracts(ApiDiffOptions {
                root: path,
                from,
                to,
                retention: ApiRetentionOverrides {
                    auto_cleanup: Some(false),
                    ..ApiRetentionOverrides::default()
                },
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&diff)?);
            } else {
                print_api_contract_diff(&diff)?;
            }
            if diff.breaking_changes > 0 {
                anyhow::bail!(
                    "API contract contains {} breaking changes",
                    diff.breaking_changes
                );
            }
        }
        Some(Command::Api {
            command:
                ApiCommand::Diff {
                    from,
                    to,
                    path,
                    cleanup,
                    no_cleanup,
                    keep_snapshots,
                    keep_diffs,
                    json,
                },
        }) => {
            let diff = diff_api_contracts(ApiDiffOptions {
                root: path,
                from,
                to,
                retention: retention_overrides(cleanup, no_cleanup, keep_snapshots, keep_diffs),
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&diff)?);
            } else {
                print_api_contract_diff(&diff)?;
            }
        }
        Some(Command::Api {
            command: ApiCommand::Registry { path, json },
        }) => {
            let report =
                athanor_app::query_api_registry(athanor_app::ApiRegistryOptions { root: path })
                    .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_api_registry_report(&report)?;
            }
        }
        Some(Command::Api {
            command:
                ApiCommand::Cleanup {
                    path,
                    dry_run,
                    keep_snapshots,
                    keep_diffs,
                    json,
                },
        }) => {
            let report = cleanup_api_contracts(ApiCleanupOptions {
                root: path,
                dry_run,
                keep_snapshots,
                keep_diffs,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_api_cleanup_report(&report);
            }
        }
        Some(Command::Wiki { path, output }) => {
            let report = project_wiki(WikiOptions { root: path, output }).await?;
            println!(
                "projected {} entities and {} open diagnostics from snapshot {}",
                report.entities, report.open_diagnostics, report.snapshot
            );
            println!("wrote Markdown wiki to {}", report.output_dir.display());
        }
        Some(Command::Report {
            command: ReportCommand::Html { path, output },
        }) => {
            let report = project_html_report(HtmlReportOptions { root: path, output }).await?;
            println!(
                "projected {} entities and {} open diagnostics from snapshot {}",
                report.entities, report.open_diagnostics, report.snapshot
            );
            println!("wrote HTML report to {}", report.output_dir.display());
        }
        Some(Command::Generate { path }) => {
            let report = generate_project_with_composition(
                GenerationOptions {
                    root: path,
                    force: false,
                },
                &composition,
            )
            .await?;
            if report.status == athanor_app::GenerationStatus::UpToDate {
                println!(
                    "current generation {} is already up to date for snapshot {}",
                    report.generation, report.snapshot
                );
            } else {
                println!(
                    "published generation {} from snapshot {}",
                    report.generation, report.snapshot
                );
                println!(
                    "wrote generated outputs to {}",
                    report.generation_dir.display()
                );
                println!(
                    "updated current pointer at {}",
                    report.current_pointer.display()
                );
            }
            println!(
                "generation timings: total={}ms snapshot={}ms jsonl={}ms wiki={}ms html={}ms publish={}ms",
                report.metrics.total_ms,
                report.metrics.snapshot_load_ms,
                report.metrics.jsonl_ms,
                report.metrics.wiki_ms,
                report.metrics.html_ms,
                report.metrics.publish_ms
            );
        }
        Some(Command::Graph { command }) => match command {
            GraphCommand::Export {
                path,
                format,
                max_entities,
                max_relations,
            } => {
                let export = athanor_app::export_graph(GraphExportOptions {
                    root: path,
                    max_entities,
                    max_relations,
                })
                .await?;
                match format {
                    GraphExportFormatArg::Json => {
                        println!("{}", serde_json::to_string_pretty(&export)?);
                    }
                    GraphExportFormatArg::Graphml => {
                        print!("{}", athanor_app::graph_export_to_graphml(&export));
                    }
                }
            }
            GraphCommand::Related {
                stable_key,
                path,
                depth,
                max_entities,
                max_relations,
                json,
            } => {
                let related = athanor_app::related_graph(GraphRelatedOptions {
                    root: path,
                    stable_key,
                    depth,
                    max_entities,
                    max_relations,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&related)?);
                } else {
                    print_related_graph(&related);
                }
            }
            GraphCommand::Path {
                from_stable_key,
                to_stable_key,
                path,
                max_depth,
                max_visited,
                json,
            } => {
                let path_report = athanor_app::shortest_graph_path(GraphPathOptions {
                    root: path,
                    from_stable_key,
                    to_stable_key,
                    max_depth,
                    max_visited,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&path_report)?);
                } else {
                    print_graph_path(&path_report);
                }
            }
            GraphCommand::Hubs {
                path,
                limit,
                kind,
                max_relation_ids,
                json,
            } => {
                let report = athanor_app::graph_hubs(GraphHubsOptions {
                    root: path,
                    limit,
                    kind,
                    max_relation_ids,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_graph_hubs(&report);
                }
            }
            GraphCommand::Pagerank {
                path,
                limit,
                kind,
                damping,
                max_iterations,
                tolerance,
                max_relation_ids,
                json,
            } => {
                let report = athanor_app::graph_pagerank(GraphPageRankOptions {
                    root: path,
                    limit,
                    kind,
                    damping,
                    max_iterations,
                    tolerance,
                    max_relation_ids,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_graph_pagerank(&report);
                }
            }
            GraphCommand::Cycles {
                path,
                limit,
                max_depth,
                max_starts,
                json,
            } => {
                let report = athanor_app::graph_cycles(GraphCyclesOptions {
                    root: path,
                    limit,
                    max_depth,
                    max_starts,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_graph_cycles(&report);
                }
            }
        },
        Some(Command::Projects { command }) => match command {
            ProjectCommand::List { registry, json } => {
                let report = list_registered_projects(ProjectRegistryOptions {
                    registry_path: project_registry_path(registry)?,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_project_registry(&report);
                }
            }
            ProjectCommand::Add {
                project_id,
                path,
                registry,
                json,
            } => {
                let report = register_project(ProjectRegisterOptions {
                    registry_path: project_registry_path(registry)?,
                    project_id,
                    root: path,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_project_registry(&report);
                }
            }
            ProjectCommand::Remove {
                project_id,
                registry,
                json,
            } => {
                let report = unregister_project(ProjectUnregisterOptions {
                    registry_path: project_registry_path(registry)?,
                    project_id,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_project_registry(&report);
                }
            }
            ProjectCommand::Resolve {
                project_id,
                registry,
                json,
            } => {
                let report = resolve_registered_project(
                    ProjectRegistryOptions {
                        registry_path: project_registry_path(registry)?,
                    },
                    &project_id,
                )?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("Resolved from {}", report.registry_path.display());
                    print_project_registration(&report.project);
                }
            }
        },
        Some(Command::Plugins { command }) => match command {
            PluginCommand::List {
                path,
                trust_store,
                json,
            } => {
                let report = list_adapter_plugin_trust(AdapterTrustListOptions {
                    root: path,
                    trust_path: adapter_trust_path(trust_store)?,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_adapter_trust_report(&report);
                }
            }
            PluginCommand::Trust {
                manifest,
                trust_store,
                json,
            } => {
                let report = trust_adapter_plugin(AdapterTrustOptions {
                    trust_path: adapter_trust_path(trust_store)?,
                    manifest_path: manifest,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_adapter_trust_report(&report);
                }
            }
            PluginCommand::Untrust {
                manifest,
                trust_store,
                json,
            } => {
                let report = untrust_adapter_plugin(AdapterTrustOptions {
                    trust_path: adapter_trust_path(trust_store)?,
                    manifest_path: manifest,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_adapter_trust_report(&report);
                }
            }
        },
        Some(Command::Repair { command }) => match command {
            RepairCommand::Inspect { path, json } => {
                let report = inspect_repair(RepairInspectOptions { root: path })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_repair_inspect_report(&report);
                }
            }
            RepairCommand::Cleanup {
                path,
                dry_run,
                keep_canonical,
                keep_generated,
                generated_only,
                json,
            } => {
                let report = cleanup_repair(RepairCleanupOptions {
                    root: path,
                    dry_run,
                    keep_canonical,
                    keep_generated,
                    generated_only,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_repair_cleanup_report(&report);
                }
            }
            RepairCommand::Regenerate {
                path,
                dry_run,
                json,
            } => {
                let report = regenerate_repair(RepairRegenerateOptions {
                    root: path,
                    dry_run,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_repair_regenerate_report(&report);
                }
            }
            RepairCommand::RecoverCanonical {
                path,
                dry_run,
                json,
            } => {
                let report = recover_canonical_repair(RepairRecoverCanonicalOptions {
                    root: path,
                    dry_run,
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_repair_recover_canonical_report(&report);
                }
            }
            RepairCommand::Apply {
                path,
                dry_run,
                keep_canonical,
                keep_generated,
                generated_only,
                json,
            } => {
                let report = apply_repair(RepairApplyOptions {
                    root: path,
                    dry_run,
                    keep_canonical,
                    keep_generated,
                    generated_only,
                })
                .await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_repair_apply_report(&report);
                }
            }
        },
        Some(Command::Search {
            query,
            path,
            limit,
            json,
        }) => {
            let report = athanor_app::search_project(athanor_app::SearchOptions {
                root: path,
                query: query.clone(),
                limit,
            })
            .await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "search results for query \"{}\" in snapshot {} ({} of limit {}):",
                    report.query, report.snapshot, report.returned, report.limit
                );
                if report.results.is_empty() {
                    println!("No results found.");
                } else {
                    for item in &report.results {
                        println!(
                            "[{:.4}] {} ({}) - {}",
                            item.score, item.name, item.kind, item.stable_key
                        );
                        println!("  entity: {}", item.entity_id.0);
                        if let Some(source) = &item.source {
                            println!("  source: {}", source.path);
                        }
                    }
                    if report.truncated {
                        println!(
                            "results truncated by limit; at least {} more result(s) omitted",
                            report.omitted.results_lower_bound
                        );
                    }
                }
            }
        }
        Some(Command::Mcp { path }) => {
            athanor_transport_mcp::run_mcp_server(path).await?;
        }
        None => {
            println!("Athanor {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[derive(Debug)]
struct CoverageFlags {
    path: PathBuf,
    adapter: Option<String>,
    file: Option<PathBuf>,
    limit: usize,
    json: bool,
}

async fn handle_manual_coverage_command(
    composition: &athanor_app::RuntimeComposition,
) -> Result<bool> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [first, rest @ ..] = args.as_slice() else {
        return Ok(false);
    };
    if first != "coverage" {
        return Ok(false);
    }
    if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_coverage_help();
        return Ok(true);
    }
    let flags = parse_coverage_flags(rest)?;
    let report = coverage_project_with_composition(
        CoverageOptions {
            root: flags.path,
            adapter: flags.adapter,
            file: flags.file,
            limit: flags.limit,
        },
        composition,
    )
    .await?;
    if flags.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_coverage_report(&report);
    }
    Ok(true)
}

fn print_coverage_help() {
    println!("Report bounded analysis coverage from the latest canonical snapshot");
    println!();
    println!("Usage: ath coverage [PATH] [--adapter <ID>] [--file <PATH>] [--limit <N>] [--json]");
    println!();
    println!("Options:");
    println!("      --adapter <ID>   Restrict coverage rows to one adapter name");
    println!(
        "      --file <PATH>    Restrict coverage rows to one source file under the project root"
    );
    println!("      --limit <N>      Maximum rows per coverage section [default: 50]");
    println!("      --json           Print the complete coverage report as JSON");
    println!("  -h, --help           Print help");
}

fn parse_coverage_flags(args: &[String]) -> Result<CoverageFlags> {
    let mut path = None;
    let mut adapter = None;
    let mut file = None;
    let mut limit = 50;
    let mut json = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--adapter" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("--adapter requires a value"))?;
                adapter = Some(value.clone());
                index += 2;
            }
            "--file" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("--file requires a value"))?;
                file = Some(PathBuf::from(value));
                index += 2;
            }
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("--limit requires a value"))?;
                limit = value
                    .parse::<usize>()
                    .context("--limit must be a positive integer")?;
                index += 2;
            }
            value if value.starts_with("--") => {
                anyhow::bail!("unknown coverage flag `{value}`");
            }
            value => {
                if path.is_some() {
                    anyhow::bail!("coverage accepts at most one project path");
                }
                path = Some(PathBuf::from(value));
                index += 1;
            }
        }
    }

    Ok(CoverageFlags {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        adapter,
        file,
        limit,
        json,
    })
}

#[derive(Debug)]
struct CapabilitiesFlags {
    path: PathBuf,
    limit: usize,
    confidence_threshold: f32,
    json: bool,
}

async fn handle_manual_capabilities_command(
    composition: &athanor_app::RuntimeComposition,
) -> Result<bool> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [first, rest @ ..] = args.as_slice() else {
        return Ok(false);
    };
    if first != "capabilities" {
        return Ok(false);
    }
    if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_capabilities_help();
        return Ok(true);
    }
    let flags = parse_capabilities_flags(rest)?;
    let report = capabilities_project_with_composition(
        CapabilitiesOptions {
            root: flags.path,
            limit: flags.limit,
            confidence_threshold: flags.confidence_threshold,
        },
        composition,
    )
    .await?;
    if flags.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_capabilities_report(&report);
    }
    Ok(true)
}

fn print_capabilities_help() {
    println!("Report bounded analysis completeness from the latest canonical snapshot");
    println!();
    println!("Usage: ath capabilities [PATH] [--limit <N>] [--min-confidence <F>] [--json]");
    println!();
    println!("Options:");
    println!("      --limit <N>            Maximum rows per completeness section [default: 50]");
    println!(
        "      --min-confidence <F>   Facts below this confidence are reported as low confidence [default: 1.0]"
    );
    println!("      --json                 Print the complete capabilities report as JSON");
    println!("  -h, --help                 Print help");
}

fn parse_capabilities_flags(args: &[String]) -> Result<CapabilitiesFlags> {
    let mut path = None;
    let mut limit = DEFAULT_CAPABILITIES_LIMIT;
    let mut confidence_threshold = DEFAULT_CONFIDENCE_THRESHOLD;
    let mut json = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("--limit requires a value"))?;
                limit = value
                    .parse::<usize>()
                    .context("--limit must be a positive integer")?;
                index += 2;
            }
            "--min-confidence" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("--min-confidence requires a value"))?;
                confidence_threshold = value
                    .parse::<f32>()
                    .context("--min-confidence must be a number")?;
                if !(0.0..=1.0).contains(&confidence_threshold) {
                    anyhow::bail!("--min-confidence must be between 0.0 and 1.0");
                }
                index += 2;
            }
            value if value.starts_with("--") => {
                anyhow::bail!("unknown capabilities flag `{value}`");
            }
            value => {
                if path.is_some() {
                    anyhow::bail!("capabilities accepts at most one project path");
                }
                path = Some(PathBuf::from(value));
                index += 1;
            }
        }
    }

    Ok(CapabilitiesFlags {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        limit,
        confidence_threshold,
        json,
    })
}

async fn handle_manual_rustok_arch_command() -> Result<bool> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [first, second, third, intent, rest @ ..]
            if first == "rustok" && second == "architecture" && third == "context" =>
        {
            let flags = parse_rustok_architecture_context_flags(rest)?;
            let mut options =
                RustokArchitectureContextOptions::bounded(flags.path, intent.clone(), flags.module);
            options.max_modules = flags.max_modules;
            options.max_contracts = flags.max_contracts;
            options.max_interactions = flags.max_interactions;
            options.max_evidence = flags.max_evidence;
            let report = rustok_architecture_context(options).await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_architecture_context(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "rustok" && second == "ffa" && third == "audit" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = rustok_ffa_audit(RustokFfaAuditOptions { root: flags.path }).await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_ffa_audit(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "rustok" && second == "fba" && third == "audit" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = rustok_fba_audit(RustokFbaAuditOptions { root: flags.path }).await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_fba_audit(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "rustok" && second == "page-builder" && third == "audit" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report =
                rustok_page_builder_audit(RustokPageBuilderAuditOptions { root: flags.path })
                    .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_page_builder_audit(&report);
            }
            Ok(true)
        }
        [first, second, third, module, surface, rest @ ..]
            if first == "graph" && second == "ffa" && third == "surface" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = graph_ffa_surface(GraphFfaSurfaceOptions {
                root: flags.path,
                module: module.clone(),
                surface: surface.clone(),
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_ffa_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, module, rest @ ..]
            if first == "graph" && second == "fba" && third == "module" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = graph_fba_module(GraphFbaModuleOptions {
                root: flags.path,
                module: module.clone(),
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_fba_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "page-builder" && third == "provider" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = graph_page_builder_provider(GraphPageBuilderProviderOptions {
                root: flags.path,
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_page_builder_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, module, rest @ ..]
            if first == "graph" && second == "page-builder" && third == "consumer" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = graph_page_builder_consumer(GraphPageBuilderConsumerOptions {
                root: flags.path,
                module: module.clone(),
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_page_builder_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, module, port, rest @ ..]
            if first == "graph" && second == "fba" && third == "port" =>
        {
            let flags = parse_arch_flags(rest, true)?;
            let report = graph_fba_port(GraphFbaPortOptions {
                root: flags.path,
                module: module.clone(),
                port: port.clone(),
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_fba_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "fba" && third == "dependencies" =>
        {
            let flags = parse_fba_dependencies_flags(rest)?;
            let Some(module) = flags.module else {
                anyhow::bail!("graph fba dependencies requires --module <module>");
            };
            let report = graph_fba_dependencies(GraphFbaDependenciesOptions {
                root: flags.path,
                module,
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_fba_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "ffa" && third == "violations" =>
        {
            let flags = parse_ffa_violations_flags(rest)?;
            let report = graph_ffa_violations(GraphFfaViolationsOptions {
                root: flags.path,
                module: flags.module,
                surface: flags.surface,
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_ffa_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "fba" && third == "violations" =>
        {
            let flags = parse_fba_violations_flags(rest)?;
            let report = graph_fba_violations(GraphFbaViolationsOptions {
                root: flags.path,
                module: flags.module,
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_fba_graph(&report);
            }
            Ok(true)
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "page-builder" && third == "violations" =>
        {
            let flags = parse_fba_violations_flags(rest)?;
            let report = graph_page_builder_violations(GraphPageBuilderViolationsOptions {
                root: flags.path,
                module: flags.module,
                max_nodes: flags.max_nodes,
                max_edges: flags.max_edges,
            })
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_rustok_page_builder_graph(&report);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[derive(Debug, Clone)]
struct RustokArchitectureContextFlags {
    path: PathBuf,
    module: Option<String>,
    max_modules: usize,
    max_contracts: usize,
    max_interactions: usize,
    max_evidence: usize,
    json: bool,
}

fn parse_rustok_architecture_context_flags(
    args: &[String],
) -> Result<RustokArchitectureContextFlags> {
    let mut flags = RustokArchitectureContextFlags {
        path: PathBuf::from("."),
        module: None,
        max_modules: 6,
        max_contracts: 16,
        max_interactions: 16,
        max_evidence: 20,
        json: false,
    };
    let mut index = 0;
    while index < args.len() {
        let (target, label) = match args[index].as_str() {
            "--json" => {
                flags.json = true;
                index += 1;
                continue;
            }
            "--path" => (&mut flags.path, "--path"),
            "--module" => {
                index += 1;
                flags.module = Some(
                    args.get(index)
                        .ok_or_else(|| anyhow::anyhow!("--module requires a value"))?
                        .clone(),
                );
                index += 1;
                continue;
            }
            "--max-modules" => {
                index += 1;
                flags.max_modules = parse_positive_limit(args.get(index), "--max-modules")?;
                index += 1;
                continue;
            }
            "--max-contracts" => {
                index += 1;
                flags.max_contracts = parse_positive_limit(args.get(index), "--max-contracts")?;
                index += 1;
                continue;
            }
            "--max-interactions" => {
                index += 1;
                flags.max_interactions =
                    parse_positive_limit(args.get(index), "--max-interactions")?;
                index += 1;
                continue;
            }
            "--max-evidence" => {
                index += 1;
                flags.max_evidence = parse_positive_limit(args.get(index), "--max-evidence")?;
                index += 1;
                continue;
            }
            value => anyhow::bail!("unknown Rustok architecture context flag `{value}`"),
        };
        index += 1;
        *target = PathBuf::from(
            args.get(index)
                .ok_or_else(|| anyhow::anyhow!("{label} requires a value"))?,
        );
        index += 1;
    }
    Ok(flags)
}

fn parse_positive_limit(value: Option<&String>, label: &str) -> Result<usize> {
    let value = value.ok_or_else(|| anyhow::anyhow!("{label} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{label} must be a positive integer"))?;
    if parsed == 0 {
        anyhow::bail!("{label} must be greater than zero");
    }
    Ok(parsed)
}

#[derive(Debug, Clone)]
struct ManualArchFlags {
    path: PathBuf,
    json: bool,
    max_nodes: usize,
    max_edges: usize,
    module: Option<String>,
    surface: Option<String>,
}

fn parse_arch_flags(args: &[String], allow_positional_path: bool) -> Result<ManualArchFlags> {
    let mut path = PathBuf::from(".");
    let mut json = false;
    let mut max_nodes = 80usize;
    let mut max_edges = 160usize;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--path" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--path requires a value");
                };
                path = PathBuf::from(value);
            }
            "--max-nodes" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--max-nodes requires a value");
                };
                max_nodes = value.parse()?;
            }
            "--max-edges" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--max-edges requires a value");
                };
                max_edges = value.parse()?;
            }
            value if allow_positional_path && !value.starts_with('-') => {
                path = PathBuf::from(value);
            }
            other => anyhow::bail!("unknown architecture graph option `{other}`"),
        }
        index += 1;
    }
    Ok(ManualArchFlags {
        path,
        json,
        max_nodes,
        max_edges,
        module: None,
        surface: None,
    })
}

fn parse_ffa_violations_flags(args: &[String]) -> Result<ManualArchFlags> {
    let mut module = None;
    let mut surface = None;
    let mut passthrough = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--module" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--module requires a value");
                };
                module = Some(value.clone());
            }
            "--surface" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--surface requires a value");
                };
                surface = Some(value.clone());
            }
            other => passthrough.push(other.to_string()),
        }
        index += 1;
    }
    let mut flags = parse_arch_flags(&passthrough, false)?;
    flags.module = module;
    flags.surface = surface;
    Ok(flags)
}

fn parse_fba_violations_flags(args: &[String]) -> Result<ManualArchFlags> {
    let mut module = None;
    let mut passthrough = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--module" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--module requires a value");
                };
                module = Some(value.clone());
            }
            other => passthrough.push(other.to_string()),
        }
        index += 1;
    }
    let mut flags = parse_arch_flags(&passthrough, false)?;
    flags.module = module;
    Ok(flags)
}

fn parse_fba_dependencies_flags(args: &[String]) -> Result<ManualArchFlags> {
    parse_fba_violations_flags(args)
}

fn print_explanation(explanation: &EntityExplanation) -> Result<()> {
    println!("{}", explanation.entity.stable_key.0);
    println!("snapshot: {}", explanation.snapshot);
    println!("kind: {}", serialized_name(&explanation.entity.kind)?);
    println!("name: {}", explanation.entity.name);
    if let Some(source) = &explanation.entity.source {
        let line = source
            .line_start
            .map_or_else(String::new, |line| format!(":{line}"));
        println!("source: {}{line}", source.path);
    }
    for fact in &explanation.facts {
        println!(
            "fact: {} (confidence {:.2})",
            serialized_name(&fact.kind)?,
            fact.confidence
        );
    }
    for explained in &explanation.outgoing_relations {
        let target = explained
            .related_entity
            .as_ref()
            .map_or(explained.relation.to.0.as_str(), |entity| {
                entity.stable_key.0.as_str()
            });
        println!(
            "relation: --{}--> {} [{}]",
            serialized_name(&explained.relation.kind)?,
            target,
            serialized_name(&explained.relation.status)?
        );
    }
    for explained in &explanation.incoming_relations {
        let source = explained
            .related_entity
            .as_ref()
            .map_or(explained.relation.from.0.as_str(), |entity| {
                entity.stable_key.0.as_str()
            });
        println!(
            "relation: {} --{}--> [this] [{}]",
            source,
            serialized_name(&explained.relation.kind)?,
            serialized_name(&explained.relation.status)?
        );
    }
    for diagnostic in &explanation.diagnostics {
        println!(
            "diagnostic: {} {} — {}",
            serialized_name(&diagnostic.severity)?,
            serialized_name(&diagnostic.kind)?,
            diagnostic.title
        );
    }
    Ok(())
}

fn print_overview(overview: &RepositoryOverview) -> Result<()> {
    println!("Athanor overview (snapshot: {})", overview.snapshot);
    println!(
        "canonical objects: {} entities, {} facts, {} relations, {} diagnostics ({} open)",
        overview.totals.entities,
        overview.totals.facts,
        overview.totals.relations,
        overview.totals.diagnostics,
        overview.totals.open_diagnostics
    );
    println!("source files: {}", overview.totals.source_files);
    println!();

    println!(
        "API: {} endpoints, {} schemas, {} examples, {} documented, {} implemented",
        overview.api.endpoints,
        overview.api.schemas,
        overview.api.examples,
        overview.api.documented_endpoints,
        overview.api.implemented_endpoints
    );
    println!(
        "Docs: {} pages, {} sections, {} runbooks, {} operation steps, {} operations pages",
        overview.docs.pages,
        overview.docs.sections,
        overview.docs.runbooks,
        overview.docs.operation_steps,
        overview.docs.operations_pages
    );
    println!(
        "Operations: {} env vars, {} script commands, {} deployment resources, {} migrations, {} packages, {} dependencies",
        overview.operations.environment_variables,
        overview.operations.script_commands,
        overview.operations.deployment_resources,
        overview.operations.database_migrations,
        overview.operations.packages,
        overview.operations.dependencies
    );
    println!();

    print_named_counts("Top entity kinds", &overview.entity_kinds);
    print_named_counts("Top relation kinds", &overview.relation_kinds);
    print_named_counts("Top source roots", &overview.source_roots);

    println!("Module structure:");
    if overview.module_structure.is_empty() {
        println!("  (none)");
    } else {
        for module in &overview.module_structure {
            let source = module.source.as_deref().unwrap_or("unknown source");
            println!(
                "  - {} ({}) members={} source={}",
                module.name, module.stable_key, module.direct_members, source
            );
        }
    }

    println!("Integration boundaries:");
    if overview.integration_boundaries.is_empty() {
        println!("  (none)");
    } else {
        for boundary in &overview.integration_boundaries {
            println!(
                "  - {} -> {}: {} relations",
                boundary.from_root, boundary.to_root, boundary.relations
            );
        }
    }

    println!("Graph hubs:");
    if overview.graph_hubs.is_empty() {
        println!("  (none)");
    } else {
        for hub in &overview.graph_hubs {
            let source = hub.source.as_deref().unwrap_or("unknown source");
            println!(
                "  - [{}] {} ({}) degree={} source={}",
                hub.kind, hub.name, hub.stable_key, hub.degree, source
            );
        }
    }

    println!("Open diagnostics:");
    if overview.open_diagnostics.is_empty() {
        println!("  (none)");
    } else {
        for diagnostic in &overview.open_diagnostics {
            let source = diagnostic.source.as_deref().unwrap_or("unknown source");
            println!(
                "  - [{}] {} at {} - {}",
                diagnostic.severity, diagnostic.kind, source, diagnostic.title
            );
        }
    }

    Ok(())
}

fn print_related_graph(related: &GraphRelated) {
    println!(
        "Related graph for {} (snapshot: {})",
        related.root.entity.stable_key, related.snapshot
    );
    println!(
        "{} entities, {} relations{}",
        related.nodes.len(),
        related.edges.len(),
        if related.truncated {
            " (truncated)"
        } else {
            ""
        }
    );
    for node in &related.nodes {
        let source = node.entity.source.as_deref().unwrap_or("unknown source");
        println!(
            "  - distance={} [{}] {} ({}) source={}",
            node.distance, node.entity.kind, node.entity.name, node.entity.stable_key, source
        );
    }
    if !related.edges.is_empty() {
        println!("Relations:");
        for edge in &related.edges {
            println!(
                "  - {} [{}] {} -> {}",
                edge.id, edge.kind, edge.from, edge.to
            );
        }
    }
}

fn print_graph_path(path: &GraphPath) {
    println!(
        "Graph path from {} to {} (snapshot: {})",
        path.from.stable_key, path.to.stable_key, path.snapshot
    );
    if !path.found {
        println!(
            "No path found after visiting {} entities{}",
            path.visited,
            if path.truncated {
                " (search truncated)"
            } else {
                ""
            }
        );
        return;
    }

    println!(
        "{} hops after visiting {} entities{}",
        path.hops.unwrap_or_default(),
        path.visited,
        if path.truncated {
            " (search truncated)"
        } else {
            ""
        }
    );
    for (index, node) in path.nodes.iter().enumerate() {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!(
            "  {index}. [{}] {} ({}) source={}",
            node.kind, node.name, node.stable_key, source
        );
        if let Some(edge) = path.edges.get(index) {
            println!(
                "     via {} [{}] {} -> {}",
                edge.id, edge.kind, edge.from, edge.to
            );
        }
    }
}

fn print_graph_hubs(report: &GraphHubs) {
    let kind = report.kind.as_deref().unwrap_or("all kinds");
    println!(
        "Graph hubs for {kind} (snapshot: {}, omitted: {})",
        report.snapshot, report.omitted
    );
    if report.hubs.is_empty() {
        println!("  (none)");
        return;
    }
    for (index, hub) in report.hubs.iter().enumerate() {
        let source = hub.entity.source.as_deref().unwrap_or("unknown source");
        println!(
            "  {}. [{}] {} ({}) degree={} incoming={} outgoing={} source={}",
            index + 1,
            hub.entity.kind,
            hub.entity.name,
            hub.entity.stable_key,
            hub.entity.degree,
            hub.incoming_degree,
            hub.outgoing_degree,
            source
        );
    }
}

fn print_graph_pagerank(report: &GraphPageRank) {
    let kind = report.kind.as_deref().unwrap_or("all kinds");
    println!(
        "Graph PageRank for {kind} (snapshot: {}, entities: {}, relations: {}, iterations: {}, converged: {}, omitted: {})",
        report.snapshot,
        report.entity_count,
        report.relation_count,
        report.iterations,
        report.converged,
        report.omitted
    );
    if report.ranks.is_empty() {
        println!("  (none)");
        return;
    }
    for entry in &report.ranks {
        let source = entry.entity.source.as_deref().unwrap_or("unknown source");
        println!(
            "  {}. [{:.8}] [{}] {} ({}) source={}",
            entry.rank,
            entry.score,
            entry.entity.kind,
            entry.entity.name,
            entry.entity.stable_key,
            source
        );
    }
}

fn print_graph_cycles(report: &GraphCycles) {
    println!(
        "Directed graph cycles (snapshot: {}, starts: {}, omitted starts: {})",
        report.snapshot, report.start_entities, report.omitted_start_entities
    );
    if report.cycles.is_empty() {
        println!(
            "  (none{})",
            if report.truncated {
                "; search truncated"
            } else {
                ""
            }
        );
        return;
    }
    for (index, cycle) in report.cycles.iter().enumerate() {
        let stable_keys = cycle
            .nodes
            .iter()
            .map(|node| node.stable_key.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        println!(
            "  {}. length={} {} -> {}",
            index + 1,
            cycle.length,
            stable_keys,
            cycle.nodes[0].stable_key
        );
        println!(
            "     relations: {}",
            cycle
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if report.truncated {
        println!("  search truncated by configured limits");
    }
}

fn print_project_registry(report: &ProjectRegistryReport) {
    println!(
        "Registered projects at {}: {}",
        report.registry_path.display(),
        report.projects.len()
    );
    if report.projects.is_empty() {
        println!("  (none)");
        return;
    }
    for project in &report.projects {
        print_project_registration(project);
    }
}

fn print_project_registration(project: &ProjectRegistration) {
    println!("  {} -> {}", project.project_id, project.root.display());
}

fn print_adapter_trust_report(report: &AdapterTrustReport) {
    println!(
        "Adapter plugin trust at {}: {}",
        report.trust_path.display(),
        report.plugins.len()
    );
    if report.plugins.is_empty() {
        println!("  (none)");
        return;
    }
    for plugin in &report.plugins {
        let trust = if plugin.trusted {
            "trusted"
        } else {
            "untrusted"
        };
        let external = if plugin.has_external_process {
            "external-process"
        } else {
            "in-process"
        };
        println!(
            "  {} [{}; {}] -> {}",
            plugin.name,
            trust,
            external,
            plugin.manifest_path.display()
        );
        println!("    hash: {}", plugin.content_hash);
    }
}

fn print_named_counts(title: &str, counts: &[athanor_app::NamedCount]) {
    println!("{title}:");
    if counts.is_empty() {
        println!("  (none)");
    } else {
        for item in counts {
            println!("  - {}: {}", item.name, item.count);
        }
    }
}

fn print_index_report(report: &IndexReport, action: &str) -> Result<()> {
    if report.validate_only {
        println!(
            "validated {} files against adapter contracts using snapshot {}",
            report.files_indexed, report.snapshot
        );
        if let Some(validation_result) = &report.validation_result {
            println!("wrote validation result to {}", validation_result.display());
        }
    } else {
        println!(
            "{action} {} files into snapshot {}",
            report.files_indexed, report.snapshot
        );
    }
    println!(
        "affected files: {} changed, {} unchanged, {} removed",
        report.changed_files, report.unchanged_files, report.removed_files
    );
    if !report.validate_only {
        println!("wrote JSONL to {}", report.output_dir.display());
    }
    println!(
        "metrics: total {} ms, pipeline {} ms",
        report.metrics.total_ms, report.metrics.pipeline.total_ms
    );
    Ok(())
}

fn print_benchmark_report(report: &BenchmarkReport) {
    let pipeline = &report.index.metrics.pipeline;
    println!(
        "benchmark {}: {} files, total {} ms, index {} ms",
        report.size.as_str(),
        report.files_written,
        report.total_ms,
        report.index.metrics.total_ms
    );
    println!(
        "pipeline: discovery {} ms, extraction {} ms, linking {} ms, checking {} ms, storage {} ms",
        pipeline.source_discovery_ms,
        pipeline.extraction_ms,
        pipeline.linking_ms,
        pipeline.checking_ms,
        pipeline.storage_ms
    );
    println!("snapshot: {}", report.index.snapshot);
    if report.kept_fixture {
        println!("fixture: {}", report.fixture_root.display());
    }
}

fn print_repair_inspect_report(report: &RepairInspectReport) {
    println!("repair inspection: {:?}", report.status);
    println!(
        "canonical snapshots: {} total, latest {}",
        report.canonical.snapshot_count,
        report
            .canonical
            .latest_snapshot
            .as_deref()
            .unwrap_or("(none)")
    );
    println!(
        "generated generations: {} total, current {}",
        report.generated.generation_count,
        report
            .generated
            .current_generation
            .as_deref()
            .unwrap_or("(none)")
    );
    println!(
        "orphans: {} canonical snapshots, {} generated generations",
        report.canonical.orphan_snapshots.len(),
        report.generated.orphan_generations.len()
    );
    if report.issues.is_empty() {
        println!("issues: none");
    } else {
        println!("issues:");
        for issue in &report.issues {
            println!(
                "  - {} at {}: {}",
                issue.code,
                issue.path.display(),
                issue.message
            );
        }
    }
}

fn print_repair_cleanup_report(report: &RepairCleanupReport) {
    println!(
        "repair cleanup{}: {} artifact(s), {} retained",
        if report.dry_run { " dry run" } else { "" },
        report.removed.len(),
        report.retained.len()
    );
    for removal in &report.removed {
        println!(
            "  - {:?} {} at {}",
            removal.kind,
            removal.id,
            removal.path.display()
        );
    }
    if report.remaining_issues.is_empty() {
        println!("remaining issues: none");
    } else {
        println!("remaining issues:");
        for issue in &report.remaining_issues {
            println!(
                "  - {} at {}: {}",
                issue.code,
                issue.path.display(),
                issue.message
            );
        }
    }
}

fn print_repair_regenerate_report(report: &RepairRegenerateReport) {
    println!(
        "repair regenerate{}: {}",
        if report.dry_run { " dry run" } else { "" },
        if report.needed {
            "needed"
        } else {
            "not needed"
        }
    );
    if let Some(generated) = &report.generated {
        println!(
            "published generation {} for snapshot {}",
            generated.generation, generated.snapshot
        );
        println!("generation dir: {}", generated.generation_dir.display());
        println!("current pointer: {}", generated.current_pointer.display());
    }
    if report.remaining_issues.is_empty() {
        println!("remaining issues: none");
    } else {
        println!("remaining issues:");
        for issue in &report.remaining_issues {
            println!(
                "  - {} at {}: {}",
                issue.code,
                issue.path.display(),
                issue.message
            );
        }
    }
}

fn print_repair_recover_canonical_report(report: &RepairRecoverCanonicalReport) {
    println!(
        "repair recover-canonical{}: {}",
        if report.dry_run { " dry run" } else { "" },
        if report.needed {
            "needed"
        } else {
            "not needed"
        }
    );
    if let Some(snapshot) = &report.selected_snapshot {
        println!("selected snapshot: {snapshot}");
    }
    if let Some(snapshot) = &report.recovered_snapshot {
        println!("recovered latest pointer: {snapshot}");
    }
    if report.remaining_issues.is_empty() {
        println!("remaining issues: none");
    } else {
        println!("remaining issues:");
        for issue in &report.remaining_issues {
            println!(
                "  - {} at {}: {}",
                issue.code,
                issue.path.display(),
                issue.message
            );
        }
    }
}

fn print_repair_apply_report(report: &RepairApplyReport) {
    println!(
        "repair apply{}:",
        if report.dry_run { " dry run" } else { "" }
    );
    println!(
        "  canonical recovery: {}",
        if report.canonical.needed {
            "needed"
        } else {
            "not needed"
        }
    );
    println!(
        "  generated regeneration: {}",
        if report.generated.needed {
            "needed"
        } else {
            "not needed"
        }
    );
    println!(
        "  cleanup artifacts: {} planned/removed, {} retained",
        report.cleanup.removed.len(),
        report.cleanup.retained.len()
    );
    if report.remaining_issues.is_empty() {
        println!("remaining issues: none");
    } else {
        println!("remaining issues:");
        for issue in &report.remaining_issues {
            println!(
                "  - {} at {}: {}",
                issue.code,
                issue.path.display(),
                issue.message
            );
        }
    }
}

fn print_rustok_ffa_audit(report: &RustokFfaAudit) {
    let completion = report
        .summary
        .completion_percent
        .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
    println!(
        "RusTok FFA audit (snapshot: {}, observed: {}, actionable: {}, complete: {}, incomplete: {}, structural completion: {}, missing core/transport/ui: {}/{}/{}, scaffolds: {}, host wiring: {}, diagnostics: {})",
        report.snapshot,
        report.summary.observed_surfaces,
        report.summary.surfaces_total,
        report.summary.core_transport_ui,
        report.summary.incomplete,
        completion,
        report.summary.missing_core,
        report.summary.missing_transport,
        report.summary.missing_ui_adapter,
        report.summary.scaffold_surfaces,
        report.summary.host_wiring_surfaces,
        report.summary.diagnostics_open
    );
    if report.surfaces.is_empty() {
        println!("  (none)");
        return;
    }
    for surface in &report.surfaces {
        let diagnostics = if surface.diagnostics.is_empty() {
            "none".to_string()
        } else {
            surface.diagnostics.join(", ")
        };
        let completion = surface
            .completion_percent
            .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
        println!(
            "  - {} shape={} structural_completion={} requirements={}/{} layers={} files={} diagnostics={}",
            surface.id,
            surface.shape,
            completion,
            surface.requirements_met,
            surface.requirements_total,
            surface.layers.join(","),
            surface.files.len(),
            diagnostics
        );
    }
}

fn print_rustok_architecture_context(report: &RustokArchitectureContext) {
    println!(
        "RusTok architecture context (snapshot: {}, resolution: {}, primary module: {})",
        report.snapshot,
        report.resolution.status,
        report
            .resolution
            .primary_module
            .as_deref()
            .unwrap_or("unresolved")
    );
    println!("  {}", report.resolution.summary);
    if !report.modules.is_empty() {
        println!("  modules:");
        for module in &report.modules {
            println!(
                "    - {} score={} reasons={}",
                module.slug,
                module.score,
                module.reasons.join(", ")
            );
        }
    }
    if !report.contracts.is_empty() {
        println!("  public contracts:");
        for contract in &report.contracts {
            println!("    - {}", contract.stable_key);
        }
    }
    if !report.interactions.is_empty() {
        println!("  interactions:");
        for interaction in &report.interactions {
            println!(
                "    - {} -> {} profile={}",
                interaction.consumer, interaction.provider, interaction.profile
            );
        }
    }
    println!("  guidance:");
    for guidance in &report.guidance {
        println!("    - {guidance}");
    }
}

fn print_rustok_ffa_graph(report: &RustokFfaGraph) {
    let surface = report.surface.as_deref().unwrap_or("violations");
    println!(
        "RusTok FFA graph for {} (snapshot: {}, nodes: {}, edges: {}, diagnostics: {}, omitted nodes: {}, omitted edges: {})",
        surface,
        report.snapshot,
        report.nodes.len(),
        report.edges.len(),
        report.diagnostics.len(),
        report.omitted.nodes,
        report.omitted.edges
    );
    for diagnostic in &report.diagnostics {
        println!("  ! {:?} {}", diagnostic.severity, diagnostic.title);
    }
    for node in &report.nodes {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!("  - [{}] {} source={}", node.kind, node.id, source);
    }
    for edge in &report.edges {
        println!("    {} -> {} [{}]", edge.from, edge.to, edge.kind);
    }
}

fn print_rustok_fba_audit(report: &RustokFbaAudit) {
    let completion = report
        .summary
        .completion_percent
        .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
    println!(
        "RusTok FBA audit (snapshot: {}, modules: {}, registered: {}, dependency-only: {}, in progress: {}, status unknown: {}, contract completion: {} ({}/{}), providers: {}, consumers: {}, ports: {}, operations: {}, dependency edges: {}/{}, diagnostics: {})",
        report.snapshot,
        report.summary.modules_total,
        report.summary.registered_modules,
        report.summary.dependency_only_modules,
        report.summary.in_progress_modules,
        report.summary.status_unknown_modules,
        completion,
        report.summary.requirements_met,
        report.summary.requirements_total,
        report.summary.provider_modules,
        report.summary.consumer_modules,
        report.summary.ports_total,
        report.summary.operations_total,
        report.summary.dependency_edges_resolved,
        report.summary.dependency_edges_total,
        report.summary.diagnostics_open
    );
    if report.modules.is_empty() {
        println!("  (none)");
        return;
    }
    for module in &report.modules {
        let diagnostics = if module.diagnostics.is_empty() {
            "none".to_string()
        } else {
            module.diagnostics.join(", ")
        };
        let completion = module
            .completion_percent
            .map_or_else(|| "n/a".to_string(), |percent| format!("{percent}%"));
        println!(
            "  - {} role={} status={} contract={} contract_completion={} requirements={}/{} ports={} operations={} dependencies={} diagnostics={}",
            module.id,
            module.role.as_deref().unwrap_or("unknown"),
            module.status.as_deref().unwrap_or("unknown"),
            module.contract_version.as_deref().unwrap_or("none"),
            completion,
            module.requirements_met,
            module.requirements_total,
            module.ports.len(),
            module.operations.len(),
            module.dependencies.len(),
            diagnostics
        );
    }
}

fn print_rustok_fba_graph(report: &RustokFbaGraph) {
    let root = report.root.as_deref().unwrap_or("violations");
    println!(
        "RusTok FBA graph for {} (snapshot: {}, nodes: {}, edges: {}, diagnostics: {}, omitted nodes: {}, omitted edges: {})",
        root,
        report.snapshot,
        report.nodes.len(),
        report.edges.len(),
        report.diagnostics.len(),
        report.omitted.nodes,
        report.omitted.edges
    );
    for diagnostic in &report.diagnostics {
        println!("  ! {:?} {}", diagnostic.severity, diagnostic.title);
    }
    for node in &report.nodes {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!("  - [{}] {} source={}", node.kind, node.id, source);
    }
    for edge in &report.edges {
        println!("    {} -> {} [{}]", edge.from, edge.to, edge.kind);
    }
}

fn print_rustok_page_builder_audit(report: &RustokPageBuilderAudit) {
    println!(
        "RusTok Page Builder audit (snapshot: {}, providers: {}, consumers: {}, contracts: {}, capabilities: {}, fallback profiles: {}, wave evidence: {}, diagnostics: {})",
        report.snapshot,
        report.summary.providers_total,
        report.summary.consumers_total,
        report.summary.contracts_total,
        report.summary.capabilities_total,
        report.summary.fallback_profiles_total,
        report.summary.wave_evidence_total,
        report.summary.diagnostics_open
    );
    if report.consumers.is_empty() {
        println!("  consumers: (none)");
    } else {
        println!("  consumers:");
        for consumer in &report.consumers {
            let diagnostics = if consumer.diagnostics.is_empty() {
                "none".to_string()
            } else {
                consumer.diagnostics.join(", ")
            };
            println!(
                "  - {} module={} diagnostics={}",
                consumer.id, consumer.module, diagnostics
            );
        }
    }
    if !report.diagnostics.is_empty() {
        println!("  diagnostics: {}", report.diagnostics.join(", "));
    }
}

fn print_rustok_page_builder_graph(report: &RustokPageBuilderGraph) {
    let root = report.root.as_deref().unwrap_or("violations");
    println!(
        "RusTok Page Builder graph for {} (snapshot: {}, nodes: {}, edges: {}, diagnostics: {}, omitted nodes: {}, omitted edges: {})",
        root,
        report.snapshot,
        report.nodes.len(),
        report.edges.len(),
        report.diagnostics.len(),
        report.omitted.nodes,
        report.omitted.edges
    );
    for diagnostic in &report.diagnostics {
        println!("  ! {:?} {}", diagnostic.severity, diagnostic.title);
    }
    for node in &report.nodes {
        let source = node.source.as_deref().unwrap_or("unknown source");
        println!("  - [{}] {} source={}", node.kind, node.id, source);
    }
    for edge in &report.edges {
        println!("    {} -> {} [{}]", edge.from, edge.to, edge.kind);
    }
}

fn print_check_report(report: &DiagnosticCheckReport) -> Result<()> {
    println!(
        "{} diagnostics in {}: {} open ({} critical, {} high, {} medium, {} low)",
        serialized_name(&report.scope)?,
        report.snapshot,
        report.counts.total,
        report.counts.critical,
        report.counts.high,
        report.counts.medium,
        report.counts.low
    );
    for diagnostic in &report.diagnostics {
        let location = diagnostic
            .evidence
            .iter()
            .find_map(|evidence| {
                evidence.source_file.as_ref().map(|path| {
                    evidence
                        .line_start
                        .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
                })
            })
            .or_else(|| {
                diagnostic
                    .ownership
                    .first()
                    .map(|ownership| ownership.source_file.clone())
            })
            .unwrap_or_else(|| "unknown source".to_string());
        println!(
            "{} {} at {} — {}",
            serialized_name(&diagnostic.severity)?,
            serialized_name(&diagnostic.kind)?,
            location,
            diagnostic.title
        );
    }
    Ok(())
}

fn print_coverage_report(report: &CoverageReport) {
    println!(
        "coverage for {}: {} tracked files, {} files with canonical objects, {} open diagnostics",
        report.snapshot,
        report.totals.tracked_files,
        report.totals.files_with_canonical_objects,
        report.totals.open_diagnostics
    );
    println!(
        "canonical objects: {} entities, {} facts, {} relations, {} diagnostics",
        report.totals.entities,
        report.totals.facts,
        report.totals.relations,
        report.totals.diagnostics
    );
    if let Some(adapter) = &report.filters.adapter {
        println!("adapter filter: {adapter}");
    }
    if let Some(file) = &report.filters.file {
        println!("file filter: {file}");
    }
    println!("adapters:");
    if report.adapters.is_empty() {
        println!("  (none)");
    } else {
        for adapter in &report.adapters {
            println!(
                "  - {}: {} files, {} facts, {} evidence items, {} diagnostics",
                adapter.adapter,
                adapter.files,
                adapter.facts,
                adapter.evidence_items,
                adapter.diagnostics
            );
        }
    }
    println!("files:");
    if report.files.is_empty() {
        println!("  (none)");
    } else {
        for file in &report.files {
            println!(
                "  - {}: {} entities, {} facts, {} relations, {} diagnostics ({} open)",
                file.path,
                file.entities,
                file.facts,
                file.relations,
                file.diagnostics,
                file.open_diagnostics
            );
        }
    }
    println!("diagnostics:");
    if report.diagnostics.is_empty() {
        println!("  (none)");
    } else {
        for diagnostic in &report.diagnostics {
            println!(
                "  - {}: {} total, {} open, {} files",
                diagnostic.kind, diagnostic.total, diagnostic.open, diagnostic.files
            );
        }
    }
    if report.omitted.files > 0 || report.omitted.adapters > 0 || report.omitted.diagnostics > 0 {
        println!(
            "omitted: {} files, {} adapters, {} diagnostic kinds (limit {})",
            report.omitted.files,
            report.omitted.adapters,
            report.omitted.diagnostics,
            report.limits.limit
        );
    }
}

fn print_capabilities_report(report: &CapabilitiesReport) {
    println!(
        "capabilities for {}: {} tracked files, {} content-processed ({}%), {} unprocessed",
        report.snapshot,
        report.totals.tracked_files,
        report.totals.processed_files,
        report.totals.processed_ratio_percent,
        report.totals.unprocessed_files
    );
    println!(
        "processed means canonical objects from an adapter other than the `{}` baseline inventory adapter",
        report.baseline_adapter
    );
    println!(
        "facts: {} total, {} below confidence {} across {} adapters",
        report.totals.facts,
        report.totals.low_confidence_facts,
        report.limits.confidence_threshold,
        report.totals.adapters
    );
    println!("languages:");
    if report.languages.is_empty() {
        println!("  (none)");
    } else {
        for language in &report.languages {
            println!(
                "  - {}: {} tracked, {} processed ({}%), {} unprocessed",
                language.language_hint,
                language.tracked_files,
                language.processed_files,
                language.processed_ratio_percent,
                language.unprocessed_files
            );
        }
    }
    println!("adapters:");
    if report.adapters.is_empty() {
        println!("  (none)");
    } else {
        for adapter in &report.adapters {
            println!(
                "  - {}: {} files, {} facts, {} low confidence, min confidence {}",
                adapter.adapter,
                adapter.processed_files,
                adapter.facts,
                adapter.low_confidence_facts,
                adapter.min_confidence
            );
        }
    }
    println!("unprocessed files:");
    if report.unprocessed_files.is_empty() {
        println!("  (none)");
    } else {
        for file in &report.unprocessed_files {
            println!("  - {} [{}]", file.path, file.language_hint);
        }
    }
    println!("low confidence facts:");
    if report.low_confidence_facts.is_empty() {
        println!("  (none)");
    } else {
        for fact in &report.low_confidence_facts {
            let location = fact.path.as_deref().map_or_else(
                || "(no evidence path)".to_string(),
                |path| match fact.line_start {
                    Some(line) => format!("{path}:{line}"),
                    None => path.to_string(),
                },
            );
            println!(
                "  - {} ({}) confidence {} [{}] {}",
                fact.kind, fact.adapter, fact.confidence, location, fact.fact_id
            );
        }
    }
    if report.omitted.languages > 0
        || report.omitted.adapters > 0
        || report.omitted.unprocessed_files > 0
        || report.omitted.low_confidence_facts > 0
    {
        println!(
            "omitted: {} languages, {} adapters, {} unprocessed files, {} low confidence facts (limit {})",
            report.omitted.languages,
            report.omitted.adapters,
            report.omitted.unprocessed_files,
            report.omitted.low_confidence_facts,
            report.limits.limit
        );
    }
}

fn print_affected_check_report(report: &AffectedCheckReport) -> Result<()> {
    println!(
        "affected diagnostics in {}: {} open ({} critical, {} high, {} medium, {} low)",
        report.snapshot,
        report.counts.total,
        report.counts.critical,
        report.counts.high,
        report.counts.medium,
        report.counts.low
    );
    println!(
        "affected files: {} changed, {} unchanged, {} removed",
        report.affected_files.changed,
        report.affected_files.unchanged,
        report.affected_files.removed
    );
    if !report.stale_artifacts.is_empty() {
        println!("stale artifacts: {}", report.stale_artifacts.len());
        for artifact in &report.stale_artifacts {
            println!(
                "{} at {}: {} (run `{}`)",
                serialized_name(&artifact.kind)?,
                artifact.path.display(),
                artifact.message,
                artifact.suggested_command
            );
        }
    }
    if !report.documentation_drift.is_empty() {
        println!(
            "affected documentation drift: {}",
            report.documentation_drift.len()
        );
        for document in &report.documentation_drift {
            println!(
                "{}: {} (verified: {})",
                document.path,
                document.reason,
                document.verified_snapshot.as_deref().unwrap_or("missing")
            );
        }
    }
    for diagnostic in &report.diagnostics {
        let location = diagnostic
            .evidence
            .iter()
            .find_map(|evidence| {
                evidence.source_file.as_ref().map(|path| {
                    evidence
                        .line_start
                        .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
                })
            })
            .or_else(|| {
                diagnostic
                    .ownership
                    .first()
                    .map(|ownership| ownership.source_file.clone())
            })
            .unwrap_or_else(|| "unknown source".to_string());
        println!(
            "{} {} at {} вЂ” {}",
            serialized_name(&diagnostic.severity)?,
            serialized_name(&diagnostic.kind)?,
            location,
            diagnostic.title
        );
    }
    Ok(())
}

fn print_docs_check_report(report: &DocsCheckReport) -> Result<()> {
    println!(
        "documentation completeness in {}: {} ({} editable documents, {} policy violations, {} diagnostics)",
        report.snapshot,
        if report.passed { "passed" } else { "failed" },
        report.editable_documents,
        report.policy_violations.len(),
        report.diagnostics.len()
    );
    for violation in &report.policy_violations {
        println!(
            "policy {} at {} — {}",
            violation.field, violation.path, violation.message
        );
    }
    for diagnostic in &report.diagnostics {
        println!(
            "diagnostic: {} — {}",
            serialized_name(&diagnostic.kind)?,
            diagnostic.title
        );
    }
    Ok(())
}

fn print_docs_drift_report(report: &DocsDriftReport) {
    println!(
        "documentation drift in {}: {} of {} editable documents require verification",
        report.snapshot,
        report.drifted_documents.len(),
        report.editable_documents
    );
    for document in &report.drifted_documents {
        println!(
            "{} at {} (last verified: {})",
            document.reason,
            document.path,
            document.verified_snapshot.as_deref().unwrap_or("never")
        );
    }
}

fn print_docs_propose_fix_report(report: &DocsProposeFixReport) {
    let changes = report
        .proposal
        .operations
        .iter()
        .map(|operation| operation.changes.len())
        .sum::<usize>();
    println!(
        "created docs patch {} for snapshot {}: {} files, {} frontmatter changes",
        report.proposal.id,
        report.proposal.snapshot,
        report.proposal.operations.len(),
        changes
    );
    println!("wrote patch proposal to {}", report.path.display());
    for operation in &report.proposal.operations {
        println!("{} ({})", operation.path, operation.stable_key);
        if operation.create {
            println!("  create file");
        }
        for change in &operation.changes {
            println!("  set {} = {}", change.field, change.new_value);
        }
    }
}

fn print_docs_apply_patch_report(report: &DocsApplyPatchReport) {
    println!(
        "applied docs patch {} for snapshot {}: {} files changed, {} frontmatter changes",
        report.id, report.snapshot, report.files_changed, report.changes_applied
    );
}

fn print_operations_docs_check_report(report: &OperationsDocsCheckReport) -> Result<()> {
    println!(
        "operational documentation in {}: {} open ({} critical, {} high, {} medium, {} low)",
        report.snapshot,
        report.counts.total,
        report.counts.critical,
        report.counts.high,
        report.counts.medium,
        report.counts.low
    );
    for scoped in [
        &report.env,
        &report.scripts,
        &report.deployment,
        &report.runbooks,
    ] {
        println!();
        print_check_report(scoped)?;
    }
    Ok(())
}

fn print_api_snapshot_report(report: &ApiSnapshotReport) {
    println!(
        "{} API contract snapshot {} ({} endpoints, {} schemas, {} examples)",
        if report.created { "created" } else { "reused" },
        report.snapshot,
        report.endpoints,
        report.schemas,
        report.examples
    );
    println!("wrote API contract to {}", report.path.display());
    if let Some(cleanup) = &report.cleanup {
        print_api_cleanup_summary(cleanup);
    }
}

fn print_api_contract_diff(diff: &ApiContractDiff) -> Result<()> {
    println!(
        "API contract {} -> {}: {} changes, {} breaking",
        diff.from,
        diff.to,
        diff.changes.len(),
        diff.breaking_changes
    );
    for change in &diff.changes {
        println!(
            "{} {}{}",
            serialized_name(&change.kind)?,
            change.stable_key,
            if change.breaking { " [breaking]" } else { "" }
        );
        for reason in &change.reasons {
            println!("  reason: {reason}");
        }
    }
    if let Some(cleanup) = &diff.cleanup {
        print_api_cleanup_summary(cleanup);
    }
    Ok(())
}

fn print_api_cleanup_report(report: &ApiCleanupReport) {
    print_api_cleanup_summary(report);
    for artifact in &report.removed {
        println!(
            "  remove {:?} {} at {}",
            artifact.kind,
            artifact.id,
            artifact.path.display()
        );
    }
}

fn print_api_cleanup_summary(report: &ApiCleanupReport) {
    println!(
        "API cleanup: {} removed, {} retained{}",
        report.removed.len(),
        report.retained.len(),
        if report.dry_run { " (dry run)" } else { "" }
    );
}

fn serialized_name(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .map_or_else(|| "unknown".to_string(), str::to_string))
}

fn print_impact_analysis(analysis: &ImpactAnalysis) -> Result<()> {
    println!("Code Impact Analysis (snapshot: {})", analysis.snapshot);
    println!("Starting Entities:");
    if analysis.starting_entities.is_empty() {
        println!("  (none)");
    } else {
        for entity in &analysis.starting_entities {
            println!(
                "  - [{}] {} ({})",
                serialized_name(&entity.kind)?,
                entity.name,
                entity.stable_key.0
            );
        }
    }
    println!();

    println!("Impacted Files ({}):", analysis.impacted_files.len());
    for file in &analysis.impacted_files {
        println!("  - {file}");
    }
    println!();

    println!("Impacted Entities ({}):", analysis.impacted_entities.len());
    if analysis.impacted_entities.is_empty() {
        println!("  (none)");
    } else {
        let mut max_depth = 0;
        for item in &analysis.impacted_entities {
            if item.depth > max_depth {
                max_depth = item.depth;
            }
        }

        for depth in 1..=max_depth {
            let items_at_depth: Vec<_> = analysis
                .impacted_entities
                .iter()
                .filter(|item| item.depth == depth)
                .collect();

            if items_at_depth.is_empty() {
                continue;
            }

            println!("  Depth {}:", depth);
            for item in items_at_depth {
                let relation_info = if item.path_steps.is_empty() {
                    "direct".to_string()
                } else {
                    item.path_steps
                        .iter()
                        .map(|step| {
                            format!(
                                "{} --{}:{}--> {}",
                                step.from.name, step.relation_kind, step.relation_id, step.to.name
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | ")
                };

                println!(
                    "    - [{}] {} ({})",
                    serialized_name(&item.entity.kind)?,
                    item.entity.name,
                    item.entity.stable_key.0
                );
                println!("      path: {relation_info}");
            }
        }
    }
    println!();

    println!(
        "Impacted Diagnostics ({}):",
        analysis.impacted_diagnostics.len()
    );
    if analysis.impacted_diagnostics.is_empty() {
        println!("  (none)");
    } else {
        for diagnostic in &analysis.impacted_diagnostics {
            println!(
                "  - [{}] {} — {}",
                serialized_name(&diagnostic.severity)?,
                serialized_name(&diagnostic.kind)?,
                diagnostic.title
            );
        }
    }

    Ok(())
}

fn print_change_map(report: &ChangeMapReport) -> Result<()> {
    println!("Change Map (snapshot: {})", report.snapshot);
    println!(
        "Returned: {} entities, {} files, {} diagnostics; omitted: {}/{}/{}",
        report.returned.entities,
        report.returned.files,
        report.returned.diagnostics,
        report.omitted.entities,
        report.omitted.files,
        report.omitted.diagnostics
    );
    println!();
    println!("Files:");
    for file in &report.files {
        println!("  {}. {} [score={}]", file.rank, file.path, file.score);
        println!("     kinds: {}", file.entity_kinds.join(", "));
    }
    if report.files.is_empty() {
        println!("  (none)");
    }
    println!();
    println!("Entities:");
    for item in &report.items {
        println!(
            "  {}. [{}] {} ({}) score={} depth={}",
            item.rank,
            serialized_name(&item.entity.kind)?,
            item.entity.name,
            item.entity.stable_key.0,
            item.score,
            item.depth
        );
        println!("     why: {}", item.reasons.join("; "));
        for step in &item.path {
            println!(
                "     path: {} --{}:{}--> {} (confidence {:.2})",
                step.from.stable_key,
                step.relation_kind,
                step.relation_id,
                step.to.stable_key,
                step.confidence
            );
        }
        println!(
            "     tests: {}{}",
            serialized_name(&item.test_coverage.status)?,
            if item.test_coverage.tests.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.test_coverage.tests.join(", "))
            }
        );
        for annotation in &item.annotations {
            println!(
                "     annotation: {} [{}] {}",
                annotation.source, annotation.schema, annotation.message
            );
        }
    }
    if report.items.is_empty() {
        println!("  (none)");
    }
    println!();
    println!("Completeness: {}", report.completeness.note);
    if report.completeness.candidate_limit_reached {
        println!(
            "Traversal stopped at the internal candidate limit of {}.",
            report.completeness.candidate_limit
        );
    }
    println!(
        "Inspect coverage: {}",
        report.completeness.suggested_command
    );
    Ok(())
}

fn print_api_registry_report(report: &athanor_app::ApiRegistryReport) -> Result<()> {
    println!(
        "API registry (snapshot: {}, {} endpoints):",
        report.snapshot,
        report.endpoints.len()
    );
    if report.endpoints.is_empty() {
        println!("  (none)");
    } else {
        for endpoint in &report.endpoints {
            println!(
                "  - {} {} ({})",
                endpoint.method, endpoint.path, endpoint.stable_key
            );
            if let Some(op_id) = &endpoint.operation_id {
                println!("    operationId: {}", op_id);
            }
            if let Some(summary) = &endpoint.summary {
                println!("    summary: {}", summary);
            }
            if let Some(handler) = &endpoint.handler {
                println!("    handler: {}", handler);
            }
            if !endpoint.documentation.is_empty() {
                println!("    documentation:");
                for doc in &endpoint.documentation {
                    println!("      - {}", doc);
                }
            }
        }
    }
    Ok(())
}
