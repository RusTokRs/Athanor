use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    ConfigDoctorReport, ConfigReportOptions, ConfigValidateReport, doctor_project_config,
    validate_project_config,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectConfigCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Config {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Parse athanor.toml and reject unknown or invalid fields.
    Validate {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the effective configuration as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Report the effective configuration and local compatibility checks.
    Doctor {
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete report as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("config") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectConfigCli::try_parse_from(argv) {
        Ok(DirectConfigCli {
            command: RootCommand::Config { command },
        }) => Ok(Some(command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print direct config help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn run(command: Command) -> Result<()> {
    match command {
        Command::Validate { path, json } => {
            let report = validate_project_config(ConfigReportOptions { root: path.clone() })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_validation_report(&path, &report)?;
            }
        }
        Command::Doctor { path, json } => {
            let report = doctor_project_config(ConfigReportOptions { root: path })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_doctor_report(&report);
            }
        }
    }
    Ok(())
}

fn print_validation_report(path: &std::path::Path, report: &ConfigValidateReport) -> Result<()> {
    println!(
        "configuration at {} is valid",
        path.join("athanor.toml").display()
    );
    println!("{}", serde_json::to_string_pretty(&report.config)?);
    Ok(())
}

fn print_doctor_report(report: &ConfigDoctorReport) {
    println!("configuration doctor: {}", report.root.display());
    for (name, label) in [
        ("storage_backend", "storage backend"),
        ("external_process_adapters", "external process adapters"),
        ("external_process_sandbox", "external process sandbox"),
    ] {
        if let Some(check) = report.checks.iter().find(|check| check.name == name) {
            println!("  {label}: {}", check.status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_non_config_commands() {
        assert!(parse(&["overview".to_string()]).unwrap().is_none());
    }

    #[test]
    fn parses_validate_and_doctor() {
        assert!(matches!(
            parse(&["config".to_string(), "validate".to_string()]).unwrap(),
            Some(Command::Validate { .. })
        ));
        assert!(matches!(
            parse(&["config".to_string(), "doctor".to_string()]).unwrap(),
            Some(Command::Doctor { .. })
        ));
    }
}
