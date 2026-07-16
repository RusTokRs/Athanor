use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CanonicalSnapshotStoreOperationExt, OperationContext,
    OperationContextCancellation, SearchDocument, SearchIndex, SearchIndexOperationExt, SearchQuery,
};
use athanor_domain::{Entity, EntityId, Ownership, SourceLocation};
use serde::{Deserialize, Serialize};

use crate::config::load_config;
use crate::json_contract::SEARCH_SCHEMA_V1;
use crate::project_path::normalize_canonical_path;
use crate::store::init_store;
use crate::RuntimeComposition;

const SEARCH_REBUILD_POLL_DOCUMENTS: usize = 256;

pub type SearchIndexFactory =
    fn(&Path, Option<Vec<SearchDocument>>) -> Result<Arc<dyn SearchIndex>>;
pub type SearchIndexOperationFactory = fn(
    &Path,
    Option<Vec<SearchDocument>>,
    &OperationContext,
) -> Result<Arc<dyn SearchIndex>>;

static SEARCH_INDEX_FACTORY: OnceLock<SearchIndexFactory> = OnceLock::new();
static SEARCH_INDEX_OPERATION_FACTORY: OnceLock<SearchIndexOperationFactory> = OnceLock::new();

pub fn install_search_index_factory(factory: SearchIndexFactory) {
    let _ = SEARCH_INDEX_FACTORY.set(factory);
}

pub fn install_search_index_operation_factory(factory: SearchIndexOperationFactory) {
    let _ = SEARCH_INDEX_OPERATION_FACTORY.set(factory);
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub root: PathBuf,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    pub entity_id: EntityId,
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub title: Option<String>,
    pub source: Option<SourceLocation>,
    pub ownership: Vec<Ownership>,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub schema: String,
    pub root: PathBuf,
    pub snapshot: String,
    pub query: String,
    pub limit: usize,
    pub returned: usize,
    pub truncated: bool,
    pub omitted: SearchOmissions,
    pub results: Vec<SearchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOmissions {
    pub results_lower_bound: usize,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexMeta {
    snapshot_id: String,
}

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
    let index = get_or_build_search_index_with_factory(
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
        get_or_build_search_index_with_factory_and_operation(
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
    let index = get_or_build_search_index(snapshot, &snapshot_id, &index_dir).await?;
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
        get_or_build_search_index_with_operation_context(
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

pub async fn get_or_build_search_index(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
) -> Result<Arc<dyn SearchIndex>> {
    get_or_build_search_index_sync(snapshot, snapshot_id, index_dir)
}

pub fn get_or_build_search_index_sync(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    crate::ensure_test_runtime();
    let Some(factory) = SEARCH_INDEX_FACTORY.get() else {
        bail!("no Athanor search index factory is installed");
    };
    get_or_build_search_index_with_factory(snapshot, snapshot_id, index_dir, factory)
}

pub fn get_or_build_search_index_with_operation_context(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    operation: &OperationContext,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    crate::ensure_test_runtime();
    if let Some(factory) = SEARCH_INDEX_OPERATION_FACTORY.get() {
        return get_or_build_search_index_with_factory_and_operation(
            snapshot,
            snapshot_id,
            index_dir,
            operation,
            factory,
        );
    }
    let Some(factory) = SEARCH_INDEX_FACTORY.get() else {
        bail!("no Athanor search index factory is installed");
    };
    get_or_build_search_index_with_factory_and_operation(
        snapshot,
        snapshot_id,
        index_dir,
        operation,
        |directory, documents, operation| {
            operation.check_active().map_err(anyhow::Error::new)?;
            let index = factory(directory, documents)?;
            operation.check_active().map_err(anyhow::Error::new)?;
            Ok(index)
        },
    )
}

pub(crate) fn get_or_build_search_index_with_factory(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    factory: impl Fn(&Path, Option<Vec<SearchDocument>>) -> Result<Arc<dyn SearchIndex>>,
) -> Result<Arc<dyn SearchIndex>> {
    get_or_build_search_index_inner(snapshot, snapshot_id, index_dir, None, |dir, docs, _| {
        factory(dir, docs)
    })
}

pub(crate) fn get_or_build_search_index_with_factory_and_operation(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    operation: &OperationContext,
    factory: impl Fn(
        &Path,
        Option<Vec<SearchDocument>>,
        &OperationContext,
    ) -> Result<Arc<dyn SearchIndex>>,
) -> Result<Arc<dyn SearchIndex>> {
    get_or_build_search_index_inner(
        snapshot,
        snapshot_id,
        index_dir,
        Some(operation),
        factory,
    )
}

fn get_or_build_search_index_inner(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    operation: Option<&OperationContext>,
    factory: impl Fn(
        &Path,
        Option<Vec<SearchDocument>>,
        &OperationContext,
    ) -> Result<Arc<dyn SearchIndex>>,
) -> Result<Arc<dyn SearchIndex>> {
    check_active(operation)?;
    let meta_path = index_dir.join("index_meta.json");
    let needs_rebuild = if index_dir.exists() && meta_path.exists() {
        fs::read_to_string(&meta_path)
            .ok()
            .and_then(|contents| serde_json::from_str::<IndexMeta>(&contents).ok())
            .is_none_or(|meta| meta.snapshot_id != snapshot_id)
    } else {
        true
    };
    let fallback_operation = OperationContext::default();
    let operation_for_factory = operation.unwrap_or(&fallback_operation);

    if needs_rebuild {
        let mut documents = Vec::with_capacity(snapshot.entities.len());
        for (position, entity) in snapshot.entities.iter().enumerate() {
            if position % SEARCH_REBUILD_POLL_DOCUMENTS == 0 {
                check_active(operation)?;
            }
            documents.push(SearchDocument {
                id: entity.id.0.clone(),
                title: entity.title.clone().unwrap_or_else(|| entity.name.clone()),
                body: entity_text(entity),
                payload: serde_json::to_value(entity)?,
            });
        }
        check_active(operation)?;
        let index = factory(index_dir, Some(documents), operation_for_factory)
            .context("failed to rebuild search index")?;
        check_active(operation)?;
        let meta = IndexMeta {
            snapshot_id: snapshot_id.to_string(),
        };
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        check_active(operation)?;
        return Ok(index);
    }

    let index = factory(index_dir, None, operation_for_factory)
        .context("failed to open search index")?;
    check_active(operation)?;
    Ok(index)
}

pub fn entity_text(entity: &Entity) -> String {
    let mut parts = vec![entity.name.as_str(), entity.stable_key.0.as_str()];
    if let Some(title) = &entity.title {
        parts.push(title);
    }
    if let Some(source) = &entity.source {
        parts.push(&source.path);
    }
    parts.extend(entity.aliases.iter().map(String::as_str));
    if let Some(description) = entity.payload.get("description").and_then(|value| value.as_str()) {
        parts.push(description);
    }
    if let Some(summary) = entity.payload.get("summary").and_then(|value| value.as_str()) {
        parts.push(summary);
    }
    parts.join(" ").to_lowercase()
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
