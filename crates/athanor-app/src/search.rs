use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CanonicalSnapshotStoreOperationExt, OperationContext,
    OperationContextCancellation, SearchIndex, SearchIndexOperationExt, SearchQuery,
};
use athanor_domain::Entity;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::json_contract::SEARCH_SCHEMA_V1;
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;

#[path = "search/index.rs"]
mod index;
#[path = "search/model.rs"]
mod model;

pub use index::{
    entity_text, get_or_build_search_index, get_or_build_search_index_sync,
    get_or_build_search_index_with_operation_context,
};
pub use model::{
    SearchIndexFactory, SearchIndexOperationFactory, SearchItem, SearchOmissions, SearchOptions,
    SearchReport,
};
pub(crate) use index::{
    get_or_build_search_index_with_factory,
    get_or_build_search_index_with_factory_and_operation,
};

pub async fn search_project(options: SearchOptions) -> Result<SearchReport> {
    search_project_inner(options, None, None).await
}

pub async fn search_project_with_composition(
    options: SearchOptions,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    search_project_inner(options, Some(composition), None).await
}

pub async fn search_project_with_composition_and_operation_context(
    options: SearchOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    search_project_inner(options, Some(composition), Some(operation)).await
}

async fn search_project_inner(
    options: SearchOptions,
    composition: Option<&RuntimeComposition>,
    operation: Option<&OperationContext>,
) -> Result<SearchReport> {
    validate_search(&options.query, options.limit)?;
    check_active(operation)?;
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = match composition {
        Some(composition) => composition.init_store(&root, &config).await?,
        None => init_store(&root, &config).await?,
    };
    let snapshot = match operation {
        Some(operation) => store
            .load_latest_snapshot_with_operation_context(operation)
            .await,
        None => store.load_latest_snapshot().await,
    }
    .context("failed to load latest canonical snapshot")?
    .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;

    match (composition, operation) {
        (Some(composition), Some(operation)) => {
            search_snapshot_with_composition_and_operation_context(
                &root,
                &snapshot,
                options.query,
                options.limit,
                composition,
                operation,
            )
            .await
        }
        (Some(composition), None) => {
            search_snapshot_with_composition(
                &root,
                &snapshot,
                options.query,
                options.limit,
                composition,
            )
            .await
        }
        (None, Some(operation)) => {
            search_snapshot_with_operation_context(
                &root,
                &snapshot,
                options.query,
                options.limit,
                operation,
            )
            .await
        }
        (None, None) => search_snapshot(&root, &snapshot, options.query, options.limit).await,
    }
}

pub async fn search_snapshot_with_composition(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    let snapshot_id = required_snapshot_id(snapshot)?;
    let index_dir = root.join(".athanor/generated/current/search");
    let index = index::get_or_build_search_index_with_factory(
        snapshot,
        &snapshot_id,
        &index_dir,
        |directory, documents| composition.build_search_index(directory, documents),
    )?;
    search_snapshot_with_index(root, snapshot, query, limit, index.as_ref()).await
}

pub async fn search_snapshot_with_composition_and_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    validate_search(&query, limit)?;
    operation.check_active().map_err(anyhow::Error::new)?;
    let snapshot_id = required_snapshot_id(snapshot)?;
    let index_dir = root.join(".athanor/generated/current/search");
    let snapshot_for_worker = snapshot.clone();
    let composition = composition.clone();
    let operation_for_worker = operation.clone();
    let index = tokio::task::spawn_blocking(move || {
        index::get_or_build_search_index_with_factory_and_operation(
            &snapshot_for_worker,
            &snapshot_id,
            &index_dir,
            &operation_for_worker,
            |directory, documents, operation| {
                composition.build_search_index_with_operation_context(
                    directory,
                    documents,
                    operation,
                )
            },
        )
    })
    .await
    .context("search index rebuild worker terminated unexpectedly")??;
    search_snapshot_with_index_inner(root, snapshot, query, limit, index.as_ref(), Some(operation))
        .await
}

pub async fn search_snapshot(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
) -> Result<SearchReport> {
    validate_search(&query, limit)?;
    let snapshot_id = required_snapshot_id(snapshot)?;
    let index_dir = root.join(".athanor/generated/current/search");
    let index = index::get_or_build_search_index(snapshot, &snapshot_id, &index_dir).await?;
    search_snapshot_with_index(root, snapshot, query, limit, index.as_ref()).await
}

pub async fn search_snapshot_with_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    operation: &OperationContext,
) -> Result<SearchReport> {
    validate_search(&query, limit)?;
    operation.check_active().map_err(anyhow::Error::new)?;
    let snapshot_id = required_snapshot_id(snapshot)?;
    let index_dir = root.join(".athanor/generated/current/search");
    let snapshot_for_worker = snapshot.clone();
    let operation_for_worker = operation.clone();
    let index = tokio::task::spawn_blocking(move || {
        index::get_or_build_search_index_with_operation_context(
            &snapshot_for_worker,
            &snapshot_id,
            &index_dir,
            &operation_for_worker,
        )
    })
    .await
    .context("search index rebuild worker terminated unexpectedly")??;
    search_snapshot_with_index_inner(root, snapshot, query, limit, index.as_ref(), Some(operation))
        .await
}

pub async fn search_snapshot_with_index(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    index: &dyn SearchIndex,
) -> Result<SearchReport> {
    search_snapshot_with_index_inner(root, snapshot, query, limit, index, None).await
}

async fn search_snapshot_with_index_inner(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    index: &dyn SearchIndex,
    operation: Option<&OperationContext>,
) -> Result<SearchReport> {
    validate_search(&query, limit)?;
    check_active(operation)?;
    let snapshot_id = required_snapshot_id(snapshot)?;
    let search_query = SearchQuery {
        query: query.clone(),
        limit: limit.saturating_add(1),
    };
    let results = match operation {
        Some(operation) => index.search_with_operation_context(search_query, operation).await,
        None => index.search(search_query).await,
    }
    .context("failed to query search index")?;
    let truncated = results.len() > limit;
    let search_items = results
        .into_iter()
        .take(limit)
        .filter_map(|result| search_item(result.payload, result.score))
        .collect::<Vec<_>>();
    check_active(operation)?;
    let returned = search_items.len();

    Ok(SearchReport {
        schema: SEARCH_SCHEMA_V1.to_string(),
        root: root.to_path_buf(),
        snapshot: snapshot_id,
        query,
        limit,
        returned,
        truncated,
        omitted: SearchOmissions {
            results_lower_bound: usize::from(truncated),
            reason: truncated.then(|| "limit".to_string()),
        },
        results: search_items,
    })
}

fn legacy_search_index_factory() -> Option<SearchIndexFactory> {
    super::legacy_global::search_index_factory()
}

fn legacy_search_index_operation_factory() -> Option<SearchIndexOperationFactory> {
    super::legacy_global::search_index_operation_factory()
}

fn validate_search(query: &str, limit: usize) -> Result<()> {
    if query.trim().is_empty() {
        bail!("search query must not be empty");
    }
    if limit == 0 {
        bail!("search limit must be greater than zero");
    }
    Ok(())
}

fn required_snapshot_id(snapshot: &CanonicalSnapshot) -> Result<String> {
    snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))
}

fn check_active(operation: Option<&OperationContext>) -> Result<()> {
    if let Some(operation) = operation {
        operation.check_active().map_err(anyhow::Error::new)?;
    }
    Ok(())
}

fn search_item(payload: serde_json::Value, score: f32) -> Option<SearchItem> {
    let entity: Entity = serde_json::from_value(payload).ok()?;
    let kind = serde_json::to_value(&entity.kind)
        .ok()?
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    Some(SearchItem {
        entity_id: entity.id,
        stable_key: entity.stable_key.0,
        kind,
        name: entity.name,
        title: entity.title,
        source: entity.source,
        ownership: entity.ownership,
        score,
    })
}
