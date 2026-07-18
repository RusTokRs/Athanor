use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::project_path::normalize_canonical_path;

use super::aggregation::build_repository_overview;
use super::model::{OverviewOptions, RepositoryOverview};

/// Builds an overview with explicitly supplied runtime dependencies.
pub async fn overview_project_with_composition(
    options: OverviewOptions,
    composition: &RuntimeComposition,
) -> Result<RepositoryOverview> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    let snapshot = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no canonical snapshot found; run `ath index {}` first",
                root.display()
            )
        })?;

    Ok(build_repository_overview(&snapshot, options.top.max(1)))
}
