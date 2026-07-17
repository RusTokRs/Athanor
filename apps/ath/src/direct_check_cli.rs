use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_app::{
    AffectedCheckOptions, ApiDiffOptions, ApiRetentionOverrides, DiagnosticCheckOptions,
    DiagnosticScope,
};
use athanor_core::OperationContextCancellation;
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

use crate::direct_operation::{
    await_drained_operation, await_operation, operation, run_blocking_operation,
};
use crate::legacy;

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectCheckCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Check {
        #[arg(value_enum)]
        scope: DiagnosticScopeArg,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DiagnosticScopeArg {
    Affected,
    Api,
    Docs,
    Env,
    Scripts,
    Deployment,
    Runbooks,
    #[value(name = "rustok-ffa")]
    RustokFfa,
    #[value(name = "rustok-fba")]
    RustokFba,
    #[value(name = "rustok-page-builder")]
    RustokPageBuilder,
}

impl DiagnosticScopeArg {
    fn diagnostic_scope(self) -> Option<DiagnosticScope> {
        match self {
            Self::Affected => None,
            Self::Api => Some(DiagnosticScope::Api),
            Self::Docs => Some(DiagnosticScope::Docs),
            Self::Env => Some(DiagnosticScope::Env),
            Self::Scripts => Some(DiagnosticScope::Scripts),
            Self::Deployment => Some(DiagnosticScope::Deployment),
            Self::Runbooks => Some(DiagnosticScope::Runbooks),
            Self::RustokFfa => Some(DiagnosticScope::RustokFfa),
            Self::RustokFba => Some(DiagnosticScope::RustokFba),
            Self::RustokPageBuilder => Some(DiagnosticScope::RustokPageBuilder),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Command {
    scope: DiagnosticScopeArg,
    path: PathBuf,
    json: bool,
    strict: bool,
    deadline_unix_ms: Option<u64>,
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    if args.first().map(String::as_str) != Some("check") {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectCheckCli::try_parse_from(argv) {
        Ok(DirectCheckCli {
            command:
                RootCommand::Check {
                    scope,
                    path,
                    json,
                    strict,
                    deadline_unix_ms,
                },
        }) => Ok(Some(Command {
            scope,
            path,
            json,
            strict,
            deadline_unix_ms,
        })),
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print direct check help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();
    let (operation, cancellation) = operation("check", command.deadline_unix_ms)?;

    if matches!(command.scope, DiagnosticScopeArg::Affected) {
        if command.strict {
            bail!("--strict is currently supported only for `ath check api`");
        }
        let report = await_operation(
            &operation,
            cancellation,
            athanor_app::check_affected_with_composition(
                AffectedCheckOptions {
                    root: command.path,
                },
                &composition,
            ),
        )
        .await?;
        if command.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            legacy::print_affected_check_bridge(&report)?;
        }
        if report.counts.total > 0 {
            bail!(
                "affected check failed with {} open diagnostics",
                report.counts.total
            );
        }
        return Ok(());
    }

    operation.check_active().map_err(anyhow::Error::new)?;
    let scope = command
        .scope
        .diagnostic_scope()
        .expect("non-affected diagnostic scope expected");
    let config = athanor_app::config::load_config(&command.path)?;
    operation.check_active().map_err(anyhow::Error::new)?;
    let is_strict = command.strict || (scope == DiagnosticScope::Api && config.api.strict);
    let cancellation_lease = cancellation.clone();
    let report = await_operation(
        &operation,
        cancellation,
        athanor_app::check_project_with_composition(
            DiagnosticCheckOptions {
                root: command.path.clone(),
                scope,
            },
            &composition,
        ),
    )
    .await?;

    if is_strict {
        if scope != DiagnosticScope::Api {
            bail!("--strict is currently supported only for `ath check api`");
        }
        let operation_for_worker = operation.clone();
        let path = command.path;
        let diff = await_drained_operation(
            cancellation_lease,
            run_blocking_operation(operation_for_worker, move || {
                athanor_app::diff_api_contracts(ApiDiffOptions {
                    root: path,
                    from: None,
                    to: None,
                    retention: ApiRetentionOverrides {
                        auto_cleanup: Some(false),
                        ..ApiRetentionOverrides::default()
                    },
                })
            }),
        )
        .await?;
        if command.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema": "athanor.api_strict_check.v1",
                    "diagnostics": report,
                    "contract": diff,
                }))?
            );
        } else {
            legacy::print_check_bridge(&report)?;
            legacy::print_api_contract_diff_bridge(&diff)?;
        }
        if report.counts.total > 0 || diff.breaking_changes > 0 {
            bail!(
                "strict API check failed with {} open diagnostics and {} breaking changes",
                report.counts.total,
                diff.breaking_changes
            );
        }
    } else if command.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        legacy::print_check_bridge(&report)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_accepts_deadline_and_preserves_scope() {
        let command = parse(&[
            "check".to_string(),
            "api".to_string(),
            "--strict".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused check command");

        assert!(matches!(command.scope, DiagnosticScopeArg::Api));
        assert!(command.strict);
        assert_eq!(command.deadline_unix_ms, Some(42));
    }
}
