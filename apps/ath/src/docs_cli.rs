use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    CancellationToken, DocsApplyPatchOptions, DocsApplyPatchReport, DocsCheckOptions,
    DocsCheckReport, DocsDriftOptions, DocsDriftReport, DocsProposeFixOptions,
    DocumentationApiCurrentInspection, DocumentationApiManifestInspection,
    DocumentationApiOperationOptions, DocumentationApiPublicationReport,
    DocumentationApiPublicationStatus, DocumentationApiValidationInspection,
    DocumentationArchitectureCurrentInspection, DocumentationArchitectureManifestInspection,
    DocumentationArchitectureOperationOptions, DocumentationArchitecturePublicationReport,
    DocumentationArchitecturePublicationStatus, DocumentationArchitectureValidationInspection,
    DocumentationGenerationLimits, DocumentationGenerationRequest, DocumentationModuleCurrentInspection,
    DocumentationModuleManifestInspection, DocumentationModuleOperationOptions,
    DocumentationModulePublicationReport, DocumentationModulePublicationStatus,
    DocumentationModuleValidationInspection, DocumentationProfile, OperationsDocsCheckOptions,
    OperationsDocsCheckReport, VersionedDocsProposeFixReport, check_docs_with_composition,
    check_operations_docs_with_composition, docs_apply_patch_with_composition,
    docs_drift_with_composition, generate_documentation_api_with_composition_cancellable,
    generate_documentation_architecture_with_composition_cancellable,
    generate_documentation_module_with_composition_cancellable, inspect_documentation_api_current,
    inspect_documentation_api_manifest, inspect_documentation_api_validation,
    inspect_documentation_architecture_current, inspect_documentation_architecture_manifest,
    inspect_documentation_architecture_validation, inspect_documentation_module_current,
    inspect_documentation_module_manifest, inspect_documentation_module_validation,
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
struct DocsCli {
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
    Check {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Drift {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    ProposeFix {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    ApplyPatch {
        patch: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Generate deterministic architecture documentation from one exact committed snapshot.
    GenerateArchitecture {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Exact committed canonical snapshot id. Latest-snapshot fallback is intentionally unsupported.
        #[arg(long)]
        snapshot: String,
        /// Publish a new immutable generation even when current output is exactly up to date.
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
        /// Print the complete publication report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Generate deterministic module documentation from one exact committed snapshot.
    GenerateModule {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Exact committed canonical snapshot id. Latest-snapshot fallback is intentionally unsupported.
        #[arg(long)]
        snapshot: String,
        /// Publish a new immutable generation even when current output is exactly up to date.
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
        /// Print the complete publication report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Generate deterministic API documentation from one exact committed snapshot.
    GenerateApi {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Exact committed canonical snapshot id. Latest-snapshot fallback is intentionally unsupported.
        #[arg(long)]
        snapshot: String,
        /// Publish a new immutable generation even when current output is exactly up to date.
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
        /// Print the complete publication report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect the validated current architecture documentation generation.
    Architecture {
        #[command(subcommand)]
        command: ArchitectureCommand,
    },
    /// Inspect the validated current module documentation generation.
    Module {
        #[command(subcommand)]
        command: ModuleCommand,
    },
    /// Inspect the validated current API documentation generation.
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
    Operations {
        #[command(subcommand)]
        command: OperationsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ArchitectureCommand {
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

#[derive(Debug, Subcommand)]
pub(crate) enum ModuleCommand {
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

#[derive(Debug, Subcommand)]
pub(crate) enum ApiCommand {
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

#[derive(Debug, Subcommand)]
pub(crate) enum OperationsCommand {
    Check {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("docs") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DocsCli::try_parse_from(argv) {
        Ok(DocsCli {
            command: RootCommand::Docs { command },
        }) => Ok(Some(command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().context("failed to print docs help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();
    match command {
        Command::Check { path, json } => {
            let report =
                check_docs_with_composition(DocsCheckOptions { root: path }, &composition).await?;
            render_check(&report, json)?;
            if !report.passed {
                bail!("documentation completeness gate failed");
            }
        }
        Command::Drift { path, json } => {
            let report =
                docs_drift_with_composition(DocsDriftOptions { root: path }, &composition).await?;
            render_drift(&report, json)?;
        }
        Command::ProposeFix { path, output, json } => {
            let report = athanor_app::docs_propose_fix_with_composition(
                DocsProposeFixOptions { root: path, output },
                &composition,
            )
            .await?;
            let report = VersionedDocsProposeFixReport::from(report);
            render_proposal(&report, json)?;
        }
        Command::ApplyPatch { patch, path, json } => {
            let report = docs_apply_patch_with_composition(
                DocsApplyPatchOptions { root: path, patch },
                &composition,
            )
            .await?;
            render_apply(&report, json)?;
        }
        Command::GenerateArchitecture {
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
                DocumentationProfile::Architecture,
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
                generate_documentation_architecture_with_composition_cancellable(
                    DocumentationArchitectureOperationOptions {
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
                    signal.context("failed to listen for architecture generation cancellation")?;
                    signal_cancellation.cancel();
                    generation.await?
                }
            };
            render_architecture_generation(&report, json)?;
        }
        Command::GenerateModule {
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
                DocumentationProfile::Module,
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
                generate_documentation_module_with_composition_cancellable(
                    DocumentationModuleOperationOptions {
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
                    signal.context("failed to listen for module generation cancellation")?;
                    signal_cancellation.cancel();
                    generation.await?
                }
            };
            render_module_generation(&report, json)?;
        }
        Command::GenerateApi {
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
                DocumentationProfile::Api,
                DocumentationGenerationLimits {
                    max_entities,
                    max_facts,
                    max_relations,
                    max_diagnostics,
                },
            );
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            let mut generation = Box::pin(generate_documentation_api_with_composition_cancellable(
                DocumentationApiOperationOptions {
                    root: path,
                    request,
                    force,
                },
                &composition,
                cancellation,
            ));
            let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
            let report = tokio::select! {
                result = &mut generation => result?,
                signal = &mut ctrl_c => {
                    signal.context("failed to listen for API generation cancellation")?;
                    signal_cancellation.cancel();
                    generation.await?
                }
            };
            render_api_generation(&report, json)?;
        }
        Command::Architecture {
            command: ArchitectureCommand::Current { path, json },
        } => {
            let report = inspect_documentation_architecture_current(path)?;
            render_architecture_current(&report, json)?;
        }
        Command::Architecture {
            command: ArchitectureCommand::Manifest { path, json },
        } => {
            let report = inspect_documentation_architecture_manifest(path)?;
            render_architecture_manifest(&report, json)?;
        }
        Command::Architecture {
            command: ArchitectureCommand::Validation { path, json },
        } => {
            let report = inspect_documentation_architecture_validation(path)?;
            render_architecture_validation(&report, json)?;
        }
        Command::Module {
            command: ModuleCommand::Current { path, json },
        } => {
            let report = inspect_documentation_module_current(path)?;
            render_module_current(&report, json)?;
        }
        Command::Module {
            command: ModuleCommand::Manifest { path, json },
        } => {
            let report = inspect_documentation_module_manifest(path)?;
            render_module_manifest(&report, json)?;
        }
        Command::Module {
            command: ModuleCommand::Validation { path, json },
        } => {
            let report = inspect_documentation_module_validation(path)?;
            render_module_validation(&report, json)?;
        }
        Command::Api {
            command: ApiCommand::Current { path, json },
        } => {
            let report = inspect_documentation_api_current(path)?;
            render_api_current(&report, json)?;
        }
        Command::Api {
            command: ApiCommand::Manifest { path, json },
        } => {
            let report = inspect_documentation_api_manifest(path)?;
            render_api_manifest(&report, json)?;
        }
        Command::Api {
            command: ApiCommand::Validation { path, json },
        } => {
            let report = inspect_documentation_api_validation(path)?;
            render_api_validation(&report, json)?;
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
    }
    Ok(())
}

fn render_architecture_generation(
    report: &DocumentationArchitecturePublicationReport,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    match report.status {
        DocumentationArchitecturePublicationStatus::Published => println!(
            "published architecture documentation generation {} from snapshot {}",
            report.generation, report.snapshot
        ),
        DocumentationArchitecturePublicationStatus::UpToDate => println!(
            "architecture documentation generation {} is already up to date for snapshot {}",
            report.generation, report.snapshot
        ),
    }
    println!("document: {}", report.document.display());
    println!("validation: {}", report.validation_report.display());
    println!("manifest: {}", report.manifest.display());
    println!("current: {}", report.current_pointer.display());
    Ok(())
}

fn render_module_generation(report: &DocumentationModulePublicationReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    match report.status {
        DocumentationModulePublicationStatus::Published => println!(
            "published module documentation generation {} from snapshot {}",
            report.generation, report.snapshot
        ),
        DocumentationModulePublicationStatus::UpToDate => println!(
            "module documentation generation {} is already up to date for snapshot {}",
            report.generation, report.snapshot
        ),
    }
    println!("document: {}", report.document.display());
    println!("validation: {}", report.validation_report.display());
    println!("manifest: {}", report.manifest.display());
    println!("current: {}", report.current_pointer.display());
    Ok(())
}

fn render_api_generation(report: &DocumentationApiPublicationReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    match report.status {
        DocumentationApiPublicationStatus::Published => println!(
            "published API documentation generation {} from snapshot {}",
            report.generation, report.snapshot
        ),
        DocumentationApiPublicationStatus::UpToDate => println!(
            "API documentation generation {} is already up to date for snapshot {}",
            report.generation, report.snapshot
        ),
    }
    println!("document: {}", report.document.display());
    println!("validation: {}", report.validation_report.display());
    println!("manifest: {}", report.manifest.display());
    println!("current: {}", report.current_pointer.display());
    Ok(())
}

fn render_architecture_current(
    report: &DocumentationArchitectureCurrentInspection,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "current architecture documentation generation {} for snapshot {}",
            report.current.generation, report.current.snapshot
        );
        println!("profile: architecture");
        println!("path: {}", report.current.path);
        println!("manifest: {}", report.current.manifest);
        println!("pointer: {}", report.current_pointer.display());
    }
    Ok(())
}

fn render_module_current(report: &DocumentationModuleCurrentInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "current module documentation generation {} for snapshot {}",
            report.current.generation, report.current.snapshot
        );
        println!("profile: module");
        println!("path: {}", report.current.path);
        println!("manifest: {}", report.current.manifest);
        println!("pointer: {}", report.current_pointer.display());
    }
    Ok(())
}

fn render_api_current(report: &DocumentationApiCurrentInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "current API documentation generation {} for snapshot {}",
            report.current.generation, report.current.snapshot
        );
        println!("profile: api");
        println!("path: {}", report.current.path);
        println!("manifest: {}", report.current.manifest);
        println!("pointer: {}", report.current_pointer.display());
    }
    Ok(())
}

fn render_architecture_manifest(
    report: &DocumentationArchitectureManifestInspection,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "architecture documentation manifest {} for snapshot {}: {} artifacts",
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

fn render_module_manifest(report: &DocumentationModuleManifestInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "module documentation manifest {} for snapshot {}: {} artifacts",
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

fn render_api_manifest(report: &DocumentationApiManifestInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "API documentation manifest {} for snapshot {}: {} artifacts",
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

fn render_architecture_validation(
    report: &DocumentationArchitectureValidationInspection,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "architecture documentation validation for snapshot {}: valid",
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

fn render_module_validation(report: &DocumentationModuleValidationInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "module documentation validation for snapshot {}: valid",
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

fn render_api_validation(report: &DocumentationApiValidationInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "API documentation validation for snapshot {}: valid",
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

fn render_check(report: &DocsCheckReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
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
        let kind = serde_json::to_value(&diagnostic.kind)?
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        println!("diagnostic: {kind} — {}", diagnostic.title);
    }
    Ok(())
}

fn render_drift(report: &DocsDriftReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
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
    Ok(())
}

fn render_proposal(report: &VersionedDocsProposeFixReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    let report = &report.report;
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
    Ok(())
}

fn render_apply(report: &DocsApplyPatchReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "applied docs patch {} for snapshot {}: {} files changed, {} frontmatter changes",
            report.id, report.snapshot, report.files_changed, report.changes_applied
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_architecture_generation_with_limits_and_json() {
        let command = parse(&[
            "docs".to_string(),
            "generate-architecture".to_string(),
            "project".to_string(),
            "--snapshot".to_string(),
            "snap-exact".to_string(),
            "--max-entities".to_string(),
            "12".to_string(),
            "--force".to_string(),
            "--json".to_string(),
        ])
        .unwrap()
        .expect("docs generation command");
        assert!(matches!(
            command,
            Command::GenerateArchitecture {
                snapshot,
                max_entities: 12,
                force: true,
                json: true,
                ..
            } if snapshot == "snap-exact"
        ));
    }

    #[test]
    fn parses_exact_module_generation_with_limits_and_json() {
        let command = parse(&[
            "docs".to_string(),
            "generate-module".to_string(),
            "project".to_string(),
            "--snapshot".to_string(),
            "snap-exact".to_string(),
            "--max-relations".to_string(),
            "7".to_string(),
            "--force".to_string(),
            "--json".to_string(),
        ])
        .unwrap()
        .expect("module generation command");
        assert!(matches!(
            command,
            Command::GenerateModule {
                snapshot,
                max_relations: 7,
                force: true,
                json: true,
                ..
            } if snapshot == "snap-exact"
        ));
    }

    #[test]
    fn parses_exact_api_generation_with_limits_and_json() {
        let command = parse(&[
            "docs".to_string(),
            "generate-api".to_string(),
            "project".to_string(),
            "--snapshot".to_string(),
            "snap-exact".to_string(),
            "--max-diagnostics".to_string(),
            "5".to_string(),
            "--force".to_string(),
            "--json".to_string(),
        ])
        .unwrap()
        .expect("API generation command");
        assert!(matches!(
            command,
            Command::GenerateApi {
                snapshot,
                max_diagnostics: 5,
                force: true,
                json: true,
                ..
            } if snapshot == "snap-exact"
        ));
    }

    #[test]
    fn parses_bounded_architecture_inspection_commands() {
        for (name, expected) in [
            ("current", "current"),
            ("manifest", "manifest"),
            ("validation", "validation"),
        ] {
            let command = parse(&[
                "docs".to_string(),
                "architecture".to_string(),
                name.to_string(),
                "project".to_string(),
                "--json".to_string(),
            ])
            .unwrap()
            .expect("architecture inspection command");
            match (expected, command) {
                (
                    "current",
                    Command::Architecture {
                        command: ArchitectureCommand::Current { json: true, .. },
                    },
                )
                | (
                    "manifest",
                    Command::Architecture {
                        command: ArchitectureCommand::Manifest { json: true, .. },
                    },
                )
                | (
                    "validation",
                    Command::Architecture {
                        command: ArchitectureCommand::Validation { json: true, .. },
                    },
                ) => {}
                _ => panic!("unexpected architecture inspection command"),
            }
        }
    }

    #[test]
    fn parses_bounded_module_inspection_commands() {
        for (name, expected) in [
            ("current", "current"),
            ("manifest", "manifest"),
            ("validation", "validation"),
        ] {
            let command = parse(&[
                "docs".to_string(),
                "module".to_string(),
                name.to_string(),
                "project".to_string(),
                "--json".to_string(),
            ])
            .unwrap()
            .expect("module inspection command");
            match (expected, command) {
                (
                    "current",
                    Command::Module {
                        command: ModuleCommand::Current { json: true, .. },
                    },
                )
                | (
                    "manifest",
                    Command::Module {
                        command: ModuleCommand::Manifest { json: true, .. },
                    },
                )
                | (
                    "validation",
                    Command::Module {
                        command: ModuleCommand::Validation { json: true, .. },
                    },
                ) => {}
                _ => panic!("unexpected module inspection command"),
            }
        }
    }

    #[test]
    fn parses_bounded_api_inspection_commands() {
        for (name, expected) in [
            ("current", "current"),
            ("manifest", "manifest"),
            ("validation", "validation"),
        ] {
            let command = parse(&[
                "docs".to_string(),
                "api".to_string(),
                name.to_string(),
                "project".to_string(),
                "--json".to_string(),
            ])
            .unwrap()
            .expect("API inspection command");
            match (expected, command) {
                (
                    "current",
                    Command::Api {
                        command: ApiCommand::Current { json: true, .. },
                    },
                )
                | (
                    "manifest",
                    Command::Api {
                        command: ApiCommand::Manifest { json: true, .. },
                    },
                )
                | (
                    "validation",
                    Command::Api {
                        command: ApiCommand::Validation { json: true, .. },
                    },
                ) => {}
                _ => panic!("unexpected API inspection command"),
            }
        }
    }
}
