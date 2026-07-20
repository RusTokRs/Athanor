use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    AdapterTrustListOptions, AdapterTrustOptions, VersionedAdapterTrustReport,
    default_adapter_trust_path, list_adapter_plugin_trust_versioned,
    trust_adapter_plugin_versioned, untrust_adapter_plugin_versioned,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectPluginCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    /// Inspect and manage trusted adapter plugin manifests.
    Plugins {
        #[command(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    /// List discovered adapter plugin manifests and trust status.
    List {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Override the user-level adapter trust store path.
        #[arg(long)]
        trust_store: Option<PathBuf>,
        /// Print the trust report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Trust one adapter plugin manifest by path and current content hash.
    Trust {
        /// Path to an adapter manifest.
        manifest: PathBuf,
        /// Override the user-level adapter trust store path.
        #[arg(long)]
        trust_store: Option<PathBuf>,
        /// Print the trust report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove trust for one adapter plugin manifest by path.
    Untrust {
        /// Path to an adapter manifest.
        manifest: PathBuf,
        /// Override the user-level adapter trust store path.
        #[arg(long)]
        trust_store: Option<PathBuf>,
        /// Print the trust report as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) enum Command {
    List {
        path: PathBuf,
        trust_store: Option<PathBuf>,
        json: bool,
    },
    Trust {
        manifest: PathBuf,
        trust_store: Option<PathBuf>,
        json: bool,
    },
    Untrust {
        manifest: PathBuf,
        trust_store: Option<PathBuf>,
        json: bool,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("plugins") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectPluginCli::try_parse_from(argv) {
        Ok(DirectPluginCli {
            command:
                RootCommand::Plugins {
                    command:
                        PluginCommand::List {
                            path,
                            trust_store,
                            json,
                        },
                },
        }) => Ok(Some(Command::List {
            path,
            trust_store,
            json,
        })),
        Ok(DirectPluginCli {
            command:
                RootCommand::Plugins {
                    command:
                        PluginCommand::Trust {
                            manifest,
                            trust_store,
                            json,
                        },
                },
        }) => Ok(Some(Command::Trust {
            manifest,
            trust_store,
            json,
        })),
        Ok(DirectPluginCli {
            command:
                RootCommand::Plugins {
                    command:
                        PluginCommand::Untrust {
                            manifest,
                            trust_store,
                            json,
                        },
                },
        }) => Ok(Some(Command::Untrust {
            manifest,
            trust_store,
            json,
        })),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error
                .print()
                .context("failed to print direct plugin help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn run(command: Command) -> Result<()> {
    let (report, json) = match command {
        Command::List {
            path,
            trust_store,
            json,
        } => (
            list_adapter_plugin_trust_versioned(AdapterTrustListOptions {
                root: path,
                trust_path: trust_path(trust_store)?,
            })?,
            json,
        ),
        Command::Trust {
            manifest,
            trust_store,
            json,
        } => (
            trust_adapter_plugin_versioned(AdapterTrustOptions {
                trust_path: trust_path(trust_store)?,
                manifest_path: manifest,
            })?,
            json,
        ),
        Command::Untrust {
            manifest,
            trust_store,
            json,
        } => (
            untrust_adapter_plugin_versioned(AdapterTrustOptions {
                trust_path: trust_path(trust_store)?,
                manifest_path: manifest,
            })?,
            json,
        ),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}

fn trust_path(path: Option<PathBuf>) -> Result<PathBuf> {
    path.map_or_else(default_adapter_trust_path, Ok)
}

fn print_report(report: &VersionedAdapterTrustReport) {
    println!(
        "Adapter plugin trust at {}: {}",
        report.trust_path.display(),
        report.plugins.len()
    );
    if report.plugins.is_empty() {
        println!("  (none)");
        return;
    }
    for plugin in &report.plugins {
        let trust = if plugin.trusted {
            "trusted"
        } else {
            "untrusted"
        };
        let external = if plugin.has_external_process {
            "external-process"
        } else {
            "in-process"
        };
        println!(
            "  {} [{}; {}] -> {}",
            plugin.name,
            trust,
            external,
            plugin.manifest_path.display()
        );
        println!("    hash: {}", plugin.content_hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plugin_commands_with_json() {
        assert!(matches!(
            parse(&[
                "plugins".to_string(),
                "list".to_string(),
                ".".to_string(),
                "--json".to_string(),
            ])
            .unwrap(),
            Some(Command::List { json: true, .. })
        ));
        assert!(matches!(
            parse(&[
                "plugins".to_string(),
                "trust".to_string(),
                "plugin.json".to_string(),
            ])
            .unwrap(),
            Some(Command::Trust { json: false, .. })
        ));
        assert!(parse(&["overview".to_string()]).unwrap().is_none());
    }
}
