use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    IndexGenerationCleanupOptions, IndexGenerationCleanupReport, RepairRecoverIndexOptions,
    RepairRecoverIndexReport, cleanup_index_generations, recover_index_publication,
};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpTopic {
    Repair,
    IndexRetention,
    RecoverIndex,
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
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Command::Help(HelpTopic::RecoverIndex));
    }
    let mut path = None;
    let mut dry_run = false;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--json" => json = true,
            value if value.starts_with('-') => bail!("unknown recover-index option `{value}`"),
            value => {
                if path.replace(PathBuf::from(value)).is_some() {
                    bail!("recover-index accepts at most one project path");
                }
            }
        }
    }
    Ok(Command::RecoverIndex {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        dry_run,
        json,
    })
}

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Help(topic) => {
            print_help(topic);
            Ok(())
        }
        Command::IndexRetention {
            path,
            dry_run,
            keep,
            confirmation_token,
            json,
        } => {
            let report = cleanup_index_generations(IndexGenerationCleanupOptions {
                root: path,
                dry_run,
                keep,
                confirmation_token,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_index_retention(&report);
            }
            Ok(())
        }
        Command::RecoverIndex {
            path,
            dry_run,
            json,
        } => {
            let report = recover_index_publication(RepairRecoverIndexOptions {
                root: path,
                dry_run,
            })
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_recover_index(&report);
            }
            Ok(())
        }
    }
}

fn print_help(topic: HelpTopic) {
    match topic {
        HelpTopic::Repair => {
            println!("Transactional repair commands:");
            println!("  index-retention  Plan or apply immutable index-generation retention");
            println!("  recover-index    Recover a pending transactional index publication");
            println!();
            println!("Existing commands remain available: inspect, cleanup, regenerate, recover-canonical, apply");
        }
        HelpTopic::IndexRetention => {
            println!("Plan or apply immutable index-generation retention");
            println!();
            println!("Usage:");
            println!("  ath repair index-retention [PATH] --dry-run [--keep <N>] [--json]");
            println!("  ath repair index-retention [PATH] --confirmation-token <TOKEN> [--keep <N>] [--json]");
            println!();
            println!("A destructive invocation requires the token emitted by an exact dry-run plan.");
        }
        HelpTopic::RecoverIndex => {
            println!("Recover a pending transactional index publication without running the indexing pipeline");
            println!();
            println!("Usage: ath repair recover-index [PATH] [--dry-run] [--json]");
        }
    }
}

fn print_index_retention(report: &IndexGenerationCleanupReport) {
    let action = if report.dry_run { "planned" } else { "removed" };
    println!("index-generation cleanup at {}", report.root.display());
    println!("  {action}: {}", report.removed.len());
    println!("  retained: {}", report.retained.len());
    if let Some(token) = &report.confirmation_token {
        println!("  confirmation token: {token}");
    }
    for row in &report.removed {
        println!("  {action} {}", row.generation);
    }
    println!("  remaining issues: {}", report.remaining_issues.len());
}

fn print_recover_index(report: &RepairRecoverIndexReport) {
    println!("index publication recovery at {}", report.root.display());
    println!("  needed: {}", report.needed);
    println!("  recovered: {}", report.recovered);
    if let Some(snapshot) = &report.snapshot {
        println!("  snapshot: {snapshot}");
    }
    if let Some(generation) = &report.generation {
        println!("  generation: {generation}");
    }
    println!("  remaining issues: {}", report.remaining_issues.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_exact_retention_plan_and_apply_forms() {
        assert_eq!(
            parse(&args(&[
                "repair",
                "index-retention",
                "project",
                "--dry-run",
                "--keep",
                "2",
                "--json",
            ]))
            .unwrap(),
            Some(Command::IndexRetention {
                path: PathBuf::from("project"),
                dry_run: true,
                keep: 2,
                confirmation_token: None,
                json: true,
            })
        );
        assert_eq!(
            parse(&args(&[
                "repair",
                "index-retention",
                "--confirmation-token",
                "sha256:test",
            ]))
            .unwrap(),
            Some(Command::IndexRetention {
                path: PathBuf::from("."),
                dry_run: false,
                keep: 0,
                confirmation_token: Some("sha256:test".to_string()),
                json: false,
            })
        );
    }

    #[test]
    fn parses_recovery_and_help_without_intercepting_legacy_commands() {
        assert_eq!(
            parse(&args(&["repair", "recover-index", "project", "--dry-run"]))
                .unwrap(),
            Some(Command::RecoverIndex {
                path: PathBuf::from("project"),
                dry_run: true,
                json: false,
            })
        );
        assert_eq!(
            parse(&args(&["repair", "index-retention", "--help"])).unwrap(),
            Some(Command::Help(HelpTopic::IndexRetention))
        );
        assert_eq!(parse(&args(&["repair", "inspect"])).unwrap(), None);
    }

    #[test]
    fn rejects_ambiguous_or_invalid_retention_arguments() {
        assert!(
            parse(&args(&[
                "repair",
                "index-retention",
                "--dry-run",
                "--confirmation-token",
                "sha256:test",
            ]))
            .unwrap_err()
            .to_string()
            .contains("conflicts")
        );
        assert!(
            parse(&args(&["repair", "index-retention", "--keep", "many"]))
                .unwrap_err()
                .to_string()
                .contains("non-negative integer")
        );
    }
}
