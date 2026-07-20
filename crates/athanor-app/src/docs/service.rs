//! Service-layer orchestration for documentation operations.
//!
//! Coordinates between the docs domain (check, drift, proposal) and the
//! canonical snapshot store, keeping higher-level callers free of direct
//! store access.

use anyhow::{Context, Result};
use athanor_core::CanonicalSnapshotStore;

use crate::composition::RuntimeComposition;
use crate::config::load_config;
use crate::project_path::normalize_canonical_path;

use super::{
    DocsApplyPatchOptions, DocsApplyPatchReport, DocsCheckOptions, DocsCheckReport,
    DocsDriftOptions, DocsDriftReport, DocsProposeFixOptions, DocsProposeFixReport,
};

/// Runs the docs check operation against the latest canonical snapshot.
pub(crate) async fn check(
    options: DocsCheckOptions,
    composition: &RuntimeComposition,
) -> Result<DocsCheckReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
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
    let snapshot = canonical
        .snapshot
        .as_ref()
        .map_or_else(|| "unknown".to_string(), |s| s.0.clone());
    Ok(super::check::build_docs_check_report(
        snapshot,
        &canonical.entities,
        &canonical.diagnostics,
        &config.docs,
    ))
}

/// Runs the docs drift operation against the latest canonical snapshot.
pub(crate) async fn drift(
    options: DocsDriftOptions,
    composition: &RuntimeComposition,
) -> Result<DocsDriftReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
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
    Ok(super::check::build_docs_drift_report(
        &root,
        canonical,
        &config.docs,
    ))
}

/// Builds a patch proposal for documentation drift.
pub(crate) async fn propose(
    options: DocsProposeFixOptions,
    composition: &RuntimeComposition,
) -> Result<DocsProposeFixReport> {
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
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
    super::proposal::build_docs_patch_proposal_from_snapshot(
        options.output.as_deref(),
        canonical,
        &config.docs,
    )
}

/// Applies a previously generated documentation patch.
pub(crate) async fn apply(
    options: DocsApplyPatchOptions,
    _composition: &RuntimeComposition,
) -> Result<DocsApplyPatchReport> {
    super::frontmatter::apply_frontmatter_changes(&options.root, &options.patch)
}
