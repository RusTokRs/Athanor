//! Compatibility facade for the optional process-global store factory.
//!
//! Active application entrypoints use `RuntimeComposition::init_store` directly.
//! The `legacy-global-runtime` feature retains the old no-composition API during
//! the compatibility window; default production builds contain no Store factory state.

use std::path::Path;

use anyhow::Result;
#[cfg(not(any(feature = "legacy-global-runtime", test)))]
use anyhow::bail;

use crate::config::ProjectConfig;
use crate::legacy_factory::LegacyFactoryInstallError;

#[path = "store.rs"]
mod core;
#[cfg(any(feature = "legacy-global-runtime", test))]
#[path = "store_legacy_global.rs"]
mod legacy_global;

pub use core::{AthanorStore, StoreFactory};

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn try_install_store_factory(
    factory: StoreFactory,
) -> Result<(), LegacyFactoryInstallError> {
    legacy_global::try_install_store_factory(factory)
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn try_install_store_factory(
    _factory: StoreFactory,
) -> Result<(), LegacyFactoryInstallError> {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub fn install_store_factory(factory: StoreFactory) {
    legacy_global::install_store_factory(factory);
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub fn install_store_factory(_factory: StoreFactory) {
    panic!("legacy-global-runtime feature is disabled; use RuntimeComposition")
}

#[cfg(any(feature = "legacy-global-runtime", test))]
pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    #[cfg(test)]
    crate::ensure_test_runtime();
    legacy_global::init_store(root, config).await
}

#[cfg(not(any(feature = "legacy-global-runtime", test)))]
pub async fn init_store(_root: &Path, _config: &ProjectConfig) -> Result<AthanorStore> {
    bail!("legacy Store initialization is disabled; use RuntimeComposition::init_store")
}
