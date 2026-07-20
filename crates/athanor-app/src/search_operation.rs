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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use athanor_core::{CancellationHandle, CoreError, CoreResult, SearchDocument, SearchResult};
    use athanor_domain::SnapshotId;

    use super::*;

    struct CancellingIndex {
        cancellation: Mutex<Option<CancellationHandle>>,
    }

    #[async_trait]
    impl SearchIndex for CancellingIndex {
        async fn index_document(&self, _doc: SearchDocument) -> CoreResult<()> {
            Ok(())
        }

        async fn remove_document(&self, _id: &str) -> CoreResult<()> {
            Ok(())
        }

        async fn search(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
            self.cancellation
                .lock()
                .expect("search cancellation fixture lock")
                .take()
                .expect("search cancellation handle")
                .cancel();
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn mid_flight_cancellation_does_not_return_successful_report() {
        let operation = OperationContext::new("search.mid-flight");
        let cancellation = operation.cancellation_handle().unwrap();
        let index = CancellingIndex {
            cancellation: Mutex::new(Some(cancellation)),
        };
        let snapshot = CanonicalSnapshot {
            snapshot: Some(SnapshotId("snap_test".to_string())),
            ..CanonicalSnapshot::default()
        };

        let error = search_snapshot_with_index_and_operation_context(
            Path::new("."),
            &snapshot,
            "entity".to_string(),
            10,
            &index,
            &operation,
        )
        .await
        .expect_err("cancelled search must not return a report");

        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<CoreError>(),
            Some(CoreError::Cancelled(_))
        )));
    }
}
