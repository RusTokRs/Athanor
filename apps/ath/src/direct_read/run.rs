use anyhow::Result;
use athanor_app::{
    ChangeMapOptions, ContextLimitOverrides, ContextOptions, ExplainOptions, ImpactOptions,
    OverviewOptions, SearchOptions,
};

use super::model::Command;
use super::operation::{await_cli_operation, operation};
use super::render::{print_change_map, print_explanation, print_impact_analysis, print_overview};

pub(crate) async fn run(command: Command) -> Result<()> {
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
                print_explanation(&report)?;
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
                print_overview(&report)?;
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
                print_impact_analysis(&report)?;
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
                print_change_map(&report)?;
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
