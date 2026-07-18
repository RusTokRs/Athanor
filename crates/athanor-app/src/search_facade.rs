//! Guarded facade for legacy process-global search-index factories.
//!
//! Explicit `RuntimeComposition` entrypoints bypass this compatibility boundary.
//! Factory lookup remains private so new callers cannot couple themselves to the
//! process-global installation state.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use athanor_core::{CanonicalSnapshot, OperationContext, SearchIndex};

use crate::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError, install_once, require_installed,
};
use crate::RuntimeComposition;

#[path = "search.rs"]
mod legacy;

pub use legacy::{
    SearchIndexFactory, SearchIndexOperationFactory, SearchItem, SearchOmissions, SearchOptions,
    SearchReport, entity_text,
};
pub(crate) use legacy::{
    get_or_build_search_index_with_factory,
    get_or_build_search_index_with_factory_and_operation,
};

static SEARCH_INDEX_FACTORY_GUARD: OnceLock<SearchIndexFactory> = OnceLock::new();
static SEARCH_INDEX_OPERATION_FACTORY_GUARD: OnceLock<SearchIndexOperationFactory> = OnceLock::new();

pub fn try_install_search_index_factory(
    factory: SearchIndexFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(&SEARCH_INDEX_FACTORY_GUARD, factory, "search index")?;
    legacy::install_search_index_factory(factory);
    Ok(())
}

pub fn install_search_index_factory(factory: SearchIndexFactory) {
    try_install_search_index_factory(factory)
        .expect("conflicting legacy search index factory installation");
}

pub fn try_install_search_index_operation_factory(
    factory: SearchIndexOperationFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(
        &SEARCH_INDEX_OPERATION_FACTORY_GUARD,
        factory,
        "operation-aware search index",
    )?;
    legacy::install_search_index_operation_factory(factory);
    Ok(())
}

pub fn install_search_index_operation_factory(factory: SearchIndexOperationFactory) {
    try_install_search_index_operation_factory(factory)
        .expect("conflicting legacy operation-aware search index factory installation");
}

fn require_search_index_factory() -> Result<SearchIndexFactory, LegacyFactoryUnavailableError> {
    require_installed(&SEARCH_INDEX_FACTORY_GUARD, "search index").copied()
}

fn require_any_search_factory() -> Result<(), LegacyFactoryUnavailableError> {
    if SEARCH_INDEX_OPERATION_FACTORY_GUARD.get().is_some()
        || SEARCH_INDEX_FACTORY_GUARD.get().is_some()
    {
        Ok(())
    } else {
        Err(LegacyFactoryUnavailableError::new("search index"))
    }
}

pub async fn search_project(options: SearchOptions) -> Result<SearchReport> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    require_search_index_factory().map_err(anyhow::Error::new)?;
    legacy::search_project(options).await
}

pub async fn search_project_with_composition(
    options: SearchOptions,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    legacy::search_project_with_composition(options, composition).await
}

pub async fn search_project_with_composition_and_operation_context(
    options: SearchOptions,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    legacy::search_project_with_composition_and_operation_context(options, composition, operation)
        .await
}

pub async fn search_snapshot_with_composition(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    composition: &RuntimeComposition,
) -> Result<SearchReport> {
    legacy::search_snapshot_with_composition(root, snapshot, query, limit, composition).await
}

pub async fn search_snapshot_with_composition_and_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    composition: &RuntimeComposition,
    operation: &OperationContext,
) -> Result<SearchReport> {
    legacy::search_snapshot_with_composition_and_operation_context(
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
    crate::ensure_test_runtime();

    require_search_index_factory().map_err(anyhow::Error::new)?;
    legacy::search_snapshot(root, snapshot, query, limit).await
}

pub async fn search_snapshot_with_operation_context(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    operation: &OperationContext,
) -> Result<SearchReport> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    require_any_search_factory().map_err(anyhow::Error::new)?;
    legacy::search_snapshot_with_operation_context(root, snapshot, query, limit, operation).await
}

pub async fn search_snapshot_with_index(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
    index: &dyn SearchIndex,
) -> Result<SearchReport> {
    legacy::search_snapshot_with_index(root, snapshot, query, limit, index).await
}

pub async fn get_or_build_search_index(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    require_search_index_factory().map_err(anyhow::Error::new)?;
    legacy::get_or_build_search_index(snapshot, snapshot_id, index_dir).await
}

pub fn get_or_build_search_index_sync(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    require_search_index_factory().map_err(anyhow::Error::new)?;
    legacy::get_or_build_search_index_sync(snapshot, snapshot_id, index_dir)
}

pub fn get_or_build_search_index_with_operation_context(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
    operation: &OperationContext,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    require_any_search_factory().map_err(anyhow::Error::new)?;
    legacy::get_or_build_search_index_with_operation_context(
        snapshot,
        snapshot_id,
        index_dir,
        operation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_search_factory_is_typed() {
        let slot = OnceLock::<SearchIndexFactory>::new();
        let error = require_installed(&slot, "search index").unwrap_err();
        assert_eq!(error.factory(), "search index");
    }
}
