use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{OperationContext, SearchDocument, SearchIndex};
use athanor_domain::{EntityId, Ownership, SourceLocation};
use serde::{Deserialize, Serialize};

pub type SearchIndexFactory =
    fn(&Path, Option<Vec<SearchDocument>>) -> Result<Arc<dyn SearchIndex>>;
pub type SearchIndexOperationFactory =
    fn(&Path, Option<Vec<SearchDocument>>, &OperationContext) -> Result<Arc<dyn SearchIndex>>;

/// Bump when the on-disk search index layout changes; stale indexes are rebuilt.
pub(crate) const SEARCH_INDEX_FORMAT: u32 = 2;

/// Retrieval mode for `search`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// BM25 lexical search (Tantivy) — the default.
    #[default]
    Lexical,
    /// Semantic retrieval over local hashing embeddings (cosine similarity).
    Semantic,
    /// Both lexical and semantic retrieval, merged into one ranked list.
    Hybrid,
}

impl std::fmt::Display for SearchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            SearchMode::Lexical => "lexical",
            SearchMode::Semantic => "semantic",
            SearchMode::Hybrid => "hybrid",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub root: PathBuf,
    pub query: String,
    pub limit: usize,
    pub mode: SearchMode,
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
    pub mode: SearchMode,
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
pub(super) struct IndexMeta {
    pub(super) snapshot_id: String,
    #[serde(default)]
    pub(super) search_format: u32,
}
