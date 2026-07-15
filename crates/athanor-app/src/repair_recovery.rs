use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

mod current {
    include!("repair_retention_guard.rs");
}

pub use current::*;

const POINTER_JOURNAL_PATH: &str = ".athanor/state/index-current-publication.json";
const LEGACY_JOURNAL_PATH: &str = ".athanor/state/index-publication.json";
const PUBLICATION_LOCK_PATH: &str = ".athanor/state/index-publication.lock";

#[derive(Debug, Clone)]
pub struct RepairRecoverIndexOptions {
    pub root: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairRecoverIndexReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub recovered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    pub remaining_issues: Vec<RepairIssue>,
    pub inspection: RepairInspectReport,
}

#[derive(Debug, Deserialize)]
struct PendingPointerJournal {
    snapshot: String,
    generation: String,
}

/// Runs publication recovery without starting source discovery or the indexing pipeline.
pub async fn recover_index_publication(
    options: RepairRecoverIndexOptions,
) -> Result<RepairRecoverIndexReport> {
    let root = crate::project_path::normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );
    let pending = pending_identity(&root)?;
    let needed = has_pending_publication(&root);
    let before = inspect_repair(RepairInspectOptions { root: root.clone() })?;

    if options.dry_run || !needed {
        return Ok(report(
            root,
            options.dry_run,
            needed,
            false,
            pending,
            before,
        ));
    }

    let _lock = PublicationLock::acquire(root.join(PUBLICATION_LOCK_PATH))?;
    let config = crate::config::load_config(&root)?;
    let store = crate::store::init_store(&root, &config).await?;
    recover_index_publication_with_store(&root, &store).await?;
    let after = inspect_repair(RepairInspectOptions { root: root.clone() })?;
    Ok(report(root, false, true, true, pending, after))
}

async fn recover_index_publication_with_store(
    root: &Path,
    store: &crate::AthanorStore,
) -> Result<()> {
    crate::index_publication::recover_interrupted_publication(root, store)
        .await
        .context("failed to recover interrupted index publication")
}

fn report(
    root: PathBuf,
    dry_run: bool,
    needed: bool,
    recovered: bool,
    pending: Option<PendingPointerJournal>,
    inspection: RepairInspectReport,
) -> RepairRecoverIndexReport {
    RepairRecoverIndexReport {
        schema: "athanor.repair_recover_index.v1".to_string(),
        root,
        dry_run,
        needed,
        recovered,
        snapshot: pending.as_ref().map(|pending| pending.snapshot.clone()),
        generation: pending.map(|pending| pending.generation),
        remaining_issues: inspection.issues.clone(),
        inspection,
    }
}

fn has_pending_publication(root: &Path) -> bool {
    root.join(POINTER_JOURNAL_PATH).exists() || root.join(LEGACY_JOURNAL_PATH).exists()
}

fn pending_identity(root: &Path) -> Result<Option<PendingPointerJournal>> {
    let path = root.join(POINTER_JOURNAL_PATH);
    if !path.exists() {
        return Ok(None);
    }
    serde_json::from_slice(
        &fs::read(&path)
            .with_context(|| format!("failed to read pending pointer journal {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse pending pointer journal {}", path.display()))
    .map(Some)
}

struct PublicationLock {
    _file: File,
}

impl PublicationLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!("publication lock path has no parent: {}", path.display())
        })?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create publication lock directory {}",
                parent.display()
            )
        })?;
        let file = File::create(&path)
            .with_context(|| format!("failed to open publication lock {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("failed to acquire publication lock {}", path.display()))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::KnowledgeStore;
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn standalone_recovery_publishes_committed_pending_pointer() {
        let root = test_root("committed");
        let store = crate::AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_repair_recovery".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        store.commit_snapshot(snapshot.clone()).await.unwrap();
        let snapshot_id = snapshot.0.clone();
        write_legacy_artifacts(&root, &snapshot_id);
        fs::write(
            root.join(POINTER_JOURNAL_PATH),
            serde_json::to_vec_pretty(&json!({
                "schema": "athanor.index_current_publication.v1",
                "snapshot": snapshot_id,
                "generation": format!("gen_{snapshot_id}")
            }))
            .unwrap(),
        )
        .unwrap();

        recover_index_publication_with_store(&root, &store)
            .await
            .unwrap();

        assert!(root.join(".athanor/state/index-current.json").is_file());
        assert!(!root.join(POINTER_JOURNAL_PATH).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_reports_pending_identity_without_mutation() {
        let root = test_root("dry-run");
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(
            root.join(POINTER_JOURNAL_PATH),
            r#"{"schema":"athanor.index_current_publication.v1","snapshot":"snap_pending","generation":"gen_snap_pending"}"#,
        )
        .unwrap();

        let pending = pending_identity(&root).unwrap().unwrap();
        assert_eq!(pending.snapshot, "snap_pending");
        assert_eq!(pending.generation, "gen_snap_pending");
        assert!(has_pending_publication(&root));
        assert!(root.join(POINTER_JOURNAL_PATH).exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn write_legacy_artifacts(root: &Path, snapshot: &str) {
        let generation = format!("gen_{snapshot}");
        let read_model = root.join(".athanor/generated/current/jsonl");
        fs::create_dir_all(&read_model).unwrap();
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(
            read_model.join("manifest.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
                "snapshot": snapshot,
                "generation": generation
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(".athanor/state/index-state.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": crate::index_state::INDEX_STATE_SCHEMA,
                "snapshot": snapshot,
                "generation": format!("gen_{snapshot}"),
                "files": {}
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-repair-recovery-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
