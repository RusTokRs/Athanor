//! Guarded facade for the legacy process-global store factory.
//!
//! The implementation remains in `store.rs` while public installation and
//! lookup paths expose typed conflict/unavailable failures. New production
//! code should use `RuntimeComposition::init_store` directly. Compatibility
//! application APIs may use the task-local composition scope while migrating.

use std::future::Future;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;

use crate::composition::RuntimeComposition;
use crate::config::ProjectConfig;
use crate::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError, install_once, require_installed,
};

#[path = "store.rs"]
mod legacy;

pub use legacy::{AthanorStore, StoreFactory};

static STORE_FACTORY_GUARD: OnceLock<StoreFactory> = OnceLock::new();

tokio::task_local! {
    static SCOPED_STORE_COMPOSITION: RuntimeComposition;
}

pub fn try_install_store_factory(
    factory: StoreFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(&STORE_FACTORY_GUARD, factory, "store")?;
    legacy::install_store_factory(factory);
    Ok(())
}

pub fn install_store_factory(factory: StoreFactory) {
    try_install_store_factory(factory)
        .expect("conflicting legacy store factory installation");
}

/// Runs a compatibility future with an explicit task-local store composition.
///
/// This bridge avoids process-global installation while legacy application
/// functions are incrementally migrated to accept `RuntimeComposition`.
pub async fn with_store_composition<F>(
    composition: RuntimeComposition,
    future: F,
) -> F::Output
where
    F: Future,
{
    SCOPED_STORE_COMPOSITION.scope(composition, future).await
}

fn has_scoped_store_composition() -> bool {
    SCOPED_STORE_COMPOSITION.try_with(|_| ()).is_ok()
}

pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    #[cfg(test)]
    crate::ensure_test_runtime();

    if let Ok(composition) = SCOPED_STORE_COMPOSITION.try_with(Clone::clone) {
        return composition.init_store(root, config).await;
    }

    require_installed(&STORE_FACTORY_GUARD, "store")
        .map_err(anyhow::Error::new)?;
    legacy::init_store(root, config).await
}

pub fn require_legacy_store_factory() -> Result<StoreFactory, LegacyFactoryUnavailableError> {
    require_installed(&STORE_FACTORY_GUARD, "store").copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_store_factory_is_typed() {
        let slot = OnceLock::<StoreFactory>::new();
        let error = require_installed(&slot, "store").unwrap_err();
        assert_eq!(error.factory(), "store");
    }

    #[tokio::test]
    async fn concurrent_store_composition_scopes_do_not_leak() {
        assert!(!has_scoped_store_composition());
        let composition = crate::test_runtime::composition();

        let first = tokio::spawn(with_store_composition(composition.clone(), async {
            tokio::task::yield_now().await;
            has_scoped_store_composition()
        }));
        let second = tokio::spawn(with_store_composition(composition, async {
            tokio::task::yield_now().await;
            has_scoped_store_composition()
        }));

        assert!(first.await.unwrap());
        assert!(second.await.unwrap());
        assert!(!has_scoped_store_composition());
    }
}
