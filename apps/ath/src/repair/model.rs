use std::path::PathBuf;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Help(HelpTopic),
    IndexRetention {
        path: PathBuf,
        dry_run: bool,
        keep: usize,
        confirmation_token: Option<String>,
        json: bool,
    },
    RecoverIndex {
        path: PathBuf,
        dry_run: bool,
        json: bool,
    },
    RecoverIndexCleanup {
        path: PathBuf,
        dry_run: bool,
        json: bool,
    },
    RepairLatest {
        path: PathBuf,
        dry_run: bool,
        snapshot: Option<String>,
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpTopic {
    Repair,
    IndexRetention,
    RecoverIndex,
    RecoverIndexCleanup,
    RepairLatest,
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    let Some(first) = args.first() else {
        return Ok(None);
    };
    if first != "repair" {
        return Ok(None);
    }
    let Some(subcommand) = args.get(1) else {
        return Ok(None);
    };
    match subcommand.as_str() {
        "--help" | "-h" => Ok(Some(Command::Help(HelpTopic::Repair))),
        "index-retention" | "cleanup-index" => parse_index_retention(&args[2..]).map(Some),
        "recover-index" => parse_recover_index(&args[2..]).map(Some),
        "recover-index-cleanup" => parse_recover_index_cleanup(&args[2..]).map(Some),
        "repair-latest" => parse_repair_latest(&args[2..]).map(Some),
        _ => Ok(None),
    }
}

fn parse_index_retention(args: &[String]) -> Result<Command> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Command::Help(HelpTopic::IndexRetention));
    }
    let mut path = None;
    let mut dry_run = false;
    let mut keep = 0;
    let mut confirmation_token = None;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                dry_run = true;
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--keep" => {
                let value = args
                    .get(index + 1)
                    .context("--keep requires a non-negative integer")?;
                keep = value
                    .parse::<usize>()
                    .context("--keep must be a non-negative integer")?;
                index += 2;
            }
            "--confirmation-token" => {
                confirmation_token = Some(
                    args.get(index + 1)
                        .context("--confirmation-token requires a value")?
                        .clone(),
                );
                index += 2;
            }
            value if value.starts_with('-') => {
                bail!("unknown index-retention option `{value}`");
            }
            value => {
                if path.replace(PathBuf::from(value)).is_some() {
                    bail!("index-retention accepts at most one project path");
                }
                index += 1;
            }
        }
    }
    if dry_run && confirmation_token.is_some() {
        bail!("--dry-run conflicts with --confirmation-token");
    }
    Ok(Command::IndexRetention {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        dry_run,
        keep,
        confirmation_token,
        json,
    })
}

fn parse_recover_index(args: &[String]) -> Result<Command> {
    let (path, dry_run, json, help) = parse_recovery_flags(args, "recover-index")?;
    if help {
        return Ok(Command::Help(HelpTopic::RecoverIndex));
    }
    Ok(Command::RecoverIndex {
        path,
        dry_run,
        json,
    })
}

fn parse_recover_index_cleanup(args: &[String]) -> Result<Command> {
    let (path, dry_run, json, help) = parse_recovery_flags(args, "recover-index-cleanup")?;
    if help {
        return Ok(Command::Help(HelpTopic::RecoverIndexCleanup));
    }
    Ok(Command::RecoverIndexCleanup {
        path,
        dry_run,
        json,
    })
}

fn parse_repair_latest(args: &[String]) -> Result<Command> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Command::Help(HelpTopic::RepairLatest));
    }
    let mut path = None;
    let mut dry_run = false;
    let mut snapshot = None;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                dry_run = true;
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--snapshot" => {
                snapshot = Some(
                    args.get(index + 1)
                        .context("--snapshot requires a value")?
                        .clone(),
                );
                index += 2;
            }
            value if value.starts_with('-') => bail!("unknown repair-latest option `{value}`"),
            value => {
                if path.replace(PathBuf::from(value)).is_some() {
                    bail!("repair-latest accepts at most one project path");
                }
                index += 1;
            }
        }
    }
    Ok(Command::RepairLatest {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        dry_run,
        snapshot,
        json,
    })
}

fn parse_recovery_flags(
    args: &[String],
    command: &str,
) -> Result<(PathBuf, bool, bool, bool)> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok((PathBuf::from("."), false, false, true));
    }
    let mut path = None;
    let mut dry_run = false;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--json" => json = true,
            value if value.starts_with('-') => bail!("unknown {command} option `{value}`"),
            value => {
                if path.replace(PathBuf::from(value)).is_some() {
                    bail!("{command} accepts at most one project path");
                }
            }
        }
    }
    Ok((
        path.unwrap_or_else(|| PathBuf::from(".")),
        dry_run,
        json,
        false,
    ))
}
