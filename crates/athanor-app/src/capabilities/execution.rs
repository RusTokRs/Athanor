use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::index_state::IndexStateStore;
use crate::project_path::normalize_canonical_path;

use super::aggregation::build_capabilities_report;
use super::model::{CapabilitiesOptions, CapabilitiesReport};

/// Builds an analysis-completeness report with explicitly supplied runtime dependencies.
pub async fn capabilities_project_with_composition(
    options: CapabilitiesOptions,
    composition: &RuntimeComposition,
) -> Result<CapabilitiesReport> {
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
    let state = IndexStateStore::new(root.join(".athanor/state/index-state.json"))
        .load()
        .context("failed to load index state")?;

    Ok(build_capabilities_report(
        root,
        snapshot,
        state,
        options.limit,
        options.confidence_threshold,
    ))
}
