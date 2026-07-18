use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStoreOperationExt, OperationContext,
    OperationContextCancellation,
};
use athanor_domain::ContextLevel;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::context::{ContextLimitOverrides, ContextOptions};
use crate::derived_read_operation::context_project_with_composition_and_operation_context;
use crate::graph::{
    GraphFbaDependenciesOptions, GraphFbaModuleOptions, GraphFbaPortOptions,
    GraphFbaViolationsOptions, GraphFfaSurfaceOptions, GraphFfaViolationsOptions,
    GraphPageBuilderConsumerOptions, GraphPageBuilderProviderOptions,
    GraphPageBuilderViolationsOptions, RustokFbaAudit, RustokFbaAuditOptions, RustokFfaAudit,
    RustokFfaAuditOptions, RustokPageBuilderAudit, RustokPageBuilderAuditOptions,
};
use crate::project_path::normalize_canonical_path;
use crate::rustok_architecture::{RustokArchitectureContext, RustokArchitectureContextOptions};
use crate::rustok_architecture_cooperative::build_rustok_architecture_context_with_operation_context;
use crate::rustok_audit_cooperative::{
    build_rustok_fba_audit_with_operation_context, build_rustok_ffa_audit_with_operation_context,
    build_rustok_page_builder_audit_with_operation_context,
};
use crate::rustok_graph_cooperative::{
    build_rustok_fba_dependencies_graph_with_operation_context,
    build_rustok_fba_module_graph_with_operation_context,
    build_rustok_fba_port_graph_with_operation_context,
    build_rustok_fba_violations_graph_with_operation_context,
    build_rustok_ffa_surface_graph_with_operation_context,
    build_rustok_ffa_violations_graph_with_operation_context,
    build_rustok_page_builder_consumer_graph_with_operation_context,
    build_rustok_page_builder_provider_graph_with_operation_context,
    build_rustok_page_builder_violations_graph_with_operation_context,
};
use crate::rustok_json_contract::{
    RustokFbaDependenciesGraphReport, RustokFbaModuleGraphReport, RustokFbaPortGraphReport,
    RustokFbaViolationsGraphReport, RustokFfaSurfaceGraphReport, RustokFfaViolationsGraphReport,
    RustokPageBuilderConsumerGraphReport, RustokPageBuilderProviderGraphReport,
    RustokPageBuilderViolationsGraphReport,
};

pub async fn rustok_architecture_context_with_composition_and_operation_context(
    mut options: RustokArchitectureContextOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokArchitectureContext> {
    if options.intent.trim().is_empty() && options.module.is_none() {
        bail!("Rustok architecture context requires an intent or module");
    }
    validate_architecture_limits(&options)?;
    operation.check_active().map_err(anyhow::Error::new)?;

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let task = match &options.module {
        Some(module) if options.intent.trim().is_empty() => format!("RusTok module {module}"),
        Some(module) => format!("{} RusTok module {module}", options.intent),
        None => options.intent.clone(),
    };
    let context = context_project_with_composition_and_operation_context(
        ContextOptions {
            root: root.clone(),
            task,
            diff: false,
            level: ContextLevel::Normal,
            limits: ContextLimitOverrides {
                max_tokens: Some(12_000),
                max_files: Some(24),
                max_entities: Some(48),
                max_diagnostics: Some(24),
                max_depth: Some(2),
            },
        },
        composition,
        operation,
    )
    .await?;
    let snapshot = load_latest_snapshot(root.clone(), composition, operation).await?;
    options.root = root;
    let context_entities = context.pack.entities;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_architecture_context_with_operation_context(
            &snapshot,
            &options,
            &context_entities,
            &operation_for_worker,
        )
    })
    .await
}

pub async fn rustok_ffa_audit_with_composition_and_operation_context(
    options: RustokFfaAuditOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFfaAudit> {
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_ffa_audit_with_operation_context(&snapshot, &operation_for_worker)
    })
    .await
}

pub async fn rustok_fba_audit_with_composition_and_operation_context(
    options: RustokFbaAuditOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFbaAudit> {
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_fba_audit_with_operation_context(&snapshot, &operation_for_worker)
    })
    .await
}

pub async fn rustok_page_builder_audit_with_composition_and_operation_context(
    options: RustokPageBuilderAuditOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokPageBuilderAudit> {
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_audit_with_operation_context(&snapshot, &operation_for_worker)
    })
    .await
}

pub async fn graph_ffa_surface_with_composition_and_operation_context(
    options: GraphFfaSurfaceOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFfaSurfaceGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FFA")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_ffa_surface_graph_with_operation_context(
            &snapshot,
            &options.module,
            &options.surface,
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokFfaSurfaceGraphReport::new)
}

pub async fn graph_ffa_violations_with_composition_and_operation_context(
    options: GraphFfaViolationsOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFfaViolationsGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FFA")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_ffa_violations_graph_with_operation_context(
            &snapshot,
            options.module.as_deref(),
            options.surface.as_deref(),
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokFfaViolationsGraphReport::new)
}

pub async fn graph_fba_module_with_composition_and_operation_context(
    options: GraphFbaModuleOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFbaModuleGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_fba_module_graph_with_operation_context(
            &snapshot,
            &options.module,
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokFbaModuleGraphReport::new)
}

pub async fn graph_fba_port_with_composition_and_operation_context(
    options: GraphFbaPortOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFbaPortGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_fba_port_graph_with_operation_context(
            &snapshot,
            &options.module,
            &options.port,
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokFbaPortGraphReport::new)
}

pub async fn graph_fba_dependencies_with_composition_and_operation_context(
    options: GraphFbaDependenciesOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFbaDependenciesGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_fba_dependencies_graph_with_operation_context(
            &snapshot,
            &options.module,
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokFbaDependenciesGraphReport::new)
}

pub async fn graph_fba_violations_with_composition_and_operation_context(
    options: GraphFbaViolationsOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokFbaViolationsGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_fba_violations_graph_with_operation_context(
            &snapshot,
            options.module.as_deref(),
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokFbaViolationsGraphReport::new)
}

pub async fn graph_page_builder_provider_with_composition_and_operation_context(
    options: GraphPageBuilderProviderOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokPageBuilderProviderGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "Page Builder")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_provider_graph_with_operation_context(
            &snapshot,
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokPageBuilderProviderGraphReport::new)
}

pub async fn graph_page_builder_consumer_with_composition_and_operation_context(
    options: GraphPageBuilderConsumerOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokPageBuilderConsumerGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "Page Builder")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_consumer_graph_with_operation_context(
            &snapshot,
            &options.module,
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokPageBuilderConsumerGraphReport::new)
}

pub async fn graph_page_builder_violations_with_composition_and_operation_context(
    options: GraphPageBuilderViolationsOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<RustokPageBuilderViolationsGraphReport> {
    validate_graph_limits(options.max_nodes, options.max_edges, "Page Builder")?;
    let snapshot = load_latest_snapshot(options.root, composition, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_violations_graph_with_operation_context(
            &snapshot,
            options.module.as_deref(),
            options.max_nodes,
            options.max_edges,
            &operation_for_worker,
        )
    })
    .await
    .map(RustokPageBuilderViolationsGraphReport::new)
}

fn validate_architecture_limits(options: &RustokArchitectureContextOptions) -> Result<()> {
    if options.max_modules == 0
        || options.max_contracts == 0
        || options.max_interactions == 0
        || options.max_evidence == 0
    {
        bail!("Rustok architecture context limits must be greater than zero");
    }
    Ok(())
}

fn validate_graph_limits(max_nodes: usize, max_edges: usize, label: &str) -> Result<()> {
    if max_nodes == 0 || max_edges == 0 {
        bail!("{label} graph limits must be greater than zero");
    }
    Ok(())
}

async fn load_latest_snapshot(
    root: PathBuf,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<CanonicalSnapshot> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let root = normalize_canonical_path(
        root.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", root.display()))?,
    );
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    store
        .load_latest_snapshot_with_operation_context(operation)
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })
}

async fn run_rustok_worker<T>(
    operation: &OperationContext,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T>
where
    T: Send + 'static,
{
    operation.check_active().map_err(anyhow::Error::new)?;
    let mut worker = tokio::task::spawn_blocking(work);
    let mut terminal_error = None;
    let poll_interval = Duration::from_millis(25);

    loop {
        tokio::select! {
            joined = &mut worker => {
                let result = joined.context("Rustok read worker task terminated unexpectedly")??;
                if let Some(error) = terminal_error {
                    return Err(error);
                }
                operation.check_active().map_err(anyhow::Error::new)?;
                return Ok(result);
            }
            _ = tokio::time::sleep(poll_interval), if terminal_error.is_none() => {
                if let Err(error) = operation.check_active() {
                    terminal_error = Some(anyhow::Error::new(error));
                }
            }
        }
    }
}
