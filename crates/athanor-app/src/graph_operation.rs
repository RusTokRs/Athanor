use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStoreOperationExt, OperationContext,
    OperationContextCancellation,
};

use crate::config::load_config;
use crate::graph::{
    GraphCycles, GraphCyclesOptions, GraphExport, GraphExportOptions, GraphHubs, GraphHubsOptions,
    GraphPageRank, GraphPageRankOptions, GraphPath, GraphPathOptions, GraphRelated,
    GraphRelatedOptions, build_graph_cycles, build_graph_export, build_graph_hubs,
    build_graph_pagerank, build_related_graph, build_shortest_graph_path,
};
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;

/// Builds a bounded graph export on a blocking worker while preserving operation lifecycle.
pub async fn export_graph_with_operation_context(
    options: GraphExportOptions,
    operation: &OperationContext,
) -> Result<GraphExport> {
    if options.max_entities == 0 || options.max_relations == 0 {
        bail!("graph export entity and relation limits must be greater than zero");
    }
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_graph_worker(operation, move || {
        Ok(build_graph_export(
            &snapshot,
            options.max_entities,
            options.max_relations,
        ))
    })
    .await
}

/// Builds a bounded related-entity graph on a blocking worker.
pub async fn related_graph_with_operation_context(
    options: GraphRelatedOptions,
    operation: &OperationContext,
) -> Result<GraphRelated> {
    if options.max_entities == 0 || options.max_relations == 0 {
        bail!("graph related entity and relation limits must be greater than zero");
    }
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_graph_worker(operation, move || {
        build_related_graph(
            &snapshot,
            &options.stable_key,
            options.depth,
            options.max_entities,
            options.max_relations,
        )
    })
    .await
}

/// Builds a bounded shortest path on a blocking worker.
pub async fn shortest_graph_path_with_operation_context(
    options: GraphPathOptions,
    operation: &OperationContext,
) -> Result<GraphPath> {
    if options.max_visited == 0 {
        bail!("graph path max visited limit must be greater than zero");
    }
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_graph_worker(operation, move || {
        build_shortest_graph_path(
            &snapshot,
            &options.from_stable_key,
            &options.to_stable_key,
            options.max_depth,
            options.max_visited,
        )
    })
    .await
}

/// Ranks graph hubs on a blocking worker.
pub async fn graph_hubs_with_operation_context(
    options: GraphHubsOptions,
    operation: &OperationContext,
) -> Result<GraphHubs> {
    if options.limit == 0 || options.max_relation_ids == 0 {
        bail!("graph hubs limits must be greater than zero");
    }
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_graph_worker(operation, move || {
        build_graph_hubs(
            &snapshot,
            options.limit,
            options.kind.as_deref(),
            options.max_relation_ids,
        )
    })
    .await
}

/// Computes PageRank on a blocking worker.
pub async fn graph_pagerank_with_operation_context(
    options: GraphPageRankOptions,
    operation: &OperationContext,
) -> Result<GraphPageRank> {
    if options.limit == 0
        || options.max_iterations == 0
        || options.max_relation_ids == 0
        || !(0.0..1.0).contains(&options.damping)
        || options.tolerance <= 0.0
        || !options.tolerance.is_finite()
    {
        bail!(
            "graph pagerank requires positive limits and tolerance, with damping between zero and one"
        );
    }
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_graph_worker(operation, move || {
        build_graph_pagerank(
            &snapshot,
            options.limit,
            options.kind.as_deref(),
            options.damping,
            options.max_iterations,
            options.tolerance,
            options.max_relation_ids,
        )
    })
    .await
}

/// Finds bounded directed cycles on a blocking worker.
pub async fn graph_cycles_with_operation_context(
    options: GraphCyclesOptions,
    operation: &OperationContext,
) -> Result<GraphCycles> {
    if options.limit == 0 || options.max_depth == 0 || options.max_starts == 0 {
        bail!("graph cycle limits must be greater than zero");
    }
    let snapshot = load_latest_snapshot(options.root, operation).await?;
    run_graph_worker(operation, move || {
        build_graph_cycles(
            &snapshot,
            options.limit,
            options.max_depth,
            options.max_starts,
        )
    })
    .await
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

async fn run_graph_worker<T>(
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
                let result = joined.context("graph worker task terminated unexpectedly")??;
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
    async fn cancellation_drains_blocking_worker_before_returning() {
        let operation = OperationContext::new("graph-worker-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
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

        let error = run_graph_worker(&operation, move || {
            worker_started.store(true, Ordering::Release);
            std::thread::sleep(Duration::from_millis(50));
            worker_completed.store(true, Ordering::Release);
            Ok(42_u8)
        })
        .await
        .expect_err("cancelled worker must not return success");
        cancel_task.await.unwrap();

        assert!(completed.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }

    #[tokio::test]
    async fn expired_deadline_does_not_spawn_blocking_worker() {
        let operation = OperationContext::new("graph-worker-expired").with_deadline_unix_ms(0);
        let started = Arc::new(AtomicBool::new(false));
        let worker_started = Arc::clone(&started);

        let error = run_graph_worker(&operation, move || {
            worker_started.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .expect_err("expired operation must fail before worker spawn");

        assert!(!started.load(Ordering::Acquire));
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::DeadlineExceeded(_))
        )));
    }
}
