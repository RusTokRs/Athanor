use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStoreOperationExt, OperationContext,
    OperationContextCancellation,
};
use athanor_domain::ContextLevel;

use crate::config::load_config;
use crate::context::{ContextLimitOverrides, ContextOptions};
use crate::derived_read_operation::context_project_with_operation_context;
use crate::graph::{
    GraphFbaDependenciesOptions, GraphFbaModuleOptions, GraphFbaPortOptions,
    GraphFbaViolationsOptions, GraphFfaSurfaceOptions, GraphFfaViolationsOptions,
    GraphPageBuilderConsumerOptions, GraphPageBuilderProviderOptions,
    GraphPageBuilderViolationsOptions, RustokFbaAudit, RustokFbaAuditOptions, RustokFbaGraph,
    RustokFfaAudit, RustokFfaAuditOptions, RustokFfaGraph, RustokPageBuilderAudit,
    RustokPageBuilderAuditOptions, RustokPageBuilderGraph, build_rustok_fba_dependencies_graph,
    build_rustok_fba_module_graph, build_rustok_fba_port_graph,
    build_rustok_fba_violations_graph, build_rustok_ffa_surface_graph,
    build_rustok_ffa_violations_graph, build_rustok_page_builder_consumer_graph,
    build_rustok_page_builder_provider_graph, build_rustok_page_builder_violations_graph,
};
use crate::project_path::normalize_canonical_path;
use crate::rustok_architecture::{RustokArchitectureContext, RustokArchitectureContextOptions};
use crate::rustok_architecture_cooperative::build_rustok_architecture_context_with_operation_context;
use crate::rustok_audit_cooperative::{
    build_rustok_fba_audit_with_operation_context, build_rustok_ffa_audit_with_operation_context,
    build_rustok_page_builder_audit_with_operation_context,
};
use crate::store::init_store;

pub async fn rustok_architecture_context_with_operation_context(
    mut options: RustokArchitectureContextOptions,
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
    let context = context_project_with_operation_context(
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
        operation,
    )
    .await?;
    let snapshot = load_latest_snapshot(root.clone(), operation).await?;
    options.root = root;
    let context_entities = context.entities;
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

pub async fn rustok_ffa_audit_with_operation_context(
    options: RustokFfaAuditOptions,
    operation: &OperationContext,
) -> Result<RustokFfaAudit> {
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_ffa_audit_with_operation_context(&snapshot, &operation_for_worker)
    })
    .await
}

pub async fn rustok_fba_audit_with_operation_context(
    options: RustokFbaAuditOptions,
    operation: &OperationContext,
) -> Result<RustokFbaAudit> {
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_fba_audit_with_operation_context(&snapshot, &operation_for_worker)
    })
    .await
}

pub async fn rustok_page_builder_audit_with_operation_context(
    options: RustokPageBuilderAuditOptions,
    operation: &OperationContext,
) -> Result<RustokPageBuilderAudit> {
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    let operation_for_worker = operation.clone();
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_audit_with_operation_context(&snapshot, &operation_for_worker)
    })
    .await
}

pub async fn graph_ffa_surface_with_operation_context(
    options: GraphFfaSurfaceOptions,
    operation: &OperationContext,
) -> Result<RustokFfaGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FFA")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        build_rustok_ffa_surface_graph(
            &snapshot,
            &options.module,
            &options.surface,
            options.max_nodes,
            options.max_edges,
        )
    })
    .await
}

pub async fn graph_ffa_violations_with_operation_context(
    options: GraphFfaViolationsOptions,
    operation: &OperationContext,
) -> Result<RustokFfaGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FFA")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        Ok(build_rustok_ffa_violations_graph(
            &snapshot,
            options.module.as_deref(),
            options.surface.as_deref(),
            options.max_nodes,
            options.max_edges,
        ))
    })
    .await
}

pub async fn graph_fba_module_with_operation_context(
    options: GraphFbaModuleOptions,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        build_rustok_fba_module_graph(
            &snapshot,
            &options.module,
            options.max_nodes,
            options.max_edges,
        )
    })
    .await
}

pub async fn graph_fba_port_with_operation_context(
    options: GraphFbaPortOptions,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        build_rustok_fba_port_graph(
            &snapshot,
            &options.module,
            &options.port,
            options.max_nodes,
            options.max_edges,
        )
    })
    .await
}

pub async fn graph_fba_dependencies_with_operation_context(
    options: GraphFbaDependenciesOptions,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        build_rustok_fba_dependencies_graph(
            &snapshot,
            &options.module,
            options.max_nodes,
            options.max_edges,
        )
    })
    .await
}

pub async fn graph_fba_violations_with_operation_context(
    options: GraphFbaViolationsOptions,
    operation: &OperationContext,
) -> Result<RustokFbaGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "FBA")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        Ok(build_rustok_fba_violations_graph(
            &snapshot,
            options.module.as_deref(),
            options.max_nodes,
            options.max_edges,
        ))
    })
    .await
}

pub async fn graph_page_builder_provider_with_operation_context(
    options: GraphPageBuilderProviderOptions,
    operation: &OperationContext,
) -> Result<RustokPageBuilderGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "Page Builder")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_provider_graph(
            &snapshot,
            options.max_nodes,
            options.max_edges,
        )
    })
    .await
}

pub async fn graph_page_builder_consumer_with_operation_context(
    options: GraphPageBuilderConsumerOptions,
    operation: &OperationContext,
) -> Result<RustokPageBuilderGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "Page Builder")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        build_rustok_page_builder_consumer_graph(
            &snapshot,
            &options.module,
            options.max_nodes,
            options.max_edges,
        )
    })
    .await
}

pub async fn graph_page_builder_violations_with_operation_context(
    options: GraphPageBuilderViolationsOptions,
    operation: &OperationContext,
) -> Result<RustokPageBuilderGraph> {
    validate_graph_limits(options.max_nodes, options.max_edges, "Page Builder")?;
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_rustok_worker(operation, move || {
        Ok(build_rustok_page_builder_violations_graph(
            &snapshot,
            options.module.as_deref(),
            options.max_nodes,
            options.max_edges,
        ))
    })
    .await
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
    operation: &OperationContext,
) -> Result<CanonicalSnapshot> {
    operation.check_active().map_err(anyhow::Error::new)?;
    let root = normalize_canonical_path(
        root.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", root.display()))?,
    );
    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use athanor_core::CoreError;

    use super::*;

    #[tokio::test]
    async fn pre_cancelled_rustok_read_does_not_spawn_worker() {
        let operation = OperationContext::new("rustok-read-pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();
        let started = Arc::new(AtomicBool::new(false));
        let worker_started = Arc::clone(&started);

        let error = run_rustok_worker(&operation, move || {
            worker_started.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .expect_err("cancelled Rustok read must fail before worker spawn");

        assert!(!started.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[tokio::test]
    async fn cancellation_drains_rustok_worker_before_returning() {
        let operation = OperationContext::new("rustok-read-worker-cancelled");
        let cancellation_lease = operation.cancellation_handle().unwrap();
        let cancellation = cancellation_lease.clone();
        let started = Arc::new(AtomicBool::new(false));
        let completed = Arc::new(AtomicBool::new(false));
        let worker_started = Arc::clone(&started);
        let worker_completed = Arc::clone(&completed);
        let cancel_started = Arc::clone(&started);
        let cancel_task = tokio::spawn(async move {
            while !cancel_started.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
            cancellation.cancel();
        });

        let error = run_rustok_worker(&operation, move || {
            worker_started.store(true, Ordering::Release);
            std::thread::sleep(Duration::from_millis(50));
            worker_completed.store(true, Ordering::Release);
            Ok(42_u8)
        })
        .await
        .expect_err("cancelled Rustok worker must not return late success");
        cancel_task.await.unwrap();

        assert!(cancellation_lease.is_cancelled());
        assert!(completed.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }
}
