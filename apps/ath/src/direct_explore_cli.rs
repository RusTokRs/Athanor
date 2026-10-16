use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    DEFAULT_EXPLORE_DEPTH, DEFAULT_EXPLORE_ENTRIES, DEFAULT_EXPLORE_LINKS, DEFAULT_EXPLORE_PATHS,
    ExploreOptions, ExploreReport, explore_project_with_composition_and_operation_context,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

use crate::direct_operation::{await_drained_operation, operation};
use crate::direct_search_cli::SearchModeArg;

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectExploreCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Answer one code question in a single bounded call over the latest canonical snapshot.
    Explore {
        /// Retrieval query; ignored when `--target` resolves an exact entity or path.
        query: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Exact canonical stable key or repository path to explore instead of searching.
        #[arg(long)]
        target: Option<String>,
        #[arg(long, default_value_t = DEFAULT_EXPLORE_ENTRIES)]
        entries: usize,
        #[arg(long, default_value_t = DEFAULT_EXPLORE_LINKS)]
        links: usize,
        #[arg(long, default_value_t = DEFAULT_EXPLORE_DEPTH)]
        depth: usize,
        #[arg(long, default_value_t = DEFAULT_EXPLORE_PATHS)]
        paths: usize,
        #[arg(long, value_enum, default_value_t = SearchModeArg::Lexical)]
        mode: SearchModeArg,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("explore") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectExploreCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error
                .print()
                .context("failed to print direct explore help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();

    match command {
        Command::Explore {
            query,
            path,
            target,
            entries,
            links,
            depth,
            paths,
            mode,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("explore", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                explore_project_with_composition_and_operation_context(
                    ExploreOptions {
                        root: path,
                        query: query.unwrap_or_default(),
                        target,
                        entries,
                        links,
                        depth,
                        paths,
                        mode: mode.into(),
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

fn print_report(report: &ExploreReport) {
    let anchor = report
        .anchor
        .as_deref()
        .filter(|anchor| !anchor.is_empty())
        .unwrap_or(report.query.as_str());
    println!(
        "explore \"{anchor}\" in snapshot {} (mode {}, depth {}, {} entries):",
        report.snapshot,
        report.mode,
        report.limits.depth,
        report.entries.len()
    );
    if report.entries.is_empty() {
        println!("No canonical entities matched this request.");
    }
    for entry in &report.entries {
        let source = entry
            .source
            .as_ref()
            .map(|source| describe_source(source.path.as_str(), source.line_start))
            .unwrap_or_else(|| "no source anchor".to_string());
        println!(
            "[{:.4}] {} ({}) - {} @ {source}",
            entry.score, entry.name, entry.kind, entry.stable_key
        );
        for citation in &entry.citations {
            if let Some(path) = &citation.source_file {
                println!(
                    "  cite: {}",
                    describe_source(path.as_str(), citation.line_start)
                );
            }
        }
        for link in &entry.outgoing {
            println!(
                "  -> {} {} ({})",
                link.relation, link.counterpart.stable_key, link.status
            );
        }
        for link in &entry.incoming {
            println!(
                "  <- {} {} ({})",
                link.relation, link.counterpart.stable_key, link.status
            );
        }
        if entry.open_diagnostics > 0 {
            println!("  diagnostics: {} open", entry.open_diagnostics);
        }
    }

    if !report.paths.is_empty() {
        println!();
        println!("Call and relation chains ({}):", report.paths.len());
        for path in &report.paths {
            let steps = path
                .steps
                .iter()
                .map(|step| {
                    format!(
                        "{} -{}({}){} {}",
                        step.from.name,
                        step.relation,
                        step.status,
                        if step.direction == "backward" {
                            "<-"
                        } else {
                            "->"
                        },
                        step.to.name
                    )
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            println!("  {} [{}] {}", path.depth, path.target.stable_key, steps);
        }
    }

    println!();
    println!(
        "blast radius within {} hop(s): {} entries, {} reached entities, {} relations, {} files",
        report.blast.depth,
        report.blast.entries,
        report.blast.reached,
        report.blast.relations,
        report.blast.files.len()
    );
    for file in &report.blast.files {
        println!("  file: {file}");
    }
    if !report.omitted.notes.is_empty() {
        println!();
        for note in &report.omitted.notes {
            println!("omitted: {note}");
        }
    }
}

fn describe_source(path: &str, line_start: Option<u32>) -> String {
    match line_start {
        Some(line) => format!("{path}:{line}"),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_keeps_explore_budgets_and_target() {
        let command = parse(&[
            "explore".to_string(),
            "store load".to_string(),
            "--entries".to_string(),
            "3".to_string(),
            "--links".to_string(),
            "4".to_string(),
            "--depth".to_string(),
            "3".to_string(),
            "--paths".to_string(),
            "9".to_string(),
            "--target".to_string(),
            "src/store.rs".to_string(),
            "--mode".to_string(),
            "hybrid".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused explore command");

        assert!(matches!(
            command,
            Command::Explore {
                entries: 3,
                links: 4,
                depth: 3,
                paths: 9,
                mode: SearchModeArg::Hybrid,
                deadline_unix_ms: Some(42),
                ..
            }
        ));
    }

    #[test]
    fn parser_accepts_a_target_without_a_query() {
        let command = parse(&[
            "explore".to_string(),
            "--target".to_string(),
            "symbol://rust:crate::store".to_string(),
        ])
        .unwrap()
        .expect("anchored explore command");

        assert!(matches!(command, Command::Explore { query: None, .. }));
    }
}
