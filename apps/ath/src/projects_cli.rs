use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    ProjectRegisterOptions, ProjectRegistration, ProjectRegistryOptions, ProjectRegistryReport,
    ProjectUnregisterOptions, default_project_registry_path, list_registered_projects,
    register_project, resolve_registered_project, unregister_project,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct ProjectsCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Projects {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    List {
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Add {
        project_id: String,
        path: PathBuf,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Remove {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Resolve {
        project_id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("projects") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match ProjectsCli::try_parse_from(argv) {
        Ok(ProjectsCli {
            command: RootCommand::Projects { command },
        }) => Ok(Some(command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().context("failed to print projects help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn run(command: Command) -> Result<()> {
    match command {
        Command::List { registry, json } => {
            let report = list_registered_projects(ProjectRegistryOptions {
                registry_path: registry_path(registry)?,
            })?;
            render_registry(&report, json)?;
        }
        Command::Add {
            project_id,
            path,
            registry,
            json,
        } => {
            let report = register_project(ProjectRegisterOptions {
                registry_path: registry_path(registry)?,
                project_id,
                root: path,
            })?;
            render_registry(&report, json)?;
        }
        Command::Remove {
            project_id,
            registry,
            json,
        } => {
            let report = unregister_project(ProjectUnregisterOptions {
                registry_path: registry_path(registry)?,
                project_id,
            })?;
            render_registry(&report, json)?;
        }
        Command::Resolve {
            project_id,
            registry,
            json,
        } => {
            let report = resolve_registered_project(
                ProjectRegistryOptions {
                    registry_path: registry_path(registry)?,
                },
                &project_id,
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Resolved from {}", report.registry_path.display());
                print_registration(&report.project);
            }
        }
    }
    Ok(())
}

fn registry_path(path: Option<PathBuf>) -> Result<PathBuf> {
    path.map_or_else(default_project_registry_path, Ok)
}

fn render_registry(report: &ProjectRegistryReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!(
            "Registered projects at {}: {}",
            report.registry_path.display(),
            report.projects.len()
        );
        if report.projects.is_empty() {
            println!("  (none)");
        } else {
            for project in &report.projects {
                print_registration(project);
            }
        }
    }
    Ok(())
}

fn print_registration(project: &ProjectRegistration) {
    println!("  {} -> {}", project.project_id, project.root.display());
}
