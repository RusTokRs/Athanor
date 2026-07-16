use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{ContextLimitOverrides, ContextOptions};
use athanor_domain::ContextLevel;
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

use crate::direct_operation::{await_drained_operation, operation};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectContextCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Context {
        task: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, value_enum, default_value_t = ContextLevelArg::Normal)]
        level: ContextLevelArg,
        #[arg(long = "budget")]
        max_tokens: Option<usize>,
        #[arg(long)]
        max_files: Option<usize>,
        #[arg(long)]
        max_entities: Option<usize>,
        #[arg(long)]
        max_diagnostics: Option<usize>,
        #[arg(long)]
        max_depth: Option<usize>,
        #[arg(long)]
        diff: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ContextLevelArg {
    Summary,
    Normal,
    Deep,
    Full,
}

impl From<ContextLevelArg> for ContextLevel {
    fn from(value: ContextLevelArg) -> Self {
        match value {
            ContextLevelArg::Summary => Self::Summary,
            ContextLevelArg::Normal => Self::Normal,
            ContextLevelArg::Deep => Self::Deep,
            ContextLevelArg::Full => Self::Full,
        }
    }
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("context") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectContextCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print direct context help")?;
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
        Command::Context {
            task,
            path,
            json,
            level,
            max_tokens,
            max_files,
            max_entities,
            max_diagnostics,
            max_depth,
            diff,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("context", deadline_unix_ms)?;
            let pack = await_drained_operation(
                cancellation,
                athanor_app::context_project_with_composition_and_operation_context(
                    ContextOptions {
                        root: path,
                        task: task.unwrap_or_default(),
                        diff,
                        level: level.into(),
                        limits: ContextLimitOverrides {
                            max_tokens,
                            max_files,
                            max_entities,
                            max_diagnostics,
                            max_depth,
                        },
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&pack)?);
            } else {
                println!("{}", pack.summary);
                for file in &pack.files {
                    println!("file: {file}");
                }
                for scope in &pack.scope {
                    println!("entity: {scope}");
                }
                for diagnostic in &pack.diagnostics {
                    println!("diagnostic: {}", diagnostic.0);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_preserves_context_limits_and_accepts_deadline() {
        let command = parse(&[
            "context".to_string(),
            "authentication".to_string(),
            "--max-entities".to_string(),
            "24".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused context command");

        assert!(matches!(
            command,
            Command::Context {
                max_entities: Some(24),
                deadline_unix_ms: Some(42),
                ..
            }
        ));
    }
}
