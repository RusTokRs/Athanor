use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    CancellationToken, DOCUMENTATION_COMPLETENESS_DEFAULT_LIMIT,
    DocumentationCompletenessOperationOptions, DocumentationCompletenessReport,
    DocumentationCompletenessRequest, VersionedDocumentationCompletenessReport,
    VersionedJsonContract, documentation_completeness_with_composition_cancellable,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

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
    /// Report deterministic documentation completeness for one exact committed snapshot.
    Completeness {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        snapshot: String,
        #[arg(long, default_value_t = DOCUMENTATION_COMPLETENESS_DEFAULT_LIMIT)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("docs")
        || args.get(1).map(String::as_str) != Some("completeness")
    {
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
            error.print().context("failed to print documentation completeness help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();
    match command {
        Command::Completeness {
            path,
            snapshot,
            limit,
            json,
        } => {
            let request = DocumentationCompletenessRequest::new(snapshot, limit);
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            let mut operation = Box::pin(documentation_completeness_with_composition_cancellable(
                DocumentationCompletenessOperationOptions {
                    root: path,
                    request,
                },
                &composition,
                cancellation,
            ));
            let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
            let report = tokio::select! {
                result = &mut operation => result?,
                signal = &mut ctrl_c => {
                    signal.context("failed to listen for completeness report cancellation")?;
                    signal_cancellation.cancel();
                    operation.await?
                }
            };
            render(&report, json)?;
        }
    }
    Ok(())
}

fn render(report: &DocumentationCompletenessReport, json: bool) -> Result<()> {
    if json {
        let transport = VersionedDocumentationCompletenessReport::from(report);
        transport
            .validate_contract()
            .context("invalid documentation completeness JSON contract")?;
        println!("{}", serde_json::to_string_pretty(&transport)?);
        return Ok(());
    }

    println!(
        "documentation completeness for {}: {} tracked files, {} processed ({} bp), {} unprocessed",
        report.snapshot,
        report.totals.tracked_files,
        report.totals.processed_files,
        report.totals.processed_ratio_basis_points,
        report.totals.unprocessed_files
    );
    println!(
        "canonical objects: {} facts, {} relations, {} diagnostics; {} named content adapters",
        report.totals.facts,
        report.totals.relations,
        report.totals.diagnostics,
        report.totals.adapters
    );
    println!("languages:");
    if report.languages.is_empty() {
        println!("  (none)");
    } else {
        for row in &report.languages {
            println!(
                "  - {}: {} tracked, {} processed ({} bp), {} unprocessed, {} adapters",
                row.language,
                row.tracked_files,
                row.processed_files,
                row.processed_ratio_basis_points,
                row.unprocessed_files,
                row.adapters
            );
        }
    }
    println!("adapters:");
    if report.adapters.is_empty() {
        println!("  (none)");
    } else {
        for row in &report.adapters {
            println!(
                "  - {}: {} files, {} facts, {} relations, {} diagnostics",
                row.adapter, row.files, row.facts, row.relations, row.diagnostics
            );
        }
    }
    println!("unprocessed files:");
    if report.unprocessed_files.is_empty() {
        println!("  (none)");
    } else {
        for row in &report.unprocessed_files {
            println!("  - {} [{}]", row.path, row.language);
        }
    }
    if report.omitted.languages > 0
        || report.omitted.adapters > 0
        || report.omitted.unprocessed_files > 0
    {
        println!(
            "omitted: {} languages, {} adapters, {} unprocessed files (limit {})",
            report.omitted.languages,
            report.omitted.adapters,
            report.omitted.unprocessed_files,
            report.limit
        );
    }
    Ok(())
}
