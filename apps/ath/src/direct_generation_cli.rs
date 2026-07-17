use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    GenerationOptions, GenerationStatus, HtmlReportOptions, WikiOptions,
    generate_project_with_composition, project_html_report_with_composition,
    project_wiki_with_composition,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectGenerationCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    /// Build a Markdown wiki from the latest canonical snapshot.
    Wiki {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Wiki output directory. Relative paths are resolved from the project root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Print the complete wiki report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Build generated reports from the latest canonical snapshot.
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    /// Publish JSONL, Markdown, and HTML as one immutable generated generation.
    Generate {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print the complete generation report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ReportCommand {
    /// Build a self-contained static HTML report.
    Html {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Report output directory. Relative paths are resolved from the project root.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Print the complete HTML report as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) enum Command {
    Wiki {
        path: PathBuf,
        output: Option<PathBuf>,
        json: bool,
    },
    Html {
        path: PathBuf,
        output: Option<PathBuf>,
        json: bool,
    },
    Generate {
        path: PathBuf,
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    let selected = matches!(args.first().map(String::as_str), Some("wiki" | "generate"))
        || matches!(args.get(0..2), Some([root, command]) if root == "report" && command == "html");
    if !selected {
        return Ok(None);
    }

    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectGenerationCli::try_parse_from(argv) {
        Ok(DirectGenerationCli {
            command: RootCommand::Wiki { path, output, json },
        }) => Ok(Some(Command::Wiki { path, output, json })),
        Ok(DirectGenerationCli {
            command:
                RootCommand::Report {
                    command: ReportCommand::Html { path, output, json },
                },
        }) => Ok(Some(Command::Html { path, output, json })),
        Ok(DirectGenerationCli {
            command: RootCommand::Generate { path, json },
        }) => Ok(Some(Command::Generate { path, json })),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error
                .print()
                .context("failed to print direct generation help")?;
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
        Command::Wiki { path, output, json } => {
            let report = project_wiki_with_composition(
                WikiOptions { root: path, output },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "projected {} entities and {} open diagnostics from snapshot {}",
                    report.entities, report.open_diagnostics, report.snapshot
                );
                println!("wrote Markdown wiki to {}", report.output_dir.display());
            }
        }
        Command::Html { path, output, json } => {
            let report = project_html_report_with_composition(
                HtmlReportOptions { root: path, output },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "projected {} entities and {} open diagnostics from snapshot {}",
                    report.entities, report.open_diagnostics, report.snapshot
                );
                println!("wrote HTML report to {}", report.output_dir.display());
            }
        }
        Command::Generate { path, json } => {
            let report = generate_project_with_composition(
                GenerationOptions {
                    root: path,
                    force: false,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                if report.status == GenerationStatus::UpToDate {
                    println!(
                        "current generation {} is already up to date for snapshot {}",
                        report.generation, report.snapshot
                    );
                } else {
                    println!(
                        "published generation {} from snapshot {}",
                        report.generation, report.snapshot
                    );
                    println!(
                        "wrote generated outputs to {}",
                        report.generation_dir.display()
                    );
                    println!(
                        "updated current pointer at {}",
                        report.current_pointer.display()
                    );
                }
                println!(
                    "generation timings: total={}ms snapshot={}ms jsonl={}ms wiki={}ms html={}ms publish={}ms",
                    report.metrics.total_ms,
                    report.metrics.snapshot_load_ms,
                    report.metrics.jsonl_ms,
                    report.metrics.wiki_ms,
                    report.metrics.html_ms,
                    report.metrics.publish_ms
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shared_generation_commands_with_json() {
        assert!(matches!(
            parse(&["wiki".to_string(), "--json".to_string()]).unwrap(),
            Some(Command::Wiki { json: true, .. })
        ));
        assert!(matches!(
            parse(&[
                "report".to_string(),
                "html".to_string(),
                "--json".to_string(),
            ])
            .unwrap(),
            Some(Command::Html { json: true, .. })
        ));
        assert!(matches!(
            parse(&["generate".to_string(), "--json".to_string()]).unwrap(),
            Some(Command::Generate { json: true, .. })
        ));
    }

    #[test]
    fn ignores_unrelated_report_commands() {
        assert!(
            parse(&["report".to_string(), "unknown".to_string()])
                .unwrap()
                .is_none()
        );
    }
}
