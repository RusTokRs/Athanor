use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    ApiRetentionOverrides, ApiSnapshotOptions, DocsProposeFixOptions,
    VersionedApiSnapshotReport, VersionedDocsProposeFixReport, docs_propose_fix,
    snapshot_api_contract,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectApplicationReportCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
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
}

#[derive(Debug, Subcommand)]
enum DocsCommand {
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
}

pub(crate) enum Command {
    ApiSnapshot {
        path: PathBuf,
        cleanup: bool,
        no_cleanup: bool,
        keep_snapshots: Option<usize>,
        keep_diffs: Option<usize>,
        json: bool,
    },
    DocsProposeFix {
        path: PathBuf,
        output: Option<PathBuf>,
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    let selected = matches!(
        args.get(0..2),
        Some([root, command])
            if (root.as_str() == "api" && command.as_str() == "snapshot")
                || (root.as_str() == "docs" && command.as_str() == "propose-fix")
    );
    if !selected {
        return Ok(None);
    }

    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectApplicationReportCli::try_parse_from(argv) {
        Ok(DirectApplicationReportCli {
            command:
                RootCommand::Api {
                    command:
                        ApiCommand::Snapshot {
                            path,
                            cleanup,
                            no_cleanup,
                            keep_snapshots,
                            keep_diffs,
                            json,
                        },
                },
        }) => Ok(Some(Command::ApiSnapshot {
            path,
            cleanup,
            no_cleanup,
            keep_snapshots,
            keep_diffs,
            json,
        })),
        Ok(DirectApplicationReportCli {
            command:
                RootCommand::Docs {
                    command: DocsCommand::ProposeFix { path, output, json },
                },
        }) => Ok(Some(Command::DocsProposeFix { path, output, json })),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error
                .print()
                .context("failed to print versioned application report help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    #[allow(deprecated)]
    {
        athanor_runtime_defaults::install();
    }

    match command {
        Command::ApiSnapshot {
            path,
            cleanup,
            no_cleanup,
            keep_snapshots,
            keep_diffs,
            json,
        } => {
            let report = snapshot_api_contract(ApiSnapshotOptions {
                root: path,
                retention: retention_overrides(cleanup, no_cleanup, keep_snapshots, keep_diffs),
            })
            .await?;
            let report = VersionedApiSnapshotReport::from(report);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_api_snapshot_report(&report);
            }
        }
        Command::DocsProposeFix { path, output, json } => {
            let report = docs_propose_fix(DocsProposeFixOptions { root: path, output }).await?;
            let report = VersionedDocsProposeFixReport::from(report);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_docs_propose_fix_report(&report);
            }
        }
    }
    Ok(())
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

fn print_api_snapshot_report(report: &VersionedApiSnapshotReport) {
    let report = &report.report;
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
        println!(
            "API cleanup: {} removed, {} retained{}",
            cleanup.removed.len(),
            cleanup.retained.len(),
            if cleanup.dry_run { " (dry run)" } else { "" }
        );
    }
}

fn print_docs_propose_fix_report(report: &VersionedDocsProposeFixReport) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_unrelated_api_and_docs_commands() {
        assert!(parse(&["api".to_string(), "diff".to_string()]).unwrap().is_none());
        assert!(parse(&["docs".to_string(), "check".to_string()]).unwrap().is_none());
    }

    #[test]
    fn parses_selected_versioned_report_commands() {
        assert!(matches!(
            parse(&["api".to_string(), "snapshot".to_string()]).unwrap(),
            Some(Command::ApiSnapshot { .. })
        ));
        assert!(matches!(
            parse(&["docs".to_string(), "propose-fix".to_string()]).unwrap(),
            Some(Command::DocsProposeFix { .. })
        ));
    }
}
