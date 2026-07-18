use std::sync::Arc;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStoreOperationExt, CoreError, OperationContext,
    OperationContextCancellation, SearchIndex, SearchIndexOperationExt,
};
use athanor_domain::ContextLevel;
use serde_json::Value;

use crate::config::load_config;
use crate::daemon::{CachedSearchIndex, ContextCacheKey, DaemonState, OverviewCacheKey};
use crate::explain::explain_snapshot;
use crate::search::get_or_build_search_index_with_factory_and_operation;
use crate::search_operation::search_snapshot_with_index_and_operation_context;
use crate::{
    ContextLimitOverrides, ContextLimits, ContextReport, RepositoryOverview, RuntimeComposition,
    build_repository_overview, generate_context_pack,
};

pub(crate) async fn latest_snapshot(state: &Arc<DaemonState>) -> Result<CanonicalSnapshot> {
    latest_snapshot_with_operation_context(state, &OperationContext::default()).await
}

pub(crate) async fn latest_snapshot_with_operation_context(
    state: &Arc<DaemonState>,
    operation: &OperationContext,
) -> Result<CanonicalSnapshot> {
    check_active(operation)?;
    if let Some(snapshot) = state
        .latest_snapshot_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon snapshot cache lock is poisoned"))?
        .clone()
    {
        check_active(operation)?;
        return Ok(snapshot);
    }

    let root = &state.endpoint.root;
    let config = load_config(root)?;
    check_active(operation)?;
    let composition = composition(state)
        .ok_or_else(|| anyhow::anyhow!("daemon runtime composition is unavailable"))?;
    let store = composition.init_store(root, &config).await?;
    let snapshot = store
        .load_latest_snapshot_with_operation_context(operation)
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    check_active(operation)?;
    *state
        .latest_snapshot_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon snapshot cache lock is poisoned"))? =
        Some(snapshot.clone());
    Ok(snapshot)
}

pub(crate) fn composition(state: &DaemonState) -> Option<RuntimeComposition> {
    state.composition.clone()
}

pub(crate) async fn context(
    state: &Arc<DaemonState>,
    task: &str,
    level: ContextLevel,
    overrides: &ContextLimitOverrides,
) -> Result<ContextReport> {
    context_with_operation_context(
        state,
        task,
        level,
        overrides,
        &OperationContext::default(),
    )
    .await
}

pub(crate) async fn context_with_operation_context(
    state: &Arc<DaemonState>,
    task: &str,
    level: ContextLevel,
    overrides: &ContextLimitOverrides,
    operation: &OperationContext,
) -> Result<ContextReport> {
    check_active(operation)?;
    let mut limits = ContextLimits::for_level(level);
    overrides.apply(&mut limits);
    if limits.max_tokens == 0 || limits.max_files == 0 || limits.max_entities == 0 {
        bail!("context token, file, and entity limits must be greater than zero");
    }
    let snapshot = latest_snapshot_with_operation_context(state, operation).await?;
    let cache_key = ContextCacheKey {
        snapshot_id: snapshot
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.0.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        task: task.to_string(),
        level: format!("{level:?}"),
        limits,
    };
    if let Some(pack) = state
        .context_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon context cache lock is poisoned"))?
        .get(&cache_key)
    {
        check_active(operation)?;
        return Ok(ContextReport::from(pack));
    }
    let direct_matches = match search_index_with_operation_context(state, &snapshot, operation) {
        Ok(index) => match index
            .search_with_operation_context(
                athanor_core::SearchQuery {
                    query: task.to_string(),
                    limit: limits.max_entities,
                },
                operation,
            )
            .await
        {
            Ok(results) => Some(
                results
                    .into_iter()
                    .map(|result| athanor_domain::EntityId(result.id))
                    .collect(),
            ),
            Err(error @ (CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_))) => {
                return Err(anyhow::Error::new(error));
            }
            Err(_) => None,
        },
        Err(error) if is_operation_termination(&error) => return Err(error),
        Err(_) => None,
    };
    check_active(operation)?;
    let pack = generate_context_pack(&snapshot, task, level, limits, direct_matches);
    check_active(operation)?;
    state
        .context_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon context cache lock is poisoned"))?
        .insert(cache_key, pack.clone());
    Ok(ContextReport::from(pack))
}

pub(crate) async fn overview(state: &Arc<DaemonState>, top: usize) -> Result<RepositoryOverview> {
    overview_with_operation_context(state, top, &OperationContext::default()).await
}

pub(crate) async fn overview_with_operation_context(
    state: &Arc<DaemonState>,
    top: usize,
    operation: &OperationContext,
) -> Result<RepositoryOverview> {
    check_active(operation)?;
    let snapshot = latest_snapshot_with_operation_context(state, operation).await?;
    let cache_key = OverviewCacheKey {
        snapshot_id: snapshot_id(&snapshot)?,
        top,
    };
    if let Some(overview) = state
        .overview_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon overview cache lock is poisoned"))?
        .get(&cache_key)
    {
        check_active(operation)?;
        return Ok(overview);
    }
    let overview = build_repository_overview(&snapshot, top);
    check_active(operation)?;
    state
        .overview_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon overview cache lock is poisoned"))?
        .insert(cache_key, overview.clone());
    Ok(overview)
}

pub(crate) async fn explain(
    state: &Arc<DaemonState>,
    stable_key: &str,
) -> Result<crate::explain::EntityExplanation> {
    explain_with_operation_context(state, stable_key, &OperationContext::default()).await
}

pub(crate) async fn explain_with_operation_context(
    state: &Arc<DaemonState>,
    stable_key: &str,
    operation: &OperationContext,
) -> Result<crate::explain::EntityExplanation> {
    check_active(operation)?;
    let explanation = explain_snapshot(
        &latest_snapshot_with_operation_context(state, operation).await?,
        stable_key,
    )?;
    check_active(operation)?;
    Ok(explanation)
}

pub(crate) async fn search(
    state: &Arc<DaemonState>,
    query: String,
    limit: usize,
) -> Result<crate::search::SearchReport> {
    let operation = OperationContext::default();
    let snapshot = latest_snapshot(state).await?;
    let index = search_index(state, &snapshot)?;
    search_snapshot_with_index_and_operation_context(
        &state.endpoint.root,
        &snapshot,
        query,
        limit,
        index.as_ref(),
        &operation,
    )
    .await
}

pub(crate) async fn search_with_operation_context(
    state: &Arc<DaemonState>,
    query: String,
    limit: usize,
    operation: &OperationContext,
) -> Result<crate::search::SearchReport> {
    check_active(operation)?;
    let snapshot = latest_snapshot_with_operation_context(state, operation).await?;
    let index = search_index_with_operation_context(state, &snapshot, operation)?;
    search_snapshot_with_index_and_operation_context(
        &state.endpoint.root,
        &snapshot,
        query,
        limit,
        index.as_ref(),
        operation,
    )
    .await
}

pub(crate) fn invalidate(state: &DaemonState) {
    match state.latest_snapshot_cache.lock() {
        Ok(mut cache) => *cache = None,
        Err(_) => tracing::warn!("daemon snapshot cache lock is poisoned"),
    }
    match state.search_index_cache.lock() {
        Ok(mut cache) => *cache = None,
        Err(_) => tracing::warn!("daemon search index cache lock is poisoned"),
    }
    match state.overview_cache.lock() {
        Ok(mut cache) => cache.clear(),
        Err(_) => tracing::warn!("daemon overview cache lock is poisoned"),
    }
    match state.context_cache.lock() {
        Ok(mut cache) => cache.clear(),
        Err(_) => tracing::warn!("daemon context cache lock is poisoned"),
    }
}

pub(crate) fn cache_status(state: &DaemonState) -> Value {
    serde_json::json!({
        "snapshot_loaded": state.latest_snapshot_cache.lock().is_ok_and(|cache| cache.is_some()),
        "search_index_loaded": state.search_index_cache.lock().is_ok_and(|cache| cache.is_some()),
        "overview_entries": state.overview_cache.lock().map_or(0, |cache| cache.len()),
        "context_entries": state.context_cache.lock().map_or(0, |cache| cache.len()),
    })
}

fn snapshot_id(snapshot: &CanonicalSnapshot) -> Result<String> {
    snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))
}

fn search_index(state: &DaemonState, snapshot: &CanonicalSnapshot) -> Result<Arc<dyn SearchIndex>> {
    search_index_with_operation_context(state, snapshot, &OperationContext::default())
}

fn search_index_with_operation_context(
    state: &DaemonState,
    snapshot: &CanonicalSnapshot,
    operation: &OperationContext,
) -> Result<Arc<dyn SearchIndex>> {
    check_active(operation)?;
    let snapshot_id = snapshot_id(snapshot)?;
    let mut cache = state
        .search_index_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon search index cache lock is poisoned"))?;
    if let Some(cached) = cache.as_ref()
        && cached.snapshot_id == snapshot_id
    {
        check_active(operation)?;
        return Ok(Arc::clone(&cached.index));
    }

    let stale = cache.take();
    drop(stale);
    let index_dir = state
        .endpoint
        .root
        .join(".athanor/generated/current/search");
    let composition = composition(state)
        .ok_or_else(|| anyhow::anyhow!("daemon runtime composition is unavailable"))?;
    let index = get_or_build_search_index_with_factory_and_operation(
        snapshot,
        &snapshot_id,
        &index_dir,
        operation,
        |directory, documents, operation| {
            composition.build_search_index_with_operation_context(
                directory,
                documents,
                operation,
            )
        },
    )?;
    check_active(operation)?;
    *cache = Some(CachedSearchIndex {
        snapshot_id,
        index: Arc::clone(&index),
    });
    Ok(index)
}

fn check_active(operation: &OperationContext) -> Result<()> {
    operation.check_active().map_err(anyhow::Error::new)
}

fn is_operation_termination(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<CoreError>().is_some_and(|error| {
            matches!(error, CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_))
        })
    })
}
