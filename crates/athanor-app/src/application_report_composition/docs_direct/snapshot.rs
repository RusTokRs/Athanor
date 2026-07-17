use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};

use crate::composition::RuntimeComposition;
use crate::config::{ProjectConfig, load_config};
use crate::project_path::normalize_canonical_path;

pub(super) async fn load(
    root: &Path,
    composition: &RuntimeComposition,
) -> Result<(CanonicalSnapshot, ProjectConfig)> {
    let root = canonical_root(root)?;
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let canonical = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;
    Ok((canonical, config))
}

pub(super) fn id(canonical: &CanonicalSnapshot) -> String {
    canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |snapshot| snapshot.0.clone())
}

pub(super) fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}
