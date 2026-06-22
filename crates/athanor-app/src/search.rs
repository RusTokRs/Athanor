use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, SearchDocument, SearchIndex, SearchQuery,
};
use athanor_domain::Entity;
use athanor_search_tantivy::TantivySearchIndex;
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::project_path::normalize_canonical_path;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub root: PathBuf,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    pub stable_key: String,
    pub kind: String,
    pub name: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub root: PathBuf,
    pub snapshot: String,
    pub results: Vec<SearchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexMeta {
    snapshot_id: String,
}

pub async fn search_project(options: SearchOptions) -> Result<SearchReport> {
    if options.query.trim().is_empty() {
        bail!("search query must not be empty");
    }

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );

    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;

    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|s| s.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))?;

    let index_dir = root.join(".athanor/generated/current/search");

    // Open or rebuild index if snapshot changed or index doesn't exist
    let index = get_or_build_search_index(&snapshot, &snapshot_id, &index_dir).await?;

    let results = index
        .search(SearchQuery {
            query: options.query,
            limit: options.limit,
        })
        .await
        .context("failed to query search index")?;

    let search_items = results
        .into_iter()
        .filter_map(|res| {
            let entity: Entity = serde_json::from_value(res.payload).ok()?;
            let kind = serde_json::to_value(&entity.kind)
                .ok()?
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| "unknown".to_string());
            Some(SearchItem {
                stable_key: entity.stable_key.0,
                kind,
                name: entity.name,
                score: res.score,
            })
        })
        .collect();

    Ok(SearchReport {
        root,
        snapshot: snapshot_id,
        results: search_items,
    })
}

pub async fn get_or_build_search_index(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
) -> Result<TantivySearchIndex> {
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
        if index_dir.exists() {
            let _ = fs::remove_dir_all(index_dir);
        }
        fs::create_dir_all(index_dir)?;

        let index = TantivySearchIndex::open_or_create(index_dir)
            .context("failed to create search index")?;

        for entity in &snapshot.entities {
            let body = entity_text(entity);
            index
                .index_document(SearchDocument {
                    id: entity.id.0.clone(),
                    title: entity.title.clone().unwrap_or_else(|| entity.name.clone()),
                    body,
                    payload: serde_json::to_value(entity)?,
                })
                .await
                .context("failed to index entity")?;
        }

        let meta = IndexMeta {
            snapshot_id: snapshot_id.to_string(),
        };
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    }

    TantivySearchIndex::open_or_create(index_dir).context("failed to open search index")
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
