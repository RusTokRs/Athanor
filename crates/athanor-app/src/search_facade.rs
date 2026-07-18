//! Composition-first public facade for Search APIs.
//!
//! The remaining no-composition snapshot helper is crate-private and exists only for the isolated
//! legacy ChangeMap core. External callers must supply `RuntimeComposition`.

use std::path::Path;

use anyhow::Result;
#[cfg(not(test))]
use anyhow::bail;
use athanor_core::{CanonicalSnapshot, OperationContext, SearchIndex};

use crate::RuntimeComposition;

#[path = "search.rs"]
mod core;

pub use core::{
    SearchIndexFactory, SearchIndexOperationFactory, SearchItem, SearchOmissions, SearchOptions,
    SearchReport, entity_text,
};
pub(crate) use core::{
    get_or_build_search_index_with_factory,
    get_or_build_search_index_with_factory_and_operation,
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
    core::search_project_with_composition_and_operation_context(options, composition, operation).await
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

pub(crate) async fn search_snapshot(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
) -> Result<SearchReport> {
    #[cfg(test)]
    {
        let composition = crate::test_runtime::composition();
        return core::search_snapshot_with_composition(
            root,
            snapshot,
            query,
            limit,
            &composition,
        )
        .await;
    }

    #[cfg(not(test))]
    {
        let _ = (root, snapshot, query, limit);
        bail!("explicit RuntimeComposition is required for snapshot search")
    }
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
