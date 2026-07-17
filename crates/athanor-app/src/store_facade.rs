//! Guarded facade for the legacy process-global store factory.
//!
//! The implementation remains in `store.rs` while public installation and
//! lookup paths expose typed conflict/unavailable failures. New production
//! code should use `RuntimeComposition::init_store` instead.

use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;

use crate::config::ProjectConfig;
use crate::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError, install_once, require_installed,
};

#[path = "store.rs"]
mod legacy;

pub use legacy::{AthanorStore, StoreFactory};

static STORE_FACTORY_GUARD: OnceLock<StoreFactory> = OnceLock::new();

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

pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    #[cfg(test)]
    crate::ensure_test_runtime();

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
}
