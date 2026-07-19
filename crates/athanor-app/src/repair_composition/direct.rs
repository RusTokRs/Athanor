use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalLatestIdentity, CanonicalLatestPointer};
use athanor_domain::SnapshotId;
use fs2::FileExt;
use serde::Deserialize;

use crate::composition::RuntimeComposition;
use crate::repair::{
    RepairCanonicalLatestOptions, RepairCanonicalLatestReport, RepairInspectOptions,
    RepairInspectReport, RepairRecoverIndexOptions, RepairRecoverIndexReport, inspect_repair,
};

const POINTER_JOURNAL_PATH: &str = ".athanor/state/index-current-publication.json";
const LEGACY_JOURNAL_PATH: &str = ".athanor/state/index-publication.json";
const PUBLICATION_LOCK_PATH: &str = ".athanor/state/index-publication.lock";

#[derive(Debug, Deserialize)]
struct PendingPointerJournal {
    snapshot: String,
    generation: String,
}

pub(super) async fn recover_index(
    options: RepairRecoverIndexOptions,
    composition: &RuntimeComposition,
) -> Result<RepairRecoverIndexReport> {
    let root = canonical_root(&options.root)?;
    if options.dry_run {
        let pending = pending_identity(&root)?;
        let needed = has_pending_publication(&root);
        let inspection = inspect_repair(RepairInspectOptions { root: root.clone() })?;
        return Ok(recovery_report(
            root, true, needed, false, pending, inspection,
        ));
    }

    let _lock = RepairLock::acquire(root.join(PUBLICATION_LOCK_PATH))?;
    let pending = pending_identity(&root)?;
    let needed = has_pending_publication(&root);
    let before = inspect_repair(RepairInspectOptions { root: root.clone() })?;
    if !needed {
        return Ok(recovery_report(
            root, false, false, false, pending, before,
        ));
    }

    let config = crate::config::load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    recover_index_with_store(&root, &store).await?;
    let after = inspect_repair(RepairInspectOptions { root: root.clone() })?;
    Ok(recovery_report(root, false, true, true, pending, after))
}

async fn recover_index_with_store(root: &Path, store: &crate::AthanorStore) -> Result<()> {
    crate::index_publication::recover_interrupted_publication(root, store)
        .await
        .context("failed to recover interrupted index publication")
}

pub(super) async fn repair_latest(
    options: RepairCanonicalLatestOptions,
    composition: &RuntimeComposition,
) -> Result<RepairCanonicalLatestReport> {
    let root = canonical_root(&options.root)?;
    if options.dry_run {
        let config = crate::config::load_config(&root)?;
        let store = composition.init_store(&root, &config).await?;
        return repair_latest_with_store(
            &root,
            true,
            options.snapshot.as_deref(),
            &store,
        )
        .await;
    }

    let _lock = RepairLock::acquire(root.join(PUBLICATION_LOCK_PATH))?;
    let config = crate::config::load_config(&root)?;
    let store = composition.init_store(&root, &config).await?;
    repair_latest_with_store(&root, false, options.snapshot.as_deref(), &store).await
}

async fn repair_latest_with_store(
    root: &Path,
    dry_run: bool,
    explicit_snapshot: Option<&str>,
    store: &crate::AthanorStore,
) -> Result<RepairCanonicalLatestReport> {
    let target = resolve_target(root, explicit_snapshot, store).await?;
    store
        .validate_latest_identity(&target)
        .await
        .context("canonical latest repair target is not authoritative")?;

    let (previous, previous_error) = match store.load_latest_identity().await {
        Ok(previous) => (previous, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let needed = previous.as_ref() != Some(&target) || previous_error.is_some();
    let mut repaired = false;

    if needed && !dry_run {
        store
            .repair_latest_identity(target.clone())
            .await
            .context("failed to repair canonical latest identity")?;
        let actual = store
            .load_latest_identity()
            .await
            .context("failed to verify repaired canonical latest identity")?;
        if actual.as_ref() != Some(&target) {
            bail!(
                "canonical latest repair verified {:?}, expected {} / {}",
                actual,
                target.snapshot.0,
                target.generation
            );
        }
        repaired = true;
    }

    let remaining_issues = inspect_repair(RepairInspectOptions {
        root: root.to_path_buf(),
    })?
    .issues;
    Ok(RepairCanonicalLatestReport {
        schema: "athanor.repair_canonical_latest.v1".to_string(),
        root: root.to_path_buf(),
        dry_run,
        needed,
        repaired,
        target,
        previous,
        previous_error,
        remaining_issues,
    })
}

async fn resolve_target(
    root: &Path,
    explicit_snapshot: Option<&str>,
    store: &crate::AthanorStore,
) -> Result<CanonicalLatestIdentity> {
    let requested = if let Some(snapshot) = explicit_snapshot {
        Some(CanonicalLatestIdentity::for_snapshot(SnapshotId(
            snapshot.to_string(),
        )))
    } else {
        crate::index_current::IndexCurrent::load(root)?
            .map(|current| current.canonical_identity())
    };
    let discovered = store
        .discover_latest_identity()
        .await
        .context("failed to discover authoritative latest committed generation")?
        .context("canonical store has no repairable committed generation")?;
    if let Some(requested) = requested {
        requested
            .validate()
            .context("requested latest identity is invalid")?;
        if requested != discovered {
            bail!(
                "requested latest identity {} / {} is not authoritative; backend discovered {} / {}",
                requested.snapshot.0,
                requested.generation,
                discovered.snapshot.0,
                discovered.generation
            );
        }
        Ok(requested)
    } else {
        Ok(discovered)
    }
}

fn recovery_report(
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

fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(crate::project_path::normalize_canonical_path(
        root.canonicalize()
            .with_context(|| format!("failed to canonicalize {}", root.display()))?,
    ))
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

struct RepairLock {
    _file: File,
}

impl RepairLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let parent = path
            .parent()
            .with_context(|| format!("publication lock has no parent: {}", path.display()))?;
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
    use athanor_core::{
        AtomicSnapshotPublication, CanonicalLatestPointer, KnowledgeStore, SnapshotBatch,
    };
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn repairs_corrupt_jsonl_latest_to_authoritative_generation() {
        let root = test_root("repair");
        let backend = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
        let first = backend
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(first.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        let second = backend
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(second.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        fs::write(
            root.join(".athanor/store/canonical/jsonl/latest.json"),
            "{",
        )
        .unwrap();
        let store = crate::AthanorStore::new_with_latest_pointer(backend);

        let plan = repair_latest_with_store(
            &root,
            true,
            Some(second.0.as_str()),
            &store,
        )
        .await
        .unwrap();
        assert!(plan.needed);
        assert!(!plan.repaired);
        assert!(plan.previous_error.is_some());

        let applied = repair_latest_with_store(
            &root,
            false,
            Some(second.0.as_str()),
            &store,
        )
        .await
        .unwrap();
        assert!(applied.repaired);
        assert_eq!(store.load_latest_identity().await.unwrap(), Some(applied.target));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn refuses_to_rewind_to_older_committed_generation() {
        let root = test_root("rewind");
        let backend = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
        let first = backend
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(first.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        let second = backend
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(second, SnapshotBatch::default())
            .await
            .unwrap();
        let store = crate::AthanorStore::new_with_latest_pointer(backend);

        let error = repair_latest_with_store(
            &root,
            true,
            Some(first.0.as_str()),
            &store,
        )
        .await
        .expect_err("repair must not rewind canonical latest");
        assert!(error.to_string().contains("not authoritative"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn standalone_recovery_publishes_committed_pending_pointer() {
        let root = test_root("committed");
        let store = crate::AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_repair_recovery".to_string()),
                snapshot_base(),
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
                "snapshot": snapshot_id.clone(),
                "generation": format!("gen_{snapshot_id}")
            }))
            .unwrap(),
        )
        .unwrap();

        recover_index_with_store(&root, &store).await.unwrap();

        assert!(root.join(".athanor/state/index-current.json").is_file());
        assert!(!root.join(POINTER_JOURNAL_PATH).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn dry_run_reports_pending_identity_without_mutation() {
        let root = test_root("dry-run");
        fs::create_dir_all(root.join(".athanor/state")).unwrap();
        fs::write(
            root.join(POINTER_JOURNAL_PATH),
            r#"{"schema":"athanor.index_current_publication.v1","snapshot":"snap_pending","generation":"gen_snap_pending"}"#,
        )
        .unwrap();

        let composition = crate::test_runtime::composition();
        let report = recover_index(
            RepairRecoverIndexOptions {
                root: root.clone(),
                dry_run: true,
            },
            &composition,
        )
        .await
        .unwrap();
        assert!(report.needed);
        assert!(!report.recovered);
        assert_eq!(report.snapshot.as_deref(), Some("snap_pending"));
        assert_eq!(report.generation.as_deref(), Some("gen_snap_pending"));
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

    fn snapshot_base() -> SnapshotBase {
        SnapshotBase {
            branch: None,
            commit: None,
            parent_snapshot: None,
            working_tree: true,
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-repair-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
