//! Read-only repository documentation ownership planning for one committed JSONL snapshot.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::CanonicalSnapshotStore;
use athanor_domain::SnapshotId;
use athanor_store_jsonl::JsonlKnowledgeStore;

use crate::config::load_config;
use crate::project_path::normalize_canonical_path;
use crate::{
    DocumentationRepositoryPlan, DocumentationRepositoryPlanRequest, RuntimeComposition,
    StorageMode, build_documentation_repository_plan,
};

#[derive(Debug, Clone)]
pub struct DocumentationRepositoryPlanOptions {
    pub root: PathBuf,
    pub request: DocumentationRepositoryPlanRequest,
}

/// Reads canonical data only. It never indexes, probes output paths, or writes
/// package Markdown. The composition's Store factory is deliberately bypassed:
/// a caller-supplied factory need not honor the JSONL setting or be read-only.
/// The concrete JSONL reader only loads the exact committed snapshot.
pub async fn plan_documentation_repository_with_composition(
    options: DocumentationRepositoryPlanOptions,
    _composition: &RuntimeComposition,
) -> Result<DocumentationRepositoryPlan> {
    options
        .request
        .validate()
        .map_err(anyhow::Error::new)
        .context("invalid repository documentation plan request")?;
    let root = normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let config = load_config(&root)?;
    if config.storage.mode != StorageMode::Jsonl {
        bail!("read-only repository documentation planning requires a JSONL Store");
    }
    let store_relative = &config.storage.path;
    if store_relative.is_empty()
        || store_relative.starts_with('/')
        || store_relative.contains('\\')
        || store_relative
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | "..") || part.contains(':'))
    {
        bail!("repository documentation Store path must be a confined relative path");
    }
    let canonical_store_path = root
        .join(store_relative)
        .canonicalize()
        .context("repository documentation Store must already exist")?;
    if !canonical_store_path.starts_with(&root) {
        bail!("repository documentation Store resolves outside the project root");
    }
    reject_jsonl_symlinks(&root, store_relative, &options.request.snapshot)?;
    let store = JsonlKnowledgeStore::new(canonical_store_path.clone());
    let expected = SnapshotId(options.request.snapshot.clone());
    let snapshot = store
        .load_snapshot(&expected)
        .await
        .with_context(|| format!("failed to load exact canonical snapshot {}", expected.0))?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "canonical snapshot {} is not committed or does not exist",
                expected.0
            )
        })?;
    if snapshot.snapshot.as_ref() != Some(&expected) {
        bail!(
            "canonical Store returned snapshot identity {:?}, expected {}",
            snapshot.snapshot,
            expected.0
        );
    }
    verify_snapshot_payload(&canonical_store_path, &expected, &snapshot)?;
    build_documentation_repository_plan(&options.request, &snapshot).map_err(anyhow::Error::new)
}

/// The Store's general reader tolerates an absent JSONL file as an empty list.
/// Planning needs the *complete* committed snapshot instead: every payload
/// file must exist, even when it has zero rows, and its decoded row count must
/// agree with the exact generation manifest written at commit time.
fn verify_snapshot_payload(
    store: &Path,
    id: &SnapshotId,
    snapshot: &athanor_core::CanonicalSnapshot,
) -> Result<()> {
    let generation = store.join("snapshots").join(&id.0);
    let manifest_path = generation.join("manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).with_context(|| {
            format!(
                "failed to read canonical manifest {}",
                manifest_path.display()
            )
        })?)
        .with_context(|| {
            format!(
                "failed to parse canonical manifest {}",
                manifest_path.display()
            )
        })?;
    for (name, count) in [
        ("entities", snapshot.entities.len()),
        ("facts", snapshot.facts.len()),
        ("relations", snapshot.relations.len()),
        ("diagnostics", snapshot.diagnostics.len()),
    ] {
        let path = generation.join(format!("{name}.jsonl"));
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("canonical snapshot payload {} is missing", path.display()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!(
                "canonical snapshot payload {} is not a regular file",
                path.display()
            );
        }
        if manifest.get(name).and_then(serde_json::Value::as_u64) != Some(count as u64) {
            bail!("canonical snapshot {name} count does not match its committed manifest");
        }
    }
    Ok(())
}

fn reject_jsonl_symlinks(root: &Path, store_relative: &str, snapshot: &str) -> Result<()> {
    let mut path = root.to_path_buf();
    for part in store_relative.split('/') {
        path.push(part);
        require_regular_directory(&path)?;
    }
    let snapshots = path.join("snapshots");
    reject_optional_symlink(&snapshots)?;
    reject_optional_symlink(&path.join("snapshot-sequence.json"))?;
    let generation = snapshots.join(snapshot);
    reject_optional_symlink(&generation)?;
    for name in [
        "manifest.json",
        "commit.json",
        "entities.jsonl",
        "facts.jsonl",
        "relations.jsonl",
        "diagnostics.jsonl",
    ] {
        reject_optional_symlink(&generation.join(name))?;
    }
    Ok(())
}

fn require_regular_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "repository documentation Store path {} is unavailable",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "repository documentation Store path {} must be a real directory",
            path.display()
        );
    }
    Ok(())
}

fn reject_optional_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "repository documentation Store must not contain symlink {}",
                path.display()
            );
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("cannot inspect Store path {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use athanor_core::{KnowledgeStore, SnapshotBatch};
    use athanor_domain::{RepoId, SnapshotBase};

    use super::*;
    use crate::test_runtime;

    fn forbidden_store<'a>(
        _: &'a Path,
        _: &'a crate::ProjectConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::AthanorStore>> + Send + 'a>>
    {
        panic!("read-only planning must not invoke an injected Store factory")
    }

    fn forbidden_composition() -> RuntimeComposition {
        RuntimeComposition::new(
            crate::AdapterRegistry::empty,
            |_, _, _| None,
            forbidden_store,
            |_, _| panic!("read-only planning must not initialize a search index"),
            |_, _, _, _| panic!("read-only planning must not publish wiki output"),
            |_, _, _, _| panic!("read-only planning must not publish HTML output"),
        )
    }

    struct TempProject(PathBuf);

    impl TempProject {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let root = std::env::temp_dir().join(format!(
                "athanor-repository-plan-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tree(root: &Path) -> Vec<PathBuf> {
        fn visit(root: &Path, dir: &Path, result: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                result.push(entry.path().strip_prefix(root).unwrap().to_path_buf());
                if entry.file_type().unwrap().is_dir() {
                    visit(root, &entry.path(), result);
                }
            }
        }
        let mut result = Vec::new();
        visit(root, root, &mut result);
        result.sort();
        result
    }

    #[tokio::test]
    async fn loads_only_the_committed_identity_without_creating_output_or_store_files() {
        let project = TempProject::new();
        let composition = test_runtime::composition();
        let config = load_config(&project.0).unwrap();
        let store = composition.init_store(&project.0, &config).await.unwrap();
        let id = store
            .begin_snapshot(
                RepoId("repo-plan-fixture".to_string()),
                SnapshotBase {
                    branch: Some("main".to_string()),
                    commit: Some("fixture".to_string()),
                    parent_snapshot: None,
                    working_tree: false,
                },
            )
            .await
            .unwrap();
        store
            .put_snapshot(id.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        store.commit_snapshot(id.clone()).await.unwrap();
        fs::write(project.0.join("human-readme.md"), "do not modify").unwrap();
        let before = tree(&project.0);
        let report = plan_documentation_repository_with_composition(
            DocumentationRepositoryPlanOptions {
                root: project.0.clone(),
                request: DocumentationRepositoryPlanRequest::new(id.0.clone(), 8),
            },
            &composition,
        )
        .await
        .unwrap();
        assert_eq!(report.snapshot, id.0);
        assert!(!report.writes_allowed);
        assert_eq!(tree(&project.0), before);
        assert_eq!(
            fs::read_to_string(project.0.join("human-readme.md")).unwrap(),
            "do not modify"
        );

        // A composition may contain a backend that writes on initialization.
        // Its factory is not a capability granted to this read-only operation.
        let without_factory = plan_documentation_repository_with_composition(
            DocumentationRepositoryPlanOptions {
                root: project.0.clone(),
                request: DocumentationRepositoryPlanRequest::new(id.0.clone(), 8),
            },
            &forbidden_composition(),
        )
        .await
        .unwrap();
        assert_eq!(without_factory, report);
        assert_eq!(tree(&project.0), before);

        let missing = plan_documentation_repository_with_composition(
            DocumentationRepositoryPlanOptions {
                root: project.0.clone(),
                request: DocumentationRepositoryPlanRequest::new("snap_missing", 8),
            },
            &composition,
        )
        .await
        .expect_err("no latest fallback");
        assert!(
            missing
                .to_string()
                .contains("not committed or does not exist")
        );
        assert_eq!(tree(&project.0), before);
    }

    #[tokio::test]
    async fn refuses_missing_or_count_mismatched_committed_payloads_without_writes() {
        let project = TempProject::new();
        let composition = test_runtime::composition();
        let config = load_config(&project.0).unwrap();
        let store = composition.init_store(&project.0, &config).await.unwrap();
        let id = store
            .begin_snapshot(
                RepoId("repo-plan-incomplete".to_string()),
                SnapshotBase {
                    branch: Some("main".to_string()),
                    commit: Some("fixture".to_string()),
                    parent_snapshot: None,
                    working_tree: false,
                },
            )
            .await
            .unwrap();
        store
            .put_snapshot(id.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        store.commit_snapshot(id.clone()).await.unwrap();
        let generation = project
            .0
            .join(config.storage.path.as_str())
            .join("snapshots")
            .join(&id.0);
        let entities = generation.join("entities.jsonl");
        fs::remove_file(&entities).unwrap();
        let before = tree(&project.0);
        let error = plan_documentation_repository_with_composition(
            DocumentationRepositoryPlanOptions {
                root: project.0.clone(),
                request: DocumentationRepositoryPlanRequest::new(id.0.clone(), 8),
            },
            &composition,
        )
        .await
        .expect_err("even a zero-row JSONL payload must be present");
        assert!(format!("{error:#}").contains("payload"));
        assert_eq!(tree(&project.0), before);

        fs::write(&entities, "").unwrap();
        let manifest_path = generation.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["facts"] = serde_json::json!(1);
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let before = tree(&project.0);
        let error = plan_documentation_repository_with_composition(
            DocumentationRepositoryPlanOptions {
                root: project.0.clone(),
                request: DocumentationRepositoryPlanRequest::new(id.0.clone(), 8),
            },
            &composition,
        )
        .await
        .expect_err("a manifest cannot claim records absent from its JSONL payload");
        assert!(format!("{error:#}").contains("count does not match"));
        assert_eq!(tree(&project.0), before);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_snapshot_data_symlinks_before_store_loading() {
        use std::os::unix::fs::symlink;
        let project = TempProject::new();
        let store_path = project.0.join(".athanor/store/canonical/jsonl");
        let generation = store_path.join("snapshots/snap_fixture");
        fs::create_dir_all(&generation).unwrap();
        let outside = project.0.join("human-readme.md");
        fs::write(&outside, "do not read").unwrap();
        symlink(&outside, generation.join("entities.jsonl")).unwrap();
        assert!(
            reject_jsonl_symlinks(&project.0, ".athanor/store/canonical/jsonl", "snap_fixture")
                .is_err()
        );
    }
}
