//! Composition-aware exact-snapshot documentation completeness reporting.
//!
//! Slice 6B resolves one project, initializes its configured canonical Store, loads only the
//! committed snapshot named by the request, and delegates to the pure Slice 6A completeness owner.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::SnapshotId;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::{
    CancellationToken, DocumentationCompletenessReport, DocumentationCompletenessRequest,
    RuntimeComposition, build_documentation_completeness_report,
};

#[derive(Debug, Clone)]
pub struct DocumentationCompletenessOperationOptions {
    pub root: PathBuf,
    pub request: DocumentationCompletenessRequest,
}

pub async fn documentation_completeness_with_composition(
    options: DocumentationCompletenessOperationOptions,
    composition: &RuntimeComposition,
) -> Result<DocumentationCompletenessReport> {
    documentation_completeness_with_composition_inner(options, composition, None).await
}

pub async fn documentation_completeness_with_composition_cancellable(
    options: DocumentationCompletenessOperationOptions,
    composition: &RuntimeComposition,
    cancellation: CancellationToken,
) -> Result<DocumentationCompletenessReport> {
    documentation_completeness_with_composition_inner(options, composition, Some(cancellation)).await
}

async fn documentation_completeness_with_composition_inner(
    options: DocumentationCompletenessOperationOptions,
    composition: &RuntimeComposition,
    cancellation: Option<CancellationToken>,
) -> Result<DocumentationCompletenessReport> {
    check_cancelled(&cancellation)?;
    options
        .request
        .validate()
        .map_err(anyhow::Error::msg)
        .context("invalid documentation completeness request")?;

    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    check_cancelled(&cancellation)?;

    let expected = SnapshotId(options.request.snapshot.clone());
    let snapshot = store
        .load_snapshot(&expected)
        .await
        .with_context(|| format!("failed to load canonical snapshot {}", expected.0))?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "canonical snapshot {} is not committed or does not exist; run `ath index {}` first",
                expected.0,
                root.display()
            )
        })?;
    if snapshot.snapshot.as_ref() != Some(&expected) {
        bail!(
            "canonical Store returned snapshot identity {:?}, expected {}",
            snapshot.snapshot,
            expected.0
        );
    }
    check_cancelled(&cancellation)?;

    build_documentation_completeness_report(&options.request, &snapshot).map_err(anyhow::Error::msg)
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}
