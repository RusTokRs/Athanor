use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    DocsApplyPatchOptions, DocsApplyPatchReport, DocsCheckOptions, DocsCheckReport, DocsDriftOptions,
    DocsDriftReport, OperationsDocsCheckOptions, OperationsDocsCheckReport, check_docs,
    check_operations_docs_with_composition, docs_apply_patch, docs_drift,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

use crate::render::check;

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
    ApplyPatch {
        patch: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
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
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    let selected = matches!(
        args.get(0..2),
        Some([root, command])
            if root == "docs"
                && matches!(command.as_str(), "check" | "drift" | "apply-patch" | "operations")
    );
    if !selected {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DocsCli::try_parse_from(argv) {
        Ok(DocsCli {
            command: RootCommand::Docs { command },
        }) => Ok(Some(command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print docs help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Check { path, json } => {
            let report = check_docs(DocsCheckOptions { root: path }).await?;
            render_check(&report, json)?;
            if !report.passed {
                bail!("documentation completeness gate failed");
            }
        }
        Command::Drift { path, json } => {
            let report = docs_drift(DocsDriftOptions { root: path }).await?;
            render_drift(&report, json)?;
        }
        Command::ApplyPatch { patch, path, json } => {
            let report = docs_apply_patch(DocsApplyPatchOptions { root: path, patch }).await?;
            render_apply(&report, json)?;
        }
        Command::Operations {
            command: OperationsCommand::Check { path, json },
        } => {
            let composition = athanor_runtime_defaults::production();
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
