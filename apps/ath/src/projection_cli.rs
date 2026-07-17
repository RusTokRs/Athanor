use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{HtmlReportOptions, WikiOptions, project_html_report, project_wiki};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct ProjectionCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Wiki {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ReportCommand {
    Html {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if !matches!(args.first().map(String::as_str), Some("wiki" | "report")) {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match ProjectionCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print projection help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Wiki { path, output } => {
            let report = project_wiki(WikiOptions { root: path, output }).await?;
            println!(
                "projected {} entities and {} open diagnostics from snapshot {}",
                report.entities, report.open_diagnostics, report.snapshot
            );
            println!("wrote Markdown wiki to {}", report.output_dir.display());
        }
        Command::Report {
            command: ReportCommand::Html { path, output },
        } => {
            let report = project_html_report(HtmlReportOptions { root: path, output }).await?;
            println!(
                "projected {} entities and {} open diagnostics from snapshot {}",
                report.entities, report.open_diagnostics, report.snapshot
            );
            println!("wrote HTML report to {}", report.output_dir.display());
        }
    }
    Ok(())
}
