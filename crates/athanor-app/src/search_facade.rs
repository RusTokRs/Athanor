//! Compatibility facade for optional process-global search-index factories.
//!
//! Explicit `RuntimeComposition` entrypoints are the default production path.
//! The `legacy-global-runtime` feature retains no-composition wrappers during
//! the compatibility window; default builds contain no Search factory state.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use athanor_core::{CanonicalSnapshot, OperationContext, SearchIndex};

use crate::legacy_factory::{LegacyFactoryInstallError, LegacyFactoryUnavailableError};
use crate::RuntimeComposition;

#[path = "search.rs"]
mod core;
#[cfg(any(feature = "legacy-global-runtime", test))]
#[path = "search_legacy_global.rs"]
mod legacy_global;
#[cfg(not(any(feature = "legacy-global-runtime", test)))]
mod legacy_global {
    use super::core::{SearchIndexFactory, SearchIndexOperationFactory};

    pub(super) fn search_index_factory() -> Option<SearchIndexFactory> {
        None
    }

    pub(super) fn search_index_operation_factory() -> Option<SearchIndexOperationFactory> {
        None
    }
}

pub use core::{
    SearchIndexFactory, SearchIndexOperationFactory, SearchItem, SearchOmissions, SearchOptions,
    SearchReport, entity_text,
};
pub(crate) use core::{
    get_or_build_search_index_with_factory,
    get_or_build_search_index_with_factory_and_operation,
};

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_search_index_factory(
    factory: SearchIndexFactory,
) -> Result<(), LegacyFactoryInstallError> {
    legacy_global::try_install_search_index_factory(factory)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_search_index_factory(
    _factory: SearchIndexFactory,
) -> Result<(), LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_search_index_factory(factory: SearchIndexFactory) {
    legacy_global::install_search_index_factory(factory);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_search_index_factory(_factory: SearchIndexFactory) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_search_index_operation_factory(
    factory: SearchIndexOperationFactory,
) -> Result<(), LegacyFactoryInstallError> {
    legacy_global::try_install_search_index_operation_factory(factory)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_search_index_operation_factory(
    _factory: SearchIndexOperationFactory,
) -> Result<(), LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_search_index_operation_factory(factory: SearchIndexOperationFactory) {
    legacy_global::install_search_index_operation_factory(factory);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_search_index_operation_factory(_factory: SearchIndexOperationFactory) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

fn require_legacy_search_index_factory(
) -> Result<SearchIndexFactory, LegacyFactoryUnavailableError> {
    legacy_global::search_index_factory().ok_or_else(|| {
        LegacyFactoryUnavailableError::new(
            "search index (legacy-global-runtime disabled or not installed)",
        )
    })
}

fn require_any_search_factory() -> Result<(), LegacyFactoryUnavailableError> {
    if legacy_global::search_index_operation_factory().is_some()
        || legacy_global::search_index_factory().is_some()
    {
        Ok(())
    } else {
        Err(LegacyFactoryUnavailableError::new(
            "search index (legacy-global-runtime disabled or not installed)",
        ))
    }
}

pub async fn search_project(options: SearchOptions) -> Result<SearchReport> {
    #[cfg(test)]
    crate::ensure_test_runtime();
    require_legacy_search_index_factory().map_err(anyhow::Error::new)?;
    core::search_project(options).await
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
    core::search_project_with_composition_and_operation_context(
        options,
        composition,
        operation,
    )
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

pub async fn search_snapshot(
    root: &Path,
    snapshot: &CanonicalSnapshot,
    query: String,
    limit: usize,
) -> Result<SearchReport> {
    #[cfg(test)]
    crate::ensure_test_runtime();
    require_legacy_search_index_factory().map_err(anyhow::Error::new)?;
    core::search_snapshot(root, snapshot, query, limit).await
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
    core::search_snapshot_with_operation_context(root, snapshot, query, limit, operation).await
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
    #[cfg(test)]
    crate::ensure_test_runtime();
    require_legacy_search_index_factory().map_err(anyhow::Error::new)?;
    core::get_or_build_search_index(snapshot, snapshot_id, index_dir).await
}

pub fn get_or_build_search_index_sync(
    snapshot: &CanonicalSnapshot,
    snapshot_id: &str,
    index_dir: &Path,
) -> Result<Arc<dyn SearchIndex>> {
    #[cfg(test)]
    crate::ensure_test_runtime();
    require_legacy_search_index_factory().map_err(anyhow::Error::new)?;
    core::get_or_build_search_index_sync(snapshot, snapshot_id, index_dir)
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
    core::get_or_build_search_index_with_operation_context(
        snapshot,
        snapshot_id,
        index_dir,
        operation,
    )
}
