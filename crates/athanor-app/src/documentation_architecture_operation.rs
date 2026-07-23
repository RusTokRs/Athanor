//! Composition-aware deterministic architecture generation.
//!
//! This owner resolves one project, initializes its configured canonical Store, loads the exact
//! committed snapshot named by the request, and delegates immutable publication. CLI, daemon, and MCP
//! parsing remain outside this module.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::SnapshotId;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::{
    CancellationToken, DocumentationArchitecturePublicationOptions,
    DocumentationArchitecturePublicationReport, DocumentationGenerationRequest, RuntimeComposition,
    publish_documentation_architecture_generation,
    publish_documentation_architecture_generation_cancellable,
};

#[derive(Debug, Clone)]
pub struct DocumentationArchitectureOperationOptions {
    pub root: PathBuf,
    pub request: DocumentationGenerationRequest,
    pub force: bool,
}

/// Loads the exact committed snapshot through explicit runtime composition and publishes it.
pub async fn generate_documentation_architecture_with_composition(
    options: DocumentationArchitectureOperationOptions,
    composition: &RuntimeComposition,
) -> Result<DocumentationArchitecturePublicationReport> {
    generate_documentation_architecture_with_composition_inner(options, composition, None).await
}

/// Loads and publishes with cooperative cancellation checks around the Store boundary.
pub async fn generate_documentation_architecture_with_composition_cancellable(
    options: DocumentationArchitectureOperationOptions,
    composition: &RuntimeComposition,
    cancellation: CancellationToken,
) -> Result<DocumentationArchitecturePublicationReport> {
    generate_documentation_architecture_with_composition_inner(
        options,
        composition,
        Some(cancellation),
    )
    .await
}

async fn generate_documentation_architecture_with_composition_inner(
    options: DocumentationArchitectureOperationOptions,
    composition: &RuntimeComposition,
    cancellation: Option<CancellationToken>,
) -> Result<DocumentationArchitecturePublicationReport> {
    check_cancelled(&cancellation)?;
    options
        .request
        .validate()
        .map_err(anyhow::Error::msg)
        .context("invalid documentation generation request")?;

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

    let publication = DocumentationArchitecturePublicationOptions {
        root,
        force: options.force,
    };
    match cancellation {
        Some(cancellation) => publish_documentation_architecture_generation_cancellable(
            publication,
            &options.request,
            &snapshot,
            cancellation,
        ),
        None => publish_documentation_architecture_generation(
            publication,
            &options.request,
            &snapshot,
        ),
    }
}

fn check_cancelled(cancellation: &Option<CancellationToken>) -> Result<()> {
    if let Some(cancellation) = cancellation {
        cancellation.check()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use athanor_core::{CanonicalSnapshot, KnowledgeStore, SnapshotBatch};
    use athanor_domain::{RepoId, SnapshotBase, SnapshotId};

    use super::*;
    use crate::test_runtime;
    use crate::{DocumentationGenerationLimits, DocumentationProfile};

    const FIXTURE: &str =
        include_str!("../tests/fixtures/documentation_architecture_profile.v1.json");

    #[tokio::test]
    async fn loads_exact_committed_snapshot_and_publishes() {
        let project = TempProject::new("exact");
        let composition = test_runtime::composition();
        let snapshot_id = seed_committed_snapshot(&project.root, &composition).await;
        let report = generate_documentation_architecture_with_composition(
            options(&project.root, snapshot_id.0.clone()),
            &composition,
        )
        .await
        .expect("generate architecture documentation from committed snapshot");

        assert_eq!(report.snapshot, snapshot_id.0);
        assert!(report.document.is_file());
        assert!(report.validation_report.is_file());
        assert!(report.current_pointer.is_file());
    }

    #[tokio::test]
    async fn missing_or_cancelled_snapshot_does_not_publish() {
        let project = TempProject::new("missing");
        let composition = test_runtime::composition();
        let missing = generate_documentation_architecture_with_composition(
            options(&project.root, "snap-missing".to_string()),
            &composition,
        )
        .await
        .expect_err("missing committed snapshot must fail");
        assert!(missing.to_string().contains("not committed or does not exist"));
        assert!(!documentation_root(&project.root).exists());

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let cancelled = generate_documentation_architecture_with_composition_cancellable(
            options(&project.root, "snap-missing".to_string()),
            &composition,
            cancellation,
        )
        .await
        .expect_err("pre-cancelled operation must fail");
        assert!(cancelled.to_string().contains("operation cancelled"));
        assert!(!documentation_root(&project.root).exists());
    }

    async fn seed_committed_snapshot(root: &Path, composition: &RuntimeComposition) -> SnapshotId {
        let config = load_config(root).unwrap();
        let store = composition.init_store(root, &config).await.unwrap();
        let snapshot_id = store
            .begin_snapshot(
                RepoId("repo-documentation-operation".to_string()),
                SnapshotBase {
                    branch: Some("main".to_string()),
                    commit: Some("fixture".to_string()),
                    parent_snapshot: None,
                    working_tree: false,
                },
            )
            .await
            .unwrap();
        let mut snapshot: CanonicalSnapshot = serde_json::from_str(FIXTURE).unwrap();
        snapshot.snapshot = Some(snapshot_id.clone());
        for fact in &mut snapshot.facts {
            fact.snapshot = snapshot_id.clone();
        }
        for relation in &mut snapshot.relations {
            relation.snapshot = snapshot_id.clone();
        }
        for diagnostic in &mut snapshot.diagnostics {
            diagnostic.snapshot = snapshot_id.clone();
        }
        store
            .put_snapshot(
                snapshot_id.clone(),
                SnapshotBatch {
                    entities: snapshot.entities,
                    facts: snapshot.facts,
                    relations: snapshot.relations,
                    diagnostics: snapshot.diagnostics,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(snapshot_id.clone()).await.unwrap();
        snapshot_id
    }

    fn options(root: &Path, snapshot: String) -> DocumentationArchitectureOperationOptions {
        DocumentationArchitectureOperationOptions {
            root: root.to_path_buf(),
            request: DocumentationGenerationRequest::new(
                snapshot,
                DocumentationProfile::Architecture,
                DocumentationGenerationLimits {
                    max_entities: 16,
                    max_facts: 16,
                    max_relations: 16,
                    max_diagnostics: 8,
                },
            ),
            force: false,
        }
    }

    fn documentation_root(root: &Path) -> PathBuf {
        root.join(".athanor/generated/documentation")
    }

    struct TempProject {
        root: PathBuf,
    }

    impl TempProject {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "athanor-documentation-operation-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
