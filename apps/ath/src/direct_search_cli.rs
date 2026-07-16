use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

use athanor_app::{SearchOptions, search_project_with_composition_and_operation_context};

use crate::direct_operation::{await_drained_operation, operation};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectSearchCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Search {
        query: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("search") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectSearchCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print direct search help")?;
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
    let composition = athanor_runtime_defaults::production();

    match command {
        Command::Search {
            query,
            path,
            limit,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("search", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                search_project_with_composition_and_operation_context(
                    SearchOptions {
                        root: path,
                        query,
                        limit,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_report(&report);
            }
        }
    }
    Ok(())
}

fn print_report(report: &athanor_app::SearchReport) {
    println!(
        "search results for query \"{}\" in snapshot {} ({} of limit {}):",
        report.query, report.snapshot, report.returned, report.limit
    );
    if report.results.is_empty() {
        println!("No results found.");
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_preserves_search_flags_and_accepts_deadline() {
        let command = parse(&[
            "search".to_string(),
            "login".to_string(),
            "--limit".to_string(),
            "5".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused search command");

        assert!(matches!(
            command,
            Command::Search {
                limit: 5,
                deadline_unix_ms: Some(42),
                ..
            }
        ));
    }
}
