use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, OperationContext, OperationContextCancellation, SearchIndex,
    SearchIndexOperationExt, SearchQuery,
};
use athanor_domain::Entity;

use crate::search::{SearchItem, SearchOmissions, SearchReport};

/// Queries an already-built index under the shared operation cancellation/deadline contract.
pub async fn search_snapshot_with_index_and_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    index: &dyn SearchIndex,
    operation: &OperationContext,
) -> Result<SearchReport> {
    operation.check_active().map_err(anyhow::Error::new)?;
    if query.trim().is_empty() {
        bail!("search query must not be empty");
    }
    if limit == 0 {
        bail!("search limit must be greater than zero");
    }

    let snapshot_id = snapshot
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.0.clone())
        .ok_or_else(|| anyhow::anyhow!("latest canonical snapshot has no snapshot id"))?;
    let results = index
        .search_with_operation_context(
            SearchQuery {
                query: query.clone(),
                limit: limit.saturating_add(1),
            },
            operation,
        )
        .await
        .context("failed to query search index")?;
    let truncated = results.len() > limit;

    let search_items = results
        .into_iter()
        .take(limit)
        .filter_map(|result| {
            let entity: Entity = serde_json::from_value(result.payload).ok()?;
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
                score: result.score,
            })
        })
        .collect::<Vec<_>>();
    operation.check_active().map_err(anyhow::Error::new)?;
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
