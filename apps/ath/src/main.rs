use std::path::PathBuf;

use anyhow::Result;
use athanor_app::{
    AffectedCheckOptions, AffectedCheckReport, ApiContractDiff, ApiDiffOptions, ApiSnapshotOptions,
    ApiSnapshotReport, ContextLimitOverrides, ContextOptions, DiagnosticCheckOptions,
    DiagnosticCheckReport, DiagnosticScope, DocsApplyPatchOptions, DocsApplyPatchReport,
    DocsCheckOptions, DocsCheckReport, DocsDriftOptions, DocsDriftReport, DocsProposeFixOptions,
    DocsProposeFixReport, EntityExplanation, ExplainOptions, GenerationOptions, GraphExportOptions,
    GraphPath, GraphPathOptions, GraphRelated, GraphRelatedOptions, HtmlReportOptions,
    ImpactAnalysis, ImpactOptions, IndexOptions, IndexReport, InitOptions,
    OperationsDocsCheckOptions, OperationsDocsCheckReport, OverviewOptions, RepairApplyOptions,
    RepairApplyReport, RepairCleanupOptions, RepairCleanupReport, RepairInspectOptions,
    RepairInspectReport, RepairRecoverCanonicalOptions, RepairRecoverCanonicalReport,
    RepairRegenerateOptions, RepairRegenerateReport, RepositoryOverview, WikiOptions, apply_repair,
    check_affected, check_docs, check_operations_docs, check_project, cleanup_repair,
    context_project, diff_api_contracts, docs_apply_patch, docs_drift, docs_propose_fix,
    explain_project, generate_project, impact_project, index_project, init_project, inspect_repair,
    overview_project, project_html_report, project_wiki, recover_canonical_repair,
    regenerate_repair, snapshot_api_contract,
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
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphExportFormatArg {
    Json,
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
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
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
        }) => {
            let report = index_project(IndexOptions {
                root: path,
                validation_report,
                validation_result,
                validate_only,
            })
            .await?;
            print_index_report(&report, "indexed")?;
        }
        Some(Command::Update {
            path,
            changed,
            json,
        }) => {
            if !changed {
                anyhow::bail!("update requires --changed");
            }
            let report = index_project(IndexOptions {
                root: path,
                validation_report: None,
                validation_result: None,
                validate_only: false,
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_index_report(&report, "updated")?;
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
            let pack = context_project(ContextOptions {
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
            })
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
            let explanation = explain_project(ExplainOptions {
                root: path,
                stable_key,
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&explanation)?);
            } else {
                print_explanation(&explanation)?;
            }
        }
        Some(Command::Overview { path, json, top }) => {
            let overview = overview_project(OverviewOptions { root: path, top }).await?;
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
            let analysis = impact_project(ImpactOptions {
                root: path,
                target,
                diff,
                max_depth,
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            } else {
                print_impact_analysis(&analysis)?;
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
                let report = check_affected(AffectedCheckOptions { root: path }).await?;
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
            let report = check_project(DiagnosticCheckOptions {
                root: path.clone(),
                scope,
            })
            .await?;
            if is_strict {
                if scope != DiagnosticScope::Api {
                    anyhow::bail!("--strict is currently supported only for `ath check api`");
                }
                let diff = diff_api_contracts(ApiDiffOptions {
                    root: path,
                    from: None,
                    to: None,
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
            let report = check_operations_docs(OperationsDocsCheckOptions { root: path }).await?;
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
            command: ApiCommand::Snapshot { path, json },
        }) => {
            let report = snapshot_api_contract(ApiSnapshotOptions { root: path }).await?;
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
                    json,
                },
        }) => {
            let diff = diff_api_contracts(ApiDiffOptions {
                root: path,
                from,
                to,
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
            let report = generate_project(GenerationOptions { root: path }).await?;
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
                json,
            } => {
                let report = cleanup_repair(RepairCleanupOptions {
                    root: path,
                    dry_run,
                    keep_canonical,
                    keep_generated,
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
                json,
            } => {
                let report = apply_repair(RepairApplyOptions {
                    root: path,
                    dry_run,
                    keep_canonical,
                    keep_generated,
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
                    "search results for query \"{}\" in snapshot {}:",
                    query, report.snapshot
                );
                if report.results.is_empty() {
                    println!("No results found.");
                } else {
                    for item in &report.results {
                        println!(
                            "[{:.4}] {} ({}) — {}",
                            item.score, item.name, item.kind, item.stable_key
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
    Ok(())
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
    Ok(())
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
                let relation_info = if let Some(flow) = item.path.last() {
                    let rel_kind = serialized_name(&flow.relation.kind)?;
                    match flow.direction {
                        athanor_app::FlowDirection::Forward => {
                            let prev_name = find_entity_name(analysis, &flow.relation.from);
                            format!("via {} --{}--> [this]", prev_name, rel_kind)
                        }
                        athanor_app::FlowDirection::Backward => {
                            let prev_name = find_entity_name(analysis, &flow.relation.to);
                            format!("via [this] --{}--> {}", rel_kind, prev_name)
                        }
                    }
                } else {
                    "direct".to_string()
                };

                println!(
                    "    - [{}] {} ({}) ({})",
                    serialized_name(&item.entity.kind)?,
                    item.entity.name,
                    item.entity.stable_key.0,
                    relation_info
                );
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

fn find_entity_name(analysis: &ImpactAnalysis, id: &athanor_domain::EntityId) -> String {
    if let Some(entity) = analysis.starting_entities.iter().find(|e| e.id == *id) {
        return entity.name.clone();
    }
    if let Some(item) = analysis
        .impacted_entities
        .iter()
        .find(|i| i.entity.id == *id)
    {
        return item.entity.name.clone();
    }
    id.0.clone()
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
