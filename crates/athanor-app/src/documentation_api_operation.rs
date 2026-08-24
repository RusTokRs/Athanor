//! Composition-aware deterministic API documentation generation.
//!
//! This owner resolves one project, initializes its configured canonical Store, loads the exact
//! committed snapshot named by the request, and delegates immutable API publication. CLI, daemon,
//! and MCP parsing remain outside this module.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::SnapshotId;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::{
    CancellationToken, DocumentationApiPublicationOptions, DocumentationApiPublicationReport,
    DocumentationGenerationRequest, DocumentationProfile, RuntimeComposition,
    publish_documentation_api_generation, publish_documentation_api_generation_cancellable,
};

#[derive(Debug, Clone)]
pub struct DocumentationApiOperationOptions {
    pub root: PathBuf,
    pub request: DocumentationGenerationRequest,
    pub force: bool,
}

/// Loads the exact committed snapshot through explicit runtime composition and publishes it.
pub async fn generate_documentation_api_with_composition(
    options: DocumentationApiOperationOptions,
    composition: &RuntimeComposition,
) -> Result<DocumentationApiPublicationReport> {
    generate_documentation_api_with_composition_inner(options, composition, None).await
}

/// Loads and publishes with cooperative cancellation checks around the Store boundary.
pub async fn generate_documentation_api_with_composition_cancellable(
    options: DocumentationApiOperationOptions,
    composition: &RuntimeComposition,
    cancellation: CancellationToken,
) -> Result<DocumentationApiPublicationReport> {
    generate_documentation_api_with_composition_inner(options, composition, Some(cancellation)).await
}

async fn generate_documentation_api_with_composition_inner(
    options: DocumentationApiOperationOptions,
    composition: &RuntimeComposition,
    cancellation: Option<CancellationToken>,
) -> Result<DocumentationApiPublicationReport> {
    check_cancelled(&cancellation)?;
    options
        .request
        .validate()
        .map_err(anyhow::Error::msg)
        .context("invalid documentation generation request")?;
    if options.request.profile != DocumentationProfile::Api {
        bail!("API documentation operation requires profile `api`");
    }

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

    let publication = DocumentationApiPublicationOptions {
        root,
        force: options.force,
    };
    match cancellation {
        Some(cancellation) => publish_documentation_api_generation_cancellable(
            publication,
            &options.request,
            &snapshot,
            cancellation,
        ),
        None => publish_documentation_api_generation(publication, &options.request, &snapshot),
    }
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}
