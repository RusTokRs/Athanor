use std::path::Path;

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;

use crate::RuntimeComposition;
use crate::config::load_config;
use crate::index_state::IndexStateStore;
use crate::project_path::normalize_canonical_path;

use super::aggregation::build_coverage_report;
use super::model::{CoverageOptions, CoverageReport};

/// Builds a coverage report with explicitly supplied runtime dependencies.
pub async fn coverage_project_with_composition(
    options: CoverageOptions,
    composition: &RuntimeComposition,
) -> Result<CoverageReport> {
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
    let file_filter = options
        .file
        .as_deref()
        .map(|path| relative_filter_path(&root, path))
        .transpose()?;

    Ok(build_coverage_report(
        root,
        snapshot,
        state,
        options.adapter,
        file_filter,
        options.limit,
    ))
}

fn relative_filter_path(root: &Path, path: &Path) -> Result<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let normalized = normalize_canonical_path(absolute);
    let relative = normalized
        .strip_prefix(root)
        .with_context(|| format!("coverage file filter must stay under {}", root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}
