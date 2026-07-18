use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;

use crate::config::ProjectConfig;
use crate::legacy_factory::{
    LegacyFactoryInstallError, install_once, require_installed,
};

use super::{AthanorStore, StoreFactory};

static STORE_FACTORY: OnceLock<StoreFactory> = OnceLock::new();

pub(super) fn try_install_store_factory(
    factory: StoreFactory,
) -> Result<(), LegacyFactoryInstallError> {
    install_once(&STORE_FACTORY, factory, "store")
}

pub(super) fn install_store_factory(factory: StoreFactory) {
    try_install_store_factory(factory).expect("conflicting legacy store factory installation");
}

pub(super) async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    let factory = require_installed(&STORE_FACTORY, "store").map_err(anyhow::Error::new)?;
    factory(root, config).await
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
