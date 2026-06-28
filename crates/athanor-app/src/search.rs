use crate::config::load_config;
use crate::store::init_store;
use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, SearchDocument, SearchIndex, SearchQuery,
};
use athanor_domain::{Entity, EntityId, Ownership, SourceLocation};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::project_path::normalize_canonical_path;

type SearchIndexFactory = fn(&Path, Option<Vec<SearchDocument>>) -> Result<Arc<dyn SearchIndex>>;

static SEARCH_INDEX_FACTORY: OnceLock<SearchIndexFactory> = OnceLock::new();

pub fn install_search_index_factory(factory: SearchIndexFactory) {
    let _ = SEARCH_INDEX_FACTORY.set(factory);
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
    if options.query.trim().is_empty() {
        bail!("search query must not be empty");
    }
    if options.limit == 0 {
        bail!("search limit must be greater than zero");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );

    let config = load_config(&root)?;
    let store = init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;

    search_snapshot(&root, &snapshot, options.query, options.limit).await
}

pub async fn search_snapshot(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
) -> Result<SearchReport> {
    if query.trim().is_empty() {
        bail!("search query must not be empty");
    }
    if limit == 0 {
        bail!("search limit must be greater than zero");
    }

    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|s| s.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))?;
    let index_dir = root.join(".athanor/generated/current/search");

    // Open or rebuild index if snapshot changed or index doesn't exist
    let index = get_or_build_search_index(snapshot, &snapshot_id, &index_dir).await?;
    search_snapshot_with_index(root, snapshot, query, limit, index.as_ref()).await
}

pub async fn search_snapshot_with_index(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    index: &dyn SearchIndex,
) -> Result<SearchReport> {
    if query.trim().is_empty() {
        bail!("search query must not be empty");
    }
    if limit == 0 {
        bail!("search limit must be greater than zero");
    }

    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|s| s.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))?;

    let results = index
        .search(SearchQuery {
            query: query.clone(),
            limit: limit.saturating_add(1),
        })
        .await
        .context("failed to query search index")?;
    let truncated = results.len() > limit;

    let search_items = results
        .into_iter()
        .take(limit)
        .filter_map(|res| {
            let entity: Entity = serde_json::from_value(res.payload).ok()?;
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
                score: res.score,
            })
        })
        .collect::<Vec<_>>();
    let returned = search_items.len();

    Ok(SearchReport {
        schema: "athanor.search.v1".to_string(),
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
    let Some(factory) = SEARCH_INDEX_FACTORY.get() else {
        bail!("no Athanor search index factory is installed");
    };
    let meta_path = index_dir.join("index_meta.json");
    let needs_rebuild = if index_dir.exists() && meta_path.exists() {
        if let Ok(meta_str) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<IndexMeta>(&meta_str) {
                meta.snapshot_id != snapshot_id
            } else {
                true
            }
        } else {
            true
        }
    } else {
        true
    };

    if needs_rebuild {
        let documents = snapshot
            .entities
            .iter()
            .map(|entity| {
                Ok(SearchDocument {
                    id: entity.id.0.clone(),
                    title: entity.title.clone().unwrap_or_else(|| entity.name.clone()),
                    body: entity_text(entity),
                    payload: serde_json::to_value(entity)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let index = factory(index_dir, Some(documents)).context("failed to rebuild search index")?;

        let meta = IndexMeta {
            snapshot_id: snapshot_id.to_string(),
        };
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

        return Ok(index);
    }

    factory(index_dir, None).context("failed to open search index")
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

    // Add some text fields from payload if they exist and are strings
    if let Some(desc) = entity.payload.get("description").and_then(|d| d.as_str()) {
        parts.push(desc);
    }
    if let Some(summary) = entity.payload.get("summary").and_then(|s| s.as_str()) {
        parts.push(summary);
    }

    parts.join(" ").to_lowercase()
}
