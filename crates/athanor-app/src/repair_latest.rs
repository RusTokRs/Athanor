use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalLatestIdentity, CanonicalLatestPointer};
use athanor_domain::{GenerationId, SnapshotId};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

mod current {
    include!("repair_cleanup_recovery.rs");
}

pub use current::*;

const INDEX_CURRENT_PATH: &str = ".athanor/state/index-current.json";
const PUBLICATION_LOCK_PATH: &str = ".athanor/state/index-publication.lock";
const INDEX_CURRENT_SCHEMA: &str = "athanor.index_current.v1";

#[derive(Debug, Clone)]
pub struct RepairCanonicalLatestOptions {
    pub root: PathBuf,
    pub dry_run: bool,
    /// Optional exact target. Without it, index-current is preferred and backend discovery is fallback.
    pub snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCanonicalLatestReport {
    pub schema: String,
    pub root: PathBuf,
    pub dry_run: bool,
    pub needed: bool,
    pub repaired: bool,
    pub target: CanonicalLatestIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<CanonicalLatestIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_error: Option<String>,
    pub remaining_issues: Vec<RepairIssue>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexCurrentIdentity {
    schema: String,
    snapshot: String,
    generation: GenerationId,
    read_model: String,
    index_state: String,
}

/// Repairs the backend's latest committed identity without running indexing.
///
/// The target must equal the backend's independently discovered newest committed generation. This
/// prevents an explicit or stale application pointer from rewinding canonical latest visibility.
pub async fn repair_canonical_latest(
    options: RepairCanonicalLatestOptions,
) -> Result<RepairCanonicalLatestReport> {
    let root = crate::project_path::normalize_canonical_path(
        options
            .root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", options.root.display()))?,
    );

    if options.dry_run {
        let config = crate::config::load_config(&root)?;
        let store = crate::store::init_store(&root, &config).await?;
        return repair_canonical_latest_with_store(
            &root,
            true,
            options.snapshot.as_deref(),
            &store,
        )
        .await;
    }

    let _lock = LatestRepairLock::acquire(root.join(PUBLICATION_LOCK_PATH))?;
    let config = crate::config::load_config(&root)?;
    let store = crate::store::init_store(&root, &config).await?;
    repair_canonical_latest_with_store(&root, false, options.snapshot.as_deref(), &store).await
}

async fn repair_canonical_latest_with_store(
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
        read_index_current_identity(root)?
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

fn read_index_current_identity(root: &Path) -> Result<Option<CanonicalLatestIdentity>> {
    let path = root.join(INDEX_CURRENT_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let document: IndexCurrentIdentity = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    if document.schema != INDEX_CURRENT_SCHEMA {
        bail!(
            "index-current {} has unsupported schema {}",
            path.display(),
            document.schema
        );
    }
    let identity = CanonicalLatestIdentity {
        snapshot: SnapshotId(document.snapshot),
        generation: document.generation,
    };
    identity.validate()?;

    let expected_read = format!(
        ".athanor/generated/index-generations/{}/jsonl",
        identity.generation
    );
    let expected_state = format!(".athanor/state/index-state-{}.json", identity.generation);
    if document.read_model != expected_read || document.index_state != expected_state {
        bail!("index-current {} contains non-deterministic paths", path.display());
    }
    Ok(Some(identity))
}

struct LatestRepairLock {
    _file: File,
}

impl LatestRepairLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        let parent = path
            .parent()
            .with_context(|| format!("publication lock has no parent: {}", path.display()))?;
        fs::create_dir_all(parent)?;
        let file = File::create(&path)
            .with_context(|| format!("failed to open publication lock {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("failed to acquire publication lock {}", path.display()))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::{AtomicSnapshotPublication, CanonicalLatestPointer, KnowledgeStore, SnapshotBatch};
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;

    use super::*;

    #[tokio::test]
    async fn repairs_corrupt_jsonl_latest_to_authoritative_generation() {
        let root = test_root("repair");
        let backend = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
        let first = backend
            .begin_snapshot(RepoId("repo".to_string()), SnapshotBase::default())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(first.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        let second = backend
            .begin_snapshot(RepoId("repo".to_string()), SnapshotBase::default())
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

        let plan = repair_canonical_latest_with_store(
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

        let applied = repair_canonical_latest_with_store(
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
            .begin_snapshot(RepoId("repo".to_string()), SnapshotBase::default())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(first.clone(), SnapshotBatch::default())
            .await
            .unwrap();
        let second = backend
            .begin_snapshot(RepoId("repo".to_string()), SnapshotBase::default())
            .await
            .unwrap();
        backend
            .publish_snapshot_batch(second, SnapshotBatch::default())
            .await
            .unwrap();
        let store = crate::AthanorStore::new_with_latest_pointer(backend);

        let error = repair_canonical_latest_with_store(
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

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-latest-repair-{label}-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
