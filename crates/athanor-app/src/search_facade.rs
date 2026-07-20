//! Composition-only public facade for Search APIs.
//!
//! External callers and internal read services must supply `RuntimeComposition`.

use std::path::Path;

use anyhow::Result;
use athanor_core::{CanonicalSnapshot, OperationContext, SearchIndex};

use crate::RuntimeComposition;

#[path = "search.rs"]
mod core;

pub use core::{
    SearchIndexFactory, SearchIndexOperationFactory, SearchItem, SearchOmissions, SearchOptions,
    SearchReport, entity_text,
};
pub(crate) use core::{
    get_or_build_search_index_with_factory, get_or_build_search_index_with_factory_and_operation,
};

pub async fn search_project_with_composition(
    options: SearchOptions,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    core::search_project_with_composition(options, composition).await
}

pub async fn search_project_with_composition_and_operation_context(
    options: SearchOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    core::search_project_with_composition_and_operation_context(options, composition, operation)
        .await
}

pub async fn search_snapshot_with_composition(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    core::search_snapshot_with_composition(root, snapshot, query, limit, composition).await
}

pub async fn search_snapshot_with_composition_and_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    core::search_snapshot_with_composition_and_operation_context(
        root,
        snapshot,
        query,
        limit,
        composition,
        operation,
    )
    .await
}

pub async fn search_snapshot_with_index(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    index: &dyn SearchIndex,
) -> Result<SearchReport> {
    core::search_snapshot_with_index(root, snapshot, query, limit, index).await
}
