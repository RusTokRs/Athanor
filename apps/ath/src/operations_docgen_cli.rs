use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    CancellationToken, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationOperationsCurrentInspection, DocumentationOperationsManifestInspection,
    DocumentationOperationsOperationOptions, DocumentationOperationsPublicationReport,
    DocumentationOperationsPublicationStatus, DocumentationOperationsValidationInspection,
    DocumentationProfile, OperationsDocsCheckOptions, OperationsDocsCheckReport,
    check_operations_docs_with_composition,
    generate_documentation_operations_with_composition_cancellable,
    inspect_documentation_operations_current, inspect_documentation_operations_manifest,
    inspect_documentation_operations_validation,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

use crate::render::check;

const DEFAULT_MAX_ENTITIES: usize = 512;
const DEFAULT_MAX_FACTS: usize = 1_024;
const DEFAULT_MAX_RELATIONS: usize = 1_024;
const DEFAULT_MAX_DIAGNOSTICS: usize = 128;

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Docs {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Generate deterministic operations documentation from one exact committed snapshot.
    GenerateOperations {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        force: bool,
        #[arg(long, default_value_t = DEFAULT_MAX_ENTITIES)]
        max_entities: usize,
        #[arg(long, default_value_t = DEFAULT_MAX_FACTS)]
        max_facts: usize,
        #[arg(long, default_value_t = DEFAULT_MAX_RELATIONS)]
        max_relations: usize,
        #[arg(long, default_value_t = DEFAULT_MAX_DIAGNOSTICS)]
        max_diagnostics: usize,
        #[arg(long)]
        json: bool,
    },
    /// Check or inspect operations documentation.
    Operations {
        #[command(subcommand)]
        command: OperationsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum OperationsCommand {
    Check {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Current {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Manifest {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Validation {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("docs") {
        return Ok(None);
    }
    let owned = matches!(
        args.get(1).map(String::as_str),
        Some("generate-operations" | "operations")
    );
    if !owned {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match Cli::try_parse_from(argv) {
        Ok(Cli {
            command: RootCommand::Docs { command },
        }) => Ok(Some(command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error
                .print()
                .context("failed to print operations documentation help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();
    match command {
        Command::GenerateOperations {
            path,
            snapshot,
            force,
            max_entities,
            max_facts,
            max_relations,
            max_diagnostics,
            json,
        } => {
            let request = DocumentationGenerationRequest::new(
                snapshot,
                DocumentationProfile::Operations,
                DocumentationGenerationLimits {
                    max_entities,
                    max_facts,
                    max_relations,
                    max_diagnostics,
                },
            );
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            let mut generation = Box::pin(
                generate_documentation_operations_with_composition_cancellable(
                    DocumentationOperationsOperationOptions {
                        root: path,
                        request,
                        force,
                    },
                    &composition,
                    cancellation,
                ),
            );
            let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
            let report = tokio::select! {
                result = &mut generation => result?,
                signal = &mut ctrl_c => {
                    signal.context("failed to listen for operations generation cancellation")?;
                    signal_cancellation.cancel();
                    generation.await?
                }
            };
            render_generation(&report, json)?;
        }
        Command::Operations {
            command: OperationsCommand::Check { path, json },
        } => {
            let report = check_operations_docs_with_composition(
                OperationsDocsCheckOptions { root: path },
                &composition,
            )
            .await?;
            render_operations(&report, json)?;
            if report.counts.total > 0 {
                bail!(
                    "operational documentation check failed with {} open diagnostics",
                    report.counts.total
                );
            }
        }
        Command::Operations {
            command: OperationsCommand::Current { path, json },
        } => render_current(&inspect_documentation_operations_current(path)?, json)?,
        Command::Operations {
            command: OperationsCommand::Manifest { path, json },
        } => render_manifest(&inspect_documentation_operations_manifest(path)?, json)?,
        Command::Operations {
            command: OperationsCommand::Validation { path, json },
        } => render_validation(&inspect_documentation_operations_validation(path)?, json)?,
    }
    Ok(())
}

fn render_generation(report: &DocumentationOperationsPublicationReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    match report.status {
        DocumentationOperationsPublicationStatus::Published => println!(
            "published operations documentation generation {} from snapshot {}",
            report.generation, report.snapshot
        ),
        DocumentationOperationsPublicationStatus::UpToDate => println!(
            "operations documentation generation {} is already up to date for snapshot {}",
            report.generation, report.snapshot
        ),
    }
    println!("document: {}", report.document.display());
    println!("validation: {}", report.validation_report.display());
    println!("manifest: {}", report.manifest.display());
    println!("current: {}", report.current_pointer.display());
    Ok(())
}

fn render_operations(report: &OperationsDocsCheckReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
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
        check::print_diagnostics(scoped)?;
    }
    Ok(())
}

fn render_current(report: &DocumentationOperationsCurrentInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "current operations documentation generation {} for snapshot {}",
            report.current.generation, report.current.snapshot
        );
        println!("profile: operations");
        println!("path: {}", report.current.path);
        println!("manifest: {}", report.current.manifest);
        println!("pointer: {}", report.current_pointer.display());
    }
    Ok(())
}

fn render_manifest(report: &DocumentationOperationsManifestInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "operations documentation manifest {} for snapshot {}: {} artifacts",
        report.manifest.generation,
        report.manifest.snapshot,
        report.manifest.documents.len()
    );
    for artifact in &report.manifest.documents {
        println!(
            "{} {} {} {}",
            artifact.id, artifact.media_type, artifact.path, artifact.sha256
        );
    }
    println!("manifest: {}", report.manifest_path.display());
    Ok(())
}

fn render_validation(
    report: &DocumentationOperationsValidationInspection,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "operations documentation validation for snapshot {}: valid",
        report.report.snapshot
    );
    println!("diagnostics: {}", report.report.diagnostics.len());
    println!(
        "citation coverage: {} basis points; validity: {} basis points; diagram validity: {} basis points",
        report.report.metrics.citation_coverage_basis_points,
        report.report.metrics.citation_validity_basis_points,
        report.report.metrics.diagram_validity_basis_points
    );
    println!("report: {}", report.validation_path.display());
    Ok(())
}
