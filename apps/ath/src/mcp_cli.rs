use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct McpCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Mcp {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("mcp") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match McpCli::try_parse_from(argv) {
        Ok(cli) => Ok(Some(cli.command)),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print MCP help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Mcp { path } => {
            // The transport still delegates tool execution to its legacy dispatcher.
            // Keep the bootstrap local to the dedicated MCP process until MCP-005
            // threads RuntimeComposition through every tool call.
            athanor_runtime_defaults::install();
            athanor_transport_mcp::run_mcp_server(path).await?;
        }
    }
    Ok(())
}
