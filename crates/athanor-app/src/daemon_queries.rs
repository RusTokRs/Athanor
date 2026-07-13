use std::sync::Arc;

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore, SearchIndex};
use athanor_domain::ContextLevel;
use serde_json::Value;

use crate::config::load_config;
use crate::daemon::{CachedSearchIndex, ContextCacheKey, DaemonState, OverviewCacheKey};
use crate::explain::explain_snapshot;
use crate::search::{
    get_or_build_search_index_sync, get_or_build_search_index_with_factory,
    search_snapshot_with_index,
};
use crate::store::init_store;
use crate::{
    ContextLimitOverrides, ContextLimits, RepositoryOverview, RuntimeComposition,
    build_repository_overview, generate_context_pack,
};

pub(crate) async fn latest_snapshot(state: &Arc<DaemonState>) -> Result<CanonicalSnapshot> {
    if let Some(snapshot) = state
        .latest_snapshot_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon snapshot cache lock is poisoned"))?
        .clone()
    {
        return Ok(snapshot);
    }

    let root = &state.endpoint.root;
    let config = load_config(root)?;
    let store = match composition(state) {
        Some(composition) => composition.init_store(root, &config).await?,
        None => init_store(root, &config).await?,
    };
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
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
) -> Result<athanor_domain::ContextPack> {
    let mut limits = ContextLimits::for_level(level);
    overrides.apply(&mut limits);
    if limits.max_tokens == 0 || limits.max_files == 0 || limits.max_entities == 0 {
        bail!("context token, file, and entity limits must be greater than zero");
    }
    let snapshot = latest_snapshot(state).await?;
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
        return Ok(pack);
    }
    let direct_matches = if let Ok(index) = search_index(state, &snapshot) {
        if let Ok(results) = index
            .search(athanor_core::SearchQuery {
                query: task.to_string(),
                limit: limits.max_entities,
            })
            .await
        {
            Some(
                results
                    .into_iter()
                    .map(|result| athanor_domain::EntityId(result.id))
                    .collect(),
            )
        } else {
            None
        }
    } else {
        None
    };
    let pack = generate_context_pack(&snapshot, task, level, limits, direct_matches);
    state
        .context_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon context cache lock is poisoned"))?
        .insert(cache_key, pack.clone());
    Ok(pack)
}

pub(crate) async fn overview(state: &Arc<DaemonState>, top: usize) -> Result<RepositoryOverview> {
    let snapshot = latest_snapshot(state).await?;
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
        return Ok(overview);
    }
    let overview = build_repository_overview(&snapshot, top);
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
    explain_snapshot(&latest_snapshot(state).await?, stable_key)
}

pub(crate) async fn search(
    state: &Arc<DaemonState>,
    query: String,
    limit: usize,
) -> Result<crate::search::SearchReport> {
    let snapshot = latest_snapshot(state).await?;
    let index = search_index(state, &snapshot)?;
    search_snapshot_with_index(
        &state.endpoint.root,
        &snapshot,
        query,
        limit,
        index.as_ref(),
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
    let snapshot_id = snapshot_id(snapshot)?;
    let mut cache = state
        .search_index_cache
        .lock()
        .map_err(|_| anyhow::anyhow!("daemon search index cache lock is poisoned"))?;
    if let Some(cached) = cache.as_ref()
        && cached.snapshot_id == snapshot_id
    {
        return Ok(Arc::clone(&cached.index));
    }
    let index_dir = state
        .endpoint
        .root
        .join(".athanor/generated/current/search");
    let index = match composition(state) {
        Some(composition) => get_or_build_search_index_with_factory(
            snapshot,
            &snapshot_id,
            &index_dir,
            |directory, documents| composition.build_search_index(directory, documents),
        )?,
        None => get_or_build_search_index_sync(snapshot, &snapshot_id, &index_dir)?,
    };
    *cache = Some(CachedSearchIndex {
        snapshot_id,
        index: Arc::clone(&index),
    });
    Ok(index)
}
