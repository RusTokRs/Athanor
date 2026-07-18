//! Compatibility facade for the former process-global store factory.
//!
//! Active application entrypoints use `RuntimeComposition::init_store` directly.
//! Unit tests retain the old helper shape by creating a fresh local test composition.

use std::path::Path;

use anyhow::Result;
#[cfg(not(test))]
use anyhow::bail;

use crate::config::ProjectConfig;

#[path = "store.rs"]
mod core;

pub use core::{AthanorStore, StoreFactory};

pub fn install_store_factory(_factory: StoreFactory) {
    panic!("process-global Store installation was removed; use RuntimeComposition")
}

pub async fn init_store(root: &Path, config: &ProjectConfig) -> Result<AthanorStore> {
    #[cfg(test)]
    {
        return crate::test_runtime::composition()
            .init_store(root, config)
            .await;
    }

    #[cfg(not(test))]
    {
        let _ = (root, config);
        bail!("explicit RuntimeComposition is required for Store initialization")
    }
}
