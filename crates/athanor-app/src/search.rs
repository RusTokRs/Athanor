use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CanonicalSnapshotStoreOperationExt, CoreResult,
    OperationContext, OperationContextCancellation, SearchIndex, SearchIndexOperationExt,
    SearchQuery, SearchResult,
};
use athanor_domain::Entity;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::json_contract::SEARCH_SCHEMA_V1;
use crate::project_path::normalize_canonical_path;

#[path = "search/index.rs"]
mod index;
#[path = "search/model.rs"]
mod model;

pub use index::entity_text;
pub(crate) use index::{
    get_or_build_search_index_with_factory, get_or_build_search_index_with_factory_and_operation,
};
pub use model::{
    SearchIndexFactory, SearchIndexOperationFactory, SearchItem, SearchMode, SearchOmissions,
    SearchOptions, SearchReport,
};

pub async fn search_project_with_composition(
    options: SearchOptions,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    search_project_inner(options, composition, None).await
}

pub async fn search_project_with_composition_and_operation_context(
    options: SearchOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    search_project_inner(options, composition, Some(operation)).await
}

async fn search_project_inner(
    options: SearchOptions,
    composition: &RuntimeComposition,
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
    let store = composition.init_store(&root, &config).await?;
    let snapshot = match operation {
        Some(operation) => {
            store
                .load_latest_snapshot_with_operation_context(operation)
                .await
        }
        None => store.load_latest_snapshot().await,
    }
    .context("failed to load latest canonical snapshot")?
    .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;

    match operation {
        Some(operation) => {
            search_snapshot_with_composition_and_operation_context(
                &root,
                &snapshot,
                options.query,
                options.limit,
                options.mode,
                composition,
                operation,
            )
            .await
        }
        None => {
            search_snapshot_with_composition(
                &root,
                &snapshot,
                options.query,
                options.limit,
                options.mode,
                composition,
            )
            .await
        }
    }
}

pub async fn search_snapshot_with_composition(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    mode: SearchMode,
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
    search_snapshot_with_index(root, snapshot, query, limit, mode, index.as_ref()).await
}

pub async fn search_snapshot_with_composition_and_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    mode: SearchMode,
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
                composition
                    .build_search_index_with_operation_context(directory, documents, operation)
            },
        )
    })
    .await
    .context("search index rebuild worker terminated unexpectedly")??;
    search_snapshot_with_index_inner(
        root,
        snapshot,
        query,
        limit,
        mode,
        index.as_ref(),
        Some(operation),
    )
    .await
}

pub async fn search_snapshot_with_index(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    mode: SearchMode,
    index: &dyn SearchIndex,
) -> Result<SearchReport> {
    search_snapshot_with_index_inner(root, snapshot, query, limit, mode, index, None).await
}

/// Execute a search query in the requested mode against an already-built index.
pub(crate) async fn execute_search(
    index: &dyn SearchIndex,
    query: SearchQuery,
    mode: SearchMode,
    operation: Option<&OperationContext>,
) -> CoreResult<Vec<SearchResult>> {
    match mode {
        SearchMode::Lexical => match operation {
            Some(operation) => index.search_with_operation_context(query, operation).await,
            None => index.search(query).await,
        },
        SearchMode::Semantic => match operation {
            Some(operation) => {
                index
                    .search_semantic_with_operation_context(query, operation)
                    .await
            }
            None => index.search_semantic(query).await,
        },
        SearchMode::Hybrid => {
            let limit = query.limit;
            let lexical = match operation {
                Some(operation) => {
                    index
                        .search_with_operation_context(query.clone(), operation)
                        .await?
                }
                None => index.search(query.clone()).await?,
            };
            let semantic = match operation {
                Some(operation) => {
                    index
                        .search_semantic_with_operation_context(query, operation)
                        .await?
                }
                None => index.search_semantic(query).await?,
            };
            Ok(merge_hybrid_results(lexical, semantic, limit))
        }
    }
}

/// Merge lexical (BM25) and semantic (cosine) result lists into one ranking.
/// Both score scales are min-max normalized to [0, 1] and combined with equal
/// weights; ties break by entity id for determinism.
pub(crate) fn merge_hybrid_results(
    lexical: Vec<SearchResult>,
    semantic: Vec<SearchResult>,
    limit: usize,
) -> Vec<SearchResult> {
    let lexical_max = lexical
        .iter()
        .map(|result| result.score)
        .fold(0.0f32, f32::max);
    let mut merged: HashMap<String, SearchResult> = HashMap::new();
    for result in lexical {
        let normalized = if lexical_max > 0.0 {
            (result.score / lexical_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let entry = merged
            .entry(result.id.clone())
            .or_insert_with(|| SearchResult {
                id: result.id.clone(),
                score: 0.0,
                payload: result.payload.clone(),
            });
        entry.score += 0.5 * normalized;
    }
    for result in semantic {
        let normalized = result.score.clamp(0.0, 1.0);
        let entry = merged
            .entry(result.id.clone())
            .or_insert_with(|| SearchResult {
                id: result.id.clone(),
                score: 0.0,
                payload: result.payload.clone(),
            });
        entry.score += 0.5 * normalized;
    }
    let mut results: Vec<SearchResult> = merged.into_values().collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    results.truncate(limit);
    results
}

async fn search_snapshot_with_index_inner(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    mode: SearchMode,
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
    let results = execute_search(index, search_query, mode, operation)
        .await
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
        mode,
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

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use athanor_core::SearchDocument;
    use serde_json::json;

    use super::*;

    struct StubIndex {
        lexical: Vec<SearchResult>,
        semantic: Vec<SearchResult>,
    }

    #[async_trait]
    impl SearchIndex for StubIndex {
        async fn index_document(&self, _doc: SearchDocument) -> CoreResult<()> {
            Ok(())
        }

        async fn remove_document(&self, _id: &str) -> CoreResult<()> {
            Ok(())
        }

        async fn search(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
            Ok(self.lexical.clone())
        }

        async fn search_semantic(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
            Ok(self.semantic.clone())
        }
    }

    struct LexicalOnlyIndex;

    #[async_trait]
    impl SearchIndex for LexicalOnlyIndex {
        async fn index_document(&self, _doc: SearchDocument) -> CoreResult<()> {
            Ok(())
        }

        async fn remove_document(&self, _id: &str) -> CoreResult<()> {
            Ok(())
        }

        async fn search(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
            Ok(vec![result("a", 1.0)])
        }
    }

    fn result(id: &str, score: f32) -> SearchResult {
        SearchResult {
            id: id.to_string(),
            score,
            payload: json!({ "id": id }),
        }
    }

    #[test]
    fn hybrid_merge_normalizes_and_combines_scores() {
        let lexical = vec![result("a", 2.0), result("b", 1.0)];
        let semantic = vec![result("b", 0.8), result("c", 0.6)];
        let merged = merge_hybrid_results(lexical, semantic, 10);
        assert_eq!(merged.len(), 3);
        // b: 0.5 * (1.0 / 2.0) + 0.5 * 0.8 = 0.65; a: 0.5 * 1.0 = 0.5; c: 0.5 * 0.6 = 0.3
        assert_eq!(merged[0].id, "b");
        assert!((merged[0].score - 0.65).abs() < 1e-6);
        assert_eq!(merged[1].id, "a");
        assert!((merged[1].score - 0.5).abs() < 1e-6);
        assert_eq!(merged[2].id, "c");
    }

    #[test]
    fn hybrid_merge_is_deterministic_and_respects_limit() {
        let lexical = vec![result("b", 1.0), result("a", 1.0)];
        let merged = merge_hybrid_results(lexical, Vec::new(), 1);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "a");
    }

    #[tokio::test]
    async fn execute_search_dispatches_by_mode() {
        let index = StubIndex {
            lexical: vec![result("a", 2.0)],
            semantic: vec![result("b", 0.9)],
        };
        let query = SearchQuery {
            query: "q".to_string(),
            limit: 5,
        };
        let lexical = execute_search(&index, query.clone(), SearchMode::Lexical, None)
            .await
            .unwrap();
        assert_eq!(lexical.len(), 1);
        assert_eq!(lexical[0].id, "a");
        let semantic = execute_search(&index, query.clone(), SearchMode::Semantic, None)
            .await
            .unwrap();
        assert_eq!(semantic.len(), 1);
        assert_eq!(semantic[0].id, "b");
        let hybrid = execute_search(&index, query, SearchMode::Hybrid, None)
            .await
            .unwrap();
        assert_eq!(hybrid.len(), 2);
    }

    #[tokio::test]
    async fn semantic_falls_back_to_lexical_for_plain_indexes() {
        let index = LexicalOnlyIndex;
        let query = SearchQuery {
            query: "q".to_string(),
            limit: 5,
        };
        let results = execute_search(&index, query, SearchMode::Semantic, None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");
    }
}
