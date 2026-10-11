//! Mode dispatch and hybrid ranking for snapshot search.
//!
//! Lexical, semantic, and hybrid retrieval stay behind one entry point so the
//! snapshot search paths never branch on the index implementation themselves.

use std::cmp::Ordering;
use std::collections::HashMap;

use athanor_core::{
    CoreResult, OperationContext, SearchIndex, SearchIndexOperationExt, SearchQuery, SearchResult,
};

use super::SearchMode;

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

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use athanor_core::{CoreResult, SearchDocument, SearchIndex, SearchQuery, SearchResult};
    use serde_json::json;

    use super::{SearchMode, execute_search, merge_hybrid_results};

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
