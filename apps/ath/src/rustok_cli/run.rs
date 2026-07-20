use anyhow::Result;
use athanor_app::{
    GraphFbaDependenciesOptions, GraphFbaModuleOptions, GraphFbaPortOptions,
    GraphFbaViolationsOptions, GraphFfaSurfaceOptions, GraphFfaViolationsOptions,
    GraphPageBuilderConsumerOptions, GraphPageBuilderProviderOptions,
    GraphPageBuilderViolationsOptions, RustokArchitectureContextOptions, RustokFbaAuditOptions,
    RustokFfaAuditOptions, RustokPageBuilderAuditOptions,
    graph_fba_dependencies_with_composition_and_operation_context,
    graph_fba_module_with_composition_and_operation_context,
    graph_fba_port_with_composition_and_operation_context,
    graph_fba_violations_with_composition_and_operation_context,
    graph_ffa_surface_with_composition_and_operation_context,
    graph_ffa_violations_with_composition_and_operation_context,
    graph_page_builder_consumer_with_composition_and_operation_context,
    graph_page_builder_provider_with_composition_and_operation_context,
    graph_page_builder_violations_with_composition_and_operation_context,
    rustok_architecture_context_with_composition_and_operation_context,
    rustok_fba_audit_with_composition_and_operation_context,
    rustok_ffa_audit_with_composition_and_operation_context,
    rustok_page_builder_audit_with_composition_and_operation_context,
};

use super::model::Command;
use crate::direct_operation::{await_drained_operation, operation};
use crate::render::rustok;

pub(crate) async fn run(command: Command) -> Result<()> {
    let composition = athanor_runtime_defaults::production();

    match command {
        Command::ArchitectureContext { intent, flags } => {
            let (operation, cancellation) =
                operation("rustok-architecture-context", flags.deadline_unix_ms)?;
            let mut options =
                RustokArchitectureContextOptions::bounded(flags.path, intent, flags.module);
            options.max_modules = flags.max_modules;
            options.max_contracts = flags.max_contracts;
            options.max_interactions = flags.max_interactions;
            options.max_evidence = flags.max_evidence;
            let report = await_drained_operation(
                cancellation,
                rustok_architecture_context_with_composition_and_operation_context(
                    options,
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                rustok::print_architecture_context(&report);
            }
        }
        Command::FfaAudit(flags) => {
            let (operation, cancellation) = operation("rustok-ffa-audit", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                rustok_ffa_audit_with_composition_and_operation_context(
                    RustokFfaAuditOptions { root: flags.path },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                rustok::print_ffa_audit(&report);
            }
        }
        Command::FbaAudit(flags) => {
            let (operation, cancellation) = operation("rustok-fba-audit", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                rustok_fba_audit_with_composition_and_operation_context(
                    RustokFbaAuditOptions { root: flags.path },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                rustok::print_fba_audit(&report);
            }
        }
        Command::PageBuilderAudit(flags) => {
            let (operation, cancellation) =
                operation("rustok-page-builder-audit", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                rustok_page_builder_audit_with_composition_and_operation_context(
                    RustokPageBuilderAuditOptions { root: flags.path },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            if flags.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                rustok::print_page_builder_audit(&report);
            }
        }
        Command::FfaSurface {
            module,
            surface,
            flags,
        } => {
            let (operation, cancellation) = operation("graph-ffa-surface", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_ffa_surface_with_composition_and_operation_context(
                    GraphFfaSurfaceOptions {
                        root: flags.path,
                        module,
                        surface,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_ffa_graph(&report, flags.json)?;
        }
        Command::FfaViolations(flags) => {
            let (operation, cancellation) =
                operation("graph-ffa-violations", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_ffa_violations_with_composition_and_operation_context(
                    GraphFfaViolationsOptions {
                        root: flags.path,
                        module: flags.module,
                        surface: flags.surface,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_ffa_graph(&report, flags.json)?;
        }
        Command::FbaModule { module, flags } => {
            let (operation, cancellation) = operation("graph-fba-module", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_fba_module_with_composition_and_operation_context(
                    GraphFbaModuleOptions {
                        root: flags.path,
                        module,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_fba_graph(&report, flags.json)?;
        }
        Command::FbaPort {
            module,
            port,
            flags,
        } => {
            let (operation, cancellation) = operation("graph-fba-port", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_fba_port_with_composition_and_operation_context(
                    GraphFbaPortOptions {
                        root: flags.path,
                        module,
                        port,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_fba_graph(&report, flags.json)?;
        }
        Command::FbaDependencies(flags) => {
            let module = flags.module.clone().ok_or_else(|| {
                anyhow::anyhow!("graph fba dependencies requires --module <module>")
            })?;
            let (operation, cancellation) =
                operation("graph-fba-dependencies", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_fba_dependencies_with_composition_and_operation_context(
                    GraphFbaDependenciesOptions {
                        root: flags.path,
                        module,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_fba_graph(&report, flags.json)?;
        }
        Command::FbaViolations(flags) => {
            let (operation, cancellation) =
                operation("graph-fba-violations", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_fba_violations_with_composition_and_operation_context(
                    GraphFbaViolationsOptions {
                        root: flags.path,
                        module: flags.module,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_fba_graph(&report, flags.json)?;
        }
        Command::PageBuilderProvider(flags) => {
            let (operation, cancellation) =
                operation("graph-page-builder-provider", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_page_builder_provider_with_composition_and_operation_context(
                    GraphPageBuilderProviderOptions {
                        root: flags.path,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_page_builder_graph(&report, flags.json)?;
        }
        Command::PageBuilderConsumer { module, flags } => {
            let (operation, cancellation) =
                operation("graph-page-builder-consumer", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_page_builder_consumer_with_composition_and_operation_context(
                    GraphPageBuilderConsumerOptions {
                        root: flags.path,
                        module,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_page_builder_graph(&report, flags.json)?;
        }
        Command::PageBuilderViolations(flags) => {
            let (operation, cancellation) =
                operation("graph-page-builder-violations", flags.deadline_unix_ms)?;
            let report = await_drained_operation(
                cancellation,
                graph_page_builder_violations_with_composition_and_operation_context(
                    GraphPageBuilderViolationsOptions {
                        root: flags.path,
                        module: flags.module,
                        max_nodes: flags.max_nodes,
                        max_edges: flags.max_edges,
                    },
                    &composition,
                    &operation,
                ),
            )
            .await?;
            render_page_builder_graph(&report, flags.json)?;
        }
    }
    Ok(())
}

fn render_ffa_graph(report: &athanor_app::RustokFfaGraph, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        rustok::print_ffa_graph(report);
    }
    Ok(())
}

fn render_fba_graph(report: &athanor_app::RustokFbaGraph, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        rustok::print_fba_graph(report);
    }
    Ok(())
}

fn render_page_builder_graph(
    report: &athanor_app::RustokPageBuilderGraph,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        rustok::print_page_builder_graph(report);
    }
    Ok(())
}
