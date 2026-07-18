use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, OperationContext, OperationContextCancellation, SearchDocument, SearchIndex,
};
use athanor_domain::Entity;

use super::model::IndexMeta;
use super::{check_active, legacy_search_index_factory, legacy_search_index_operation_factory};

const SEARCH_REBUILD_POLL_DOCUMENTS: usize = 256;

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
    let Some(factory) = legacy_search_index_factory() else {
        bail!("legacy search index is disabled; use RuntimeComposition::build_search_index");
    };
    get_or_build_search_index_with_factory(snapshot, snapshot_id, index_dir, factory)
}

pub fn get_or_build_search_index_with_operation_context(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    operation: &OperationContext,
) -> Result<Arc<dyn SearchIndex>> {
    if let Some(factory) = legacy_search_index_operation_factory() {
        return get_or_build_search_index_with_factory_and_operation(
            snapshot,
            snapshot_id,
            index_dir,
            operation,
            factory,
        );
    }
    let Some(factory) = legacy_search_index_factory() else {
        bail!(
            "legacy operation-aware search index is disabled; use RuntimeComposition::build_search_index_with_operation_context"
        );
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
