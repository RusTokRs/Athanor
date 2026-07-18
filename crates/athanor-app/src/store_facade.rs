//! Compatibility facade for Store APIs that still require migration to explicit composition.
//!
//! Active application entrypoints use `RuntimeComposition::init_store` directly. Unit tests retain
//! the old helper shape temporarily by creating a fresh local test composition.

use std::path::Path;

use anyhow::Result;
#[cfg(not(test))]
use anyhow::bail;

use crate::config::ProjectConfig;

#[path = "store.rs"]
mod core;

pub use core::{AthanorStore, StoreFactory};

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
