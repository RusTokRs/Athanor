use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_domain::ContextLevel;
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectReadCli {
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
    Explain {
        stable_key: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Overview {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 10)]
        top: usize,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Impact {
        target: Option<String>,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        diff: bool,
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    ChangeMap {
        task: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        diff: bool,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 30)]
        max_entities: usize,
        #[arg(long, default_value_t = 20)]
        max_files: usize,
        #[arg(long, default_value_t = 20)]
        max_diagnostics: usize,
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
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
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(None);
    };
    if !matches!(
        command,
        "context" | "explain" | "overview" | "impact" | "change-map" | "search"
    ) {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectReadCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().context("failed to print direct read help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_parser_accepts_deadline_without_changing_legacy_flags() {
        let command = parse(&[
            "search".to_string(),
            "login".to_string(),
            "--limit".to_string(),
            "5".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused read command");
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
