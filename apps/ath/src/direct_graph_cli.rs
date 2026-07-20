use std::path::PathBuf;

use anyhow::{Context, Result};
use athanor_app::{
    GraphCyclesOptions, GraphExportOptions, GraphHubsOptions, GraphPageRankOptions,
    GraphPathOptions, GraphRelatedOptions,
};
use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

use crate::direct_operation::{await_drained_operation, operation};
use crate::render::graph;

#[derive(Debug, Parser)]
#[command(name = "ath", disable_version_flag = true)]
struct DirectGraphCli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    Graph {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Export {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = GraphExportFormatArg::Json)]
        format: GraphExportFormatArg,
        #[arg(long, default_value_t = 500)]
        max_entities: usize,
        #[arg(long, default_value_t = 2_000)]
        max_relations: usize,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Related {
        stable_key: String,
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 1)]
        depth: usize,
        #[arg(long, default_value_t = 50)]
        max_entities: usize,
        #[arg(long, default_value_t = 100)]
        max_relations: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Path {
        from_stable_key: String,
        to_stable_key: String,
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 6)]
        max_depth: usize,
        #[arg(long, default_value_t = 10_000)]
        max_visited: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Hubs {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 20)]
        max_relation_ids: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Pagerank {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 0.85)]
        damping: f64,
        #[arg(long, default_value_t = 100)]
        max_iterations: usize,
        #[arg(long, default_value_t = 1e-8)]
        tolerance: f64,
        #[arg(long, default_value_t = 20)]
        max_relation_ids: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
    Cycles {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, default_value_t = 8)]
        max_depth: usize,
        #[arg(long, default_value_t = 1_000)]
        max_starts: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deadline_unix_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum GraphExportFormatArg {
    Json,
    Graphml,
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    let [first, second, ..] = args else {
        return Ok(None);
    };
    if first != "graph"
        || !matches!(
            second.as_str(),
            "export" | "related" | "path" | "hubs" | "pagerank" | "cycles"
        )
    {
        return Ok(None);
    }
    let argv = std::iter::once("ath".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    match DirectGraphCli::try_parse_from(argv) {
        Ok(DirectGraphCli {
            command: RootCommand::Graph { command },
        }) => Ok(Some(command)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().context("failed to print direct graph help")?;
            std::process::exit(0);
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();

    match command {
        Command::Export {
            path,
            format,
            max_entities,
            max_relations,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("graph-export", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                athanor_app::export_graph_with_composition_and_operation_context(
                    GraphExportOptions {
                        root: path,
                        max_entities,
                        max_relations,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            match format {
                GraphExportFormatArg::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                GraphExportFormatArg::Graphml => {
                    print!("{}", athanor_app::graph_export_to_graphml(&report));
                }
            }
        }
        Command::Related {
            stable_key,
            path,
            depth,
            max_entities,
            max_relations,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("graph-related", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                athanor_app::related_graph_with_composition_and_operation_context(
                    GraphRelatedOptions {
                        root: path,
                        stable_key,
                        depth,
                        max_entities,
                        max_relations,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                graph::print_related(&report);
            }
        }
        Command::Path {
            from_stable_key,
            to_stable_key,
            path,
            max_depth,
            max_visited,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("graph-path", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                athanor_app::shortest_graph_path_with_composition_and_operation_context(
                    GraphPathOptions {
                        root: path,
                        from_stable_key,
                        to_stable_key,
                        max_depth,
                        max_visited,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                graph::print_path(&report);
            }
        }
        Command::Hubs {
            path,
            limit,
            kind,
            max_relation_ids,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("graph-hubs", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                athanor_app::graph_hubs_with_composition_and_operation_context(
                    GraphHubsOptions {
                        root: path,
                        limit,
                        kind,
                        max_relation_ids,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                graph::print_hubs(&report);
            }
        }
        Command::Pagerank {
            path,
            limit,
            kind,
            damping,
            max_iterations,
            tolerance,
            max_relation_ids,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("graph-pagerank", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                athanor_app::graph_pagerank_with_composition_and_operation_context(
                    GraphPageRankOptions {
                        root: path,
                        limit,
                        kind,
                        damping,
                        max_iterations,
                        tolerance,
                        max_relation_ids,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                graph::print_pagerank(&report);
            }
        }
        Command::Cycles {
            path,
            limit,
            max_depth,
            max_starts,
            json,
            deadline_unix_ms,
        } => {
            let (operation, cancellation) = operation("graph-cycles", deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                athanor_app::graph_cycles_with_composition_and_operation_context(
                    GraphCyclesOptions {
                        root: path,
                        limit,
                        max_depth,
                        max_starts,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                graph::print_cycles(&report);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_intercepts_only_standard_graph_reads() {
        let command = parse(&[
            "graph".to_string(),
            "pagerank".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused graph command");
        assert!(matches!(
            command,
            Command::Pagerank {
                deadline_unix_ms: Some(42),
                ..
            }
        ));

        assert!(
            parse(&[
                "graph".to_string(),
                "ffa".to_string(),
                "violations".to_string(),
            ])
            .unwrap()
            .is_none()
        );
    }
}
