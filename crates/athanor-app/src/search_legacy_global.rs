use std::sync::OnceLock;

use anyhow::Result;

use crate::legacy_factory::{LegacyFactoryInstallError, install_once};

use super::{SearchIndexFactory, SearchIndexOperationFactory};

static SEARCH_INDEX_FACTORY: OnceLock<SearchIndexFactory> = OnceLock::new();
static SEARCH_INDEX_OPERATION_FACTORY: OnceLock<SearchIndexOperationFactory> = OnceLock::new();

pub(super) fn try_install_search_index_factory(
    factory: SearchIndexFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(&SEARCH_INDEX_FACTORY, factory, "search index")
}

pub(super) fn install_search_index_factory(factory: SearchIndexFactory) {
    try_install_search_index_factory(factory)
        .expect("conflicting legacy search index factory installation");
}

pub(super) fn try_install_search_index_operation_factory(
    factory: SearchIndexOperationFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(
        &SEARCH_INDEX_OPERATION_FACTORY,
        factory,
        "operation-aware search index",
    )
}

pub(super) fn install_search_index_operation_factory(factory: SearchIndexOperationFactory) {
    try_install_search_index_operation_factory(factory)
        .expect("conflicting legacy operation-aware search index factory installation");
}

pub(super) fn search_index_factory() -> Option<SearchIndexFactory> {
    SEARCH_INDEX_FACTORY.get().copied()
}

pub(super) fn search_index_operation_factory() -> Option<SearchIndexOperationFactory> {
    SEARCH_INDEX_OPERATION_FACTORY.get().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_factory::require_installed;

    #[test]
    fn missing_search_factory_is_typed() {
        let slot = OnceLock::<SearchIndexFactory>::new();
        let error = require_installed(&slot, "search index").unwrap_err();
        assert_eq!(error.factory(), "search index");
    }
}
