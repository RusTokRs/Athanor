use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

use athanor_app::{ChangedValidationOptions, validate_changed_with_composition};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectValidateChangedCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Validate changed source files through extractors without writing a snapshot.
    ValidateChanged {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Validate a specific source file. Repeat to validate multiple files instead of Git changes.
        #[arg(long = "file")]
        files: Vec<PathBuf>,
        /// Print the complete changed-file validation report as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("validate-changed") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectValidateChangedCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error
                .print()
                .context("failed to print direct validate-changed help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();

    match command {
        Command::ValidateChanged { path, files, json } => {
            let report = validate_changed_with_composition(
                ChangedValidationOptions { root: path, files },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "validated {} changed files through extractors using snapshot {}",
                    report.files_checked, report.snapshot
                );
                println!(
                    "affected files: {} changed, {} removed",
                    report.changed_files, report.removed_files
                );
                println!(
                    "diagnostics: {}, metrics: total {} ms, discovery {} ms, extraction {} ms",
                    report.diagnostics.len(),
                    report.metrics.total_ms,
                    report.metrics.source_discovery_ms,
                    report.metrics.extraction_ms
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
    fn parser_preserves_repeated_files_and_json() {
        let command = parse(&[
            "validate-changed".to_string(),
            "--path".to_string(),
            "repo".to_string(),
            "--file".to_string(),
            "src/lib.rs".to_string(),
            "--file".to_string(),
            "src/main.rs".to_string(),
            "--json".to_string(),
        ])
        .unwrap()
        .expect("focused validate-changed command");

        assert!(matches!(
            command,
            Command::ValidateChanged {
                path,
                files,
                json: true,
            } if path == PathBuf::from("repo") && files.len() == 2
        ));
    }
}
