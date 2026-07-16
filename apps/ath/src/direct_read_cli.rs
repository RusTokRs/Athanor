use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_app::{
    ChangeMapOptions, ContextLimitOverrides, ContextOptions, ExplainOptions, ImpactOptions,
    OverviewOptions, SearchOptions,
};
use athanor_core::{CancellationHandle, OperationContext, OperationContextCancellation};
use athanor_domain::ContextLevel;
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

use crate::legacy;

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
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            error.print().context("failed to print direct read help")?;
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
            let pack = await_cli_operation(
                &operation,
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
        Command::Explain {
            stable_key,
            path,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("explain", deadline_unix_ms)?;
            let report = await_cli_operation(
                &operation,
                cancellation,
                athanor_app::explain_project_with_composition(
                    ExplainOptions {
                        root: path,
                        stable_key,
                    },
                    &composition,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                legacy::print_explanation_bridge(&report)?;
            }
        }
        Command::Overview {
            path,
            json,
            top,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("overview", deadline_unix_ms)?;
            let report = await_cli_operation(
                &operation,
                cancellation,
                athanor_app::overview_project_with_composition(
                    OverviewOptions { root: path, top },
                    &composition,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                legacy::print_overview_bridge(&report)?;
            }
        }
        Command::Impact {
            target,
            path,
            json,
            diff,
            max_depth,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("impact", deadline_unix_ms)?;
            let report = await_cli_operation(
                &operation,
                cancellation,
                athanor_app::impact_project_with_composition(
                    ImpactOptions {
                        root: path,
                        target,
                        diff,
                        max_depth,
                    },
                    &composition,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                legacy::print_impact_bridge(&report)?;
            }
        }
        Command::ChangeMap {
            task,
            target,
            diff,
            path,
            max_entities,
            max_files,
            max_diagnostics,
            max_depth,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("change-map", deadline_unix_ms)?;
            let report = await_cli_operation(
                &operation,
                cancellation,
                athanor_app::change_map_project_with_composition_and_operation_context(
                    ChangeMapOptions {
                        root: path,
                        task,
                        target,
                        diff,
                        max_entities,
                        max_files,
                        max_diagnostics,
                        max_depth,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                legacy::print_change_map_bridge(&report)?;
            }
        }
        Command::Search {
            query,
            path,
            limit,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("search", deadline_unix_ms)?;
            let report = await_cli_operation(
                &operation,
                cancellation,
                athanor_app::search_project_with_composition(
                    SearchOptions {
                        root: path,
                        query,
                        limit,
                    },
                    &composition,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "search results for query \"{}\" in snapshot {} ({} of limit {}):",
                    report.query, report.snapshot, report.returned, report.limit
                );
                if report.results.is_empty() {
                    println!("No results found.");
                } else {
                    for item in &report.results {
                        println!(
                            "[{:.4}] {} ({}) - {}",
                            item.score, item.name, item.kind, item.stable_key
                        );
                        println!("  entity: {}", item.entity_id.0);
                        if let Some(source) = &item.source {
                            println!("  source: {}", source.path);
                        }
                    }
                    if report.truncated {
                        println!(
                            "results truncated by limit; at least {} more result(s) omitted",
                            report.omitted.results_lower_bound
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn operation(
    name: &str,
    deadline_unix_ms: Option<u64>,
) -> Result<(OperationContext, CancellationHandle)> {
    let deadline_unix_ms = match deadline_unix_ms {
        Some(deadline_unix_ms) => Some(deadline_unix_ms),
        None => match std::env::var("ATHANOR_DEADLINE_UNIX_MS") {
            Ok(value) => Some(
                value
                    .parse::<u64>()
                    .context("ATHANOR_DEADLINE_UNIX_MS must be an unsigned integer")?,
            ),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                bail!("ATHANOR_DEADLINE_UNIX_MS must contain valid Unicode")
            }
        },
    };
    let mut operation = OperationContext::new(format!("cli:{name}:{}", std::process::id()));
    if let Some(deadline_unix_ms) = deadline_unix_ms {
        operation = operation.with_deadline_unix_ms(deadline_unix_ms);
    }
    operation.check_active().map_err(anyhow::Error::new)?;
    let cancellation = operation
        .cancellation_handle()
        .map_err(anyhow::Error::new)?;
    Ok((operation, cancellation))
}

async fn await_cli_operation<T>(
    operation: &OperationContext,
    cancellation: CancellationHandle,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::pin!(future);
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    let poll_interval = Duration::from_millis(25);
    loop {
        operation.check_active().map_err(anyhow::Error::new)?;
        let wait = operation
            .remaining()
            .map(|remaining| remaining.min(poll_interval))
            .unwrap_or(poll_interval);
        tokio::select! {
            biased;
            signal = &mut ctrl_c => {
                signal.context("failed to listen for CLI cancellation")?;
                cancellation.cancel();
            }
            result = &mut future => {
                let result = result?;
                operation.check_active().map_err(anyhow::Error::new)?;
                return Ok(result);
            }
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use athanor_core::CoreError;

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

    #[tokio::test]
    async fn expired_deadline_rejects_work_before_polling_future() {
        let operation = OperationContext::new("cli:test-expired").with_deadline_unix_ms(0);
        let cancellation = operation.cancellation_handle().unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let started_in_future = Arc::clone(&started);

        let error = await_cli_operation(&operation, cancellation, async move {
            started_in_future.store(true, Ordering::Release);
            Ok::<_, anyhow::Error>(())
        })
        .await
        .expect_err("expired deadline must fail");

        assert!(!started.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
    }

    #[tokio::test]
    async fn cancellation_rejects_late_success() {
        let operation = OperationContext::new("cli:test-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        let cancel_from_future = cancellation.clone();

        let error = await_cli_operation(&operation, cancellation, async move {
            cancel_from_future.cancel();
            Ok::<_, anyhow::Error>(42_u8)
        })
        .await
        .expect_err("cancelled operation must not return success");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[tokio::test]
    async fn pending_future_is_bounded_by_operation_deadline() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let operation = OperationContext::new("cli:test-timeout")
            .with_deadline_unix_ms(now.saturating_add(50));
        let cancellation = operation.cancellation_handle().unwrap();

        let error = await_cli_operation(
            &operation,
            cancellation,
            pending::<Result<()>>(),
        )
        .await
        .expect_err("deadline must terminate pending read");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
    }
}
