use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    BenchmarkOptions, BenchmarkReport, BenchmarkSize, IndexOptions, IndexReport, InitOptions,
    benchmark_index, index_project_with_composition, init_project,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct IndexCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    Index {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        validation_report: Option<PathBuf>,
        #[arg(long)]
        validation_result: Option<PathBuf>,
        #[arg(long)]
        validate_only: bool,
        #[arg(long)]
        json: bool,
    },
    Bench {
        #[arg(long, value_enum, default_value_t = BenchSizeArg::Small)]
        size: BenchSizeArg,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        keep_fixture: bool,
        #[arg(long)]
        json: bool,
    },
    Update {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        changed: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum BenchSizeArg {
    Small,
    Medium,
    Large,
}

impl From<BenchSizeArg> for BenchmarkSize {
    fn from(value: BenchSizeArg) -> Self {
        match value {
            BenchSizeArg::Small => Self::Small,
            BenchSizeArg::Medium => Self::Medium,
            BenchSizeArg::Large => Self::Large,
        }
    }
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if !matches!(
        args.first().map(String::as_str),
        Some("init" | "index" | "bench" | "update")
    ) {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match IndexCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print index command help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Init { path } => {
            let report = init_project(InitOptions { root: path })?;
            println!("initialized Athanor project at {}", report.root.display());
            for path in report.created {
                println!("created {}", path.display());
            }
        }
        Command::Index {
            path,
            validation_report,
            validation_result,
            validate_only,
            json,
        } => {
            let composition = athanor_runtime_defaults::production();
            let report = index_project_with_composition(
                IndexOptions {
                    root: path,
                    validation_report,
                    validation_result,
                    validate_only,
                },
                &composition,
            )
            .await?;
            render_index(&report, "indexed", json)?;
        }
        Command::Bench {
            size,
            root,
            keep_fixture,
            json,
        } => {
            let report = benchmark_index(BenchmarkOptions {
                size: size.into(),
                root,
                keep_fixture,
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_benchmark(&report);
            }
        }
        Command::Update {
            path,
            changed,
            json,
        } => {
            if !changed {
                bail!("update requires --changed");
            }
            let composition = athanor_runtime_defaults::production();
            let report = index_project_with_composition(
                IndexOptions {
                    root: path,
                    validation_report: None,
                    validation_result: None,
                    validate_only: false,
                },
                &composition,
            )
            .await?;
            render_index(&report, "updated", json)?;
        }
    }
    Ok(())
}

fn render_index(report: &IndexReport, action: &str, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
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
            "{action} {} files into snapshot {}",
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
    println!(
        "metrics: total {} ms, pipeline {} ms",
        report.metrics.total_ms, report.metrics.pipeline.total_ms
    );
    Ok(())
}

fn print_benchmark(report: &BenchmarkReport) {
    let pipeline = &report.index.metrics.pipeline;
    println!(
        "benchmark {}: {} files, total {} ms, index {} ms",
        report.size.as_str(),
        report.files_written,
        report.total_ms,
        report.index.metrics.total_ms
    );
    println!(
        "pipeline: discovery {} ms, extraction {} ms, linking {} ms, checking {} ms, storage {} ms",
        pipeline.source_discovery_ms,
        pipeline.extraction_ms,
        pipeline.linking_ms,
        pipeline.checking_ms,
        pipeline.storage_ms
    );
    println!("snapshot: {}", report.index.snapshot);
    if report.kept_fixture {
        println!("fixture: {}", report.fixture_root.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_intercepts_index_family_only() {
        assert!(matches!(
            parse(&["index".to_string(), ".".to_string()]).unwrap(),
            Some(Command::Index { .. })
        ));
        assert!(parse(&["docs".to_string(), "check".to_string()])
            .unwrap()
            .is_none());
    }
}
