//! Compatibility facade for search APIs that historically used process-global factories.
//!
//! Composition-aware APIs are the production path. Unit tests retain the old helper shape by
//! constructing a fresh local test composition; production no-composition calls fail explicitly.

use std::path::Path;
use std::sync::Arc;

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

pub fn install_search_index_factory(_factory: SearchIndexFactory) {
    panic!("process-global search-index installation was removed; use RuntimeComposition")
}

pub fn install_search_index_operation_factory(_factory: SearchIndexOperationFactory) {
    panic!("process-global search-index installation was removed; use RuntimeComposition")
}

pub async fn search_project(options: SearchOptions) -> Result<SearchReport> {
    #[cfg(test)]
    {
        let composition = crate::test_runtime::composition();
        return core::search_project_with_composition(options, &composition).await;
    }

    #[cfg(not(test))]
    {
        let _ = options;
        bail!("explicit RuntimeComposition is required for project search")
    }
}

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

pub async fn search_snapshot(
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

pub async fn search_snapshot_with_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    operation: &OperationContext,
) -> Result<SearchReport> {
    #[cfg(test)]
    {
        let composition = crate::test_runtime::composition();
        return core::search_snapshot_with_composition_and_operation_context(
            root,
            snapshot,
            query,
            limit,
            &composition,
            operation,
        )
        .await;
    }

    #[cfg(not(test))]
    {
        let _ = (root, snapshot, query, limit, operation);
        bail!("explicit RuntimeComposition is required for operation-aware snapshot search")
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
    #[cfg(test)]
    {
        let composition = crate::test_runtime::composition();
        return core::get_or_build_search_index_with_factory(
            snapshot,
            snapshot_id,
            index_dir,
            |directory, documents| composition.build_search_index(directory, documents),
        );
    }

    #[cfg(not(test))]
    {
        let _ = (snapshot, snapshot_id, index_dir);
        bail!("explicit RuntimeComposition is required to build a search index")
    }
}

pub fn get_or_build_search_index_with_operation_context(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    operation: &OperationContext,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    {
        let composition = crate::test_runtime::composition();
        return core::get_or_build_search_index_with_factory_and_operation(
            snapshot,
            snapshot_id,
            index_dir,
            operation,
            |directory, documents, operation| {
                composition.build_search_index_with_operation_context(
                    directory,
                    documents,
                    operation,
                )
            },
        );
    }

    #[cfg(not(test))]
    {
        let _ = (snapshot, snapshot_id, index_dir, operation);
        bail!("explicit RuntimeComposition is required for operation-aware search indexing")
    }
}
