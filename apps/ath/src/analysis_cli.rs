use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    CapabilitiesOptions, CapabilitiesReport, CoverageOptions, CoverageReport,
    DEFAULT_CAPABILITIES_LIMIT, DEFAULT_CONFIDENCE_THRESHOLD,
    capabilities_project_with_composition, coverage_project_with_composition,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct AnalysisCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Coverage {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Capabilities {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = DEFAULT_CAPABILITIES_LIMIT)]
        limit: usize,
        #[arg(long = "min-confidence", default_value_t = DEFAULT_CONFIDENCE_THRESHOLD)]
        confidence_threshold: f32,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if !matches!(
        args.first().map(String::as_str),
        Some("coverage" | "capabilities")
    ) {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match AnalysisCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().context("failed to print analysis help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();
    match command {
        Command::Coverage {
            path,
            adapter,
            file,
            limit,
            json,
        } => {
            if limit == 0 {
                bail!("--limit must be greater than zero");
            }
            let report = coverage_project_with_composition(
                CoverageOptions {
                    root: path,
                    adapter,
                    file,
                    limit,
                },
                &composition,
            )
            .await?;
            render_coverage(&report, json)?;
        }
        Command::Capabilities {
            path,
            limit,
            confidence_threshold,
            json,
        } => {
            if limit == 0 {
                bail!("--limit must be greater than zero");
            }
            if !(0.0..=1.0).contains(&confidence_threshold) {
                bail!("--min-confidence must be between 0.0 and 1.0");
            }
            let report = capabilities_project_with_composition(
                CapabilitiesOptions {
                    root: path,
                    limit,
                    confidence_threshold,
                },
                &composition,
            )
            .await?;
            render_capabilities(&report, json)?;
        }
    }
    Ok(())
}

fn render_coverage(report: &CoverageReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "coverage for {}: {} tracked files, {} files with canonical objects, {} open diagnostics",
        report.snapshot,
        report.totals.tracked_files,
        report.totals.files_with_canonical_objects,
        report.totals.open_diagnostics
    );
    println!(
        "canonical objects: {} entities, {} facts, {} relations, {} diagnostics",
        report.totals.entities,
        report.totals.facts,
        report.totals.relations,
        report.totals.diagnostics
    );
    if let Some(adapter) = &report.filters.adapter {
        println!("adapter filter: {adapter}");
    }
    if let Some(file) = &report.filters.file {
        println!("file filter: {file}");
    }
    println!("adapters:");
    if report.adapters.is_empty() {
        println!("  (none)");
    } else {
        for adapter in &report.adapters {
            println!(
                "  - {}: {} files, {} facts, {} evidence items, {} diagnostics",
                adapter.adapter,
                adapter.files,
                adapter.facts,
                adapter.evidence_items,
                adapter.diagnostics
            );
        }
    }
    println!("files:");
    if report.files.is_empty() {
        println!("  (none)");
    } else {
        for file in &report.files {
            println!(
                "  - {}: {} entities, {} facts, {} relations, {} diagnostics ({} open)",
                file.path,
                file.entities,
                file.facts,
                file.relations,
                file.diagnostics,
                file.open_diagnostics
            );
        }
    }
    println!("diagnostics:");
    if report.diagnostics.is_empty() {
        println!("  (none)");
    } else {
        for diagnostic in &report.diagnostics {
            println!(
                "  - {}: {} total, {} open, {} files",
                diagnostic.kind, diagnostic.total, diagnostic.open, diagnostic.files
            );
        }
    }
    if report.omitted.files > 0 || report.omitted.adapters > 0 || report.omitted.diagnostics > 0 {
        println!(
            "omitted: {} files, {} adapters, {} diagnostic kinds (limit {})",
            report.omitted.files,
            report.omitted.adapters,
            report.omitted.diagnostics,
            report.limits.limit
        );
    }
    Ok(())
}

fn render_capabilities(report: &CapabilitiesReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "capabilities for {}: {} tracked files, {} content-processed ({}%), {} unprocessed",
        report.snapshot,
        report.totals.tracked_files,
        report.totals.processed_files,
        report.totals.processed_ratio_percent,
        report.totals.unprocessed_files
    );
    println!(
        "processed means canonical objects from an adapter other than the `{}` baseline inventory adapter",
        report.baseline_adapter
    );
    println!(
        "facts: {} total, {} below confidence {} across {} adapters",
        report.totals.facts,
        report.totals.low_confidence_facts,
        report.limits.confidence_threshold,
        report.totals.adapters
    );
    println!("languages:");
    if report.languages.is_empty() {
        println!("  (none)");
    } else {
        for language in &report.languages {
            println!(
                "  - {}: {} tracked, {} processed ({}%), {} unprocessed",
                language.language_hint,
                language.tracked_files,
                language.processed_files,
                language.processed_ratio_percent,
                language.unprocessed_files
            );
        }
    }
    println!("adapters:");
    if report.adapters.is_empty() {
        println!("  (none)");
    } else {
        for adapter in &report.adapters {
            println!(
                "  - {}: {} files, {} facts, {} low confidence, min confidence {}",
                adapter.adapter,
                adapter.processed_files,
                adapter.facts,
                adapter.low_confidence_facts,
                adapter.min_confidence
            );
        }
    }
    println!("unprocessed files:");
    if report.unprocessed_files.is_empty() {
        println!("  (none)");
    } else {
        for file in &report.unprocessed_files {
            println!("  - {} [{}]", file.path, file.language_hint);
        }
    }
    println!("low confidence facts:");
    if report.low_confidence_facts.is_empty() {
        println!("  (none)");
    } else {
        for fact in &report.low_confidence_facts {
            let location = fact.path.as_deref().map_or_else(
                || "(no evidence path)".to_string(),
                |path| match fact.line_start {
                    Some(line) => format!("{path}:{line}"),
                    None => path.to_string(),
                },
            );
            println!(
                "  - {} ({}) confidence {} [{}] {}",
                fact.kind, fact.adapter, fact.confidence, location, fact.fact_id
            );
        }
    }
    if report.omitted.languages > 0
        || report.omitted.adapters > 0
        || report.omitted.unprocessed_files > 0
        || report.omitted.low_confidence_facts > 0
    {
        println!(
            "omitted: {} languages, {} adapters, {} unprocessed files, {} low confidence facts (limit {})",
            report.omitted.languages,
            report.omitted.adapters,
            report.omitted.unprocessed_files,
            report.omitted.low_confidence_facts,
            report.limits.limit
        );
    }
    Ok(())
}
