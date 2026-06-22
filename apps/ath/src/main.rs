use std::path::PathBuf;

use anyhow::Result;
use athanor_app::{
    ContextLimitOverrides, ContextOptions, DiagnosticCheckOptions, DiagnosticCheckReport,
    DiagnosticScope, DocsCheckOptions, DocsCheckReport, EntityExplanation, ExplainOptions,
    GenerationOptions, HtmlReportOptions, IndexOptions, InitOptions, WikiOptions, check_docs,
    check_project, context_project, explain_project, generate_project, index_project, init_project,
    project_html_report, project_wiki,
};
use athanor_domain::ContextLevel;
use clap::{Parser, Subcommand, ValueEnum};

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
    Api,
    Docs,
}

impl From<DiagnosticScopeArg> for DiagnosticScope {
    fn from(value: DiagnosticScopeArg) -> Self {
        match value {
            DiagnosticScopeArg::Api => Self::Api,
            DiagnosticScopeArg::Docs => Self::Docs,
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
    /// Build a task-focused context pack from the latest canonical snapshot.
    Context {
        /// Task or question used to select relevant project knowledge.
        task: String,
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
    },
    /// Check editable documentation against the configured completeness policy.
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
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
}

#[tokio::main]
async fn main() -> Result<()> {
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
                    "indexed {} files into snapshot {}",
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
        }) => {
            let pack = context_project(ContextOptions {
                root: path,
                task,
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
        Some(Command::Check { scope, path, json }) => {
            let report = check_project(DiagnosticCheckOptions {
                root: path,
                scope: scope.into(),
            })
            .await?;
            if json {
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
        None => {
            println!("Athanor {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
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

fn serialized_name(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .map_or_else(|| "unknown".to_string(), str::to_string))
}
