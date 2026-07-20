use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    ApiCleanupOptions, ApiCleanupReport, ApiDiffOptions, ApiRegistryOptions, ApiRegistryReport,
    ApiRetentionOverrides, ApiSnapshotOptions, VersionedApiSnapshotReport, cleanup_api_contracts,
    diff_api_contracts, query_api_registry_with_composition,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

use crate::render::api;

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct ApiCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Api {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Snapshot {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, conflicts_with = "no_cleanup")]
        cleanup: bool,
        #[arg(long = "no-cleanup")]
        no_cleanup: bool,
        #[arg(long)]
        keep_snapshots: Option<usize>,
        #[arg(long)]
        keep_diffs: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    Diff {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, conflicts_with = "no_cleanup")]
        cleanup: bool,
        #[arg(long = "no-cleanup")]
        no_cleanup: bool,
        #[arg(long)]
        keep_snapshots: Option<usize>,
        #[arg(long)]
        keep_diffs: Option<usize>,
        #[arg(long)]
        json: bool,
    },
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
    Registry {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Cleanup {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value_t = 2)]
        keep_snapshots: usize,
        #[arg(long, default_value_t = 2)]
        keep_diffs: usize,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("api") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match ApiCli::try_parse_from(argv) {
        Ok(ApiCli {
            command: RootCommand::Api { command },
        }) => Ok(Some(command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().context("failed to print API help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();
    match command {
        Command::Snapshot {
            path,
            cleanup,
            no_cleanup,
            keep_snapshots,
            keep_diffs,
            json,
        } => {
            let report = athanor_app::snapshot_api_contract_with_composition(
                ApiSnapshotOptions {
                    root: path,
                    retention: retention_overrides(cleanup, no_cleanup, keep_snapshots, keep_diffs),
                },
                &composition,
            )
            .await?;
            let report = VersionedApiSnapshotReport::from(report);
            render_snapshot(&report, json)?;
        }
        Command::Diff {
            from,
            to,
            path,
            cleanup,
            no_cleanup,
            keep_snapshots,
            keep_diffs,
            json,
        } => {
            let diff = diff_api_contracts(ApiDiffOptions {
                root: path,
                from,
                to,
                retention: retention_overrides(cleanup, no_cleanup, keep_snapshots, keep_diffs),
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&diff)?);
            } else {
                api::print_contract_diff(&diff)?;
            }
        }
        Command::BreakingChanges {
            from,
            to,
            path,
            json,
        } => {
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
                api::print_contract_diff(&diff)?;
            }
            if diff.breaking_changes > 0 {
                bail!(
                    "API contract contains {} breaking changes",
                    diff.breaking_changes
                );
            }
        }
        Command::Registry { path, json } => {
            let report = query_api_registry_with_composition(
                ApiRegistryOptions { root: path },
                &composition,
            )
            .await?;
            render_registry(&report, json)?;
        }
        Command::Cleanup {
            path,
            dry_run,
            keep_snapshots,
            keep_diffs,
            json,
        } => {
            let report = cleanup_api_contracts(ApiCleanupOptions {
                root: path,
                dry_run,
                keep_snapshots,
                keep_diffs,
            })?;
            render_cleanup(&report, json)?;
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

fn render_snapshot(report: &VersionedApiSnapshotReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
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
    Ok(())
}

fn render_registry(report: &ApiRegistryReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
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
            if let Some(operation_id) = &endpoint.operation_id {
                println!("    operationId: {operation_id}");
            }
            if let Some(summary) = &endpoint.summary {
                println!("    summary: {summary}");
            }
            if let Some(handler) = &endpoint.handler {
                println!("    handler: {handler}");
            }
            for document in &endpoint.documentation {
                println!("    documentation: {document}");
            }
        }
    }
    Ok(())
}

fn render_cleanup(report: &ApiCleanupReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "API cleanup: {} removed, {} retained{}",
        report.removed.len(),
        report.retained.len(),
        if report.dry_run { " (dry run)" } else { "" }
    );
    for artifact in &report.removed {
        println!(
            "  remove {:?} {} at {}",
            artifact.kind,
            artifact.id,
            artifact.path.display()
        );
    }
    Ok(())
}
