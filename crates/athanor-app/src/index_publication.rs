use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshotStore, CoreError, OperationContext};
use athanor_domain::SnapshotId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::prepared_publication::{PreparedSnapshot, PreparedSnapshotPublication};
use crate::{
    AthanorStore, IndexPipelineOutput, IndexState, IndexStateStore, JsonlReadModelReport,
    JsonlReadModelWriter,
};

pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V1: &str = "athanor.index_publication.v1";
pub(crate) const INDEX_PUBLICATION_JOURNAL_SCHEMA_V2: &str = "athanor.index_publication.v2";

/// Result of publishing one prepared canonical snapshot together with application artefacts.
#[derive(Debug)]
pub(crate) struct IndexPublicationOutcome {
    pub(crate) read_model: JsonlReadModelReport,
    pub(crate) read_model_write_ms: u64,
    pub(crate) index_state_write_ms: u64,
}

/// Typed recovery record for one prepared index publication.
///
/// Version 2 persists `PreparedSnapshot` directly. Version 1 records containing a raw `snapshot`
/// string remain readable and are normalized to the version-2 representation in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct IndexPublicationJournal {
    schema: String,
    prepared: PreparedSnapshot,
    id: String,
    read_model: PathBuf,
    index_state: PathBuf,
}

impl IndexPublicationJournal {
    pub(crate) fn new(root: &Path, prepared: PreparedSnapshot) -> Self {
        Self::with_id(root, prepared, publication_id())
    }

    fn with_id(root: &Path, prepared: PreparedSnapshot, id: String) -> Self {
        Self {
            schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
            prepared,
            id,
            read_model: expected_read_model(root),
            index_state: expected_index_state(root),
        }
    }

    pub(crate) fn load(root: &Path) -> Result<Option<Self>> {
        let path = Self::path(root);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read publication journal {}", path.display()))?;
        let journal: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse publication journal {}", path.display()))?;
        journal.validate_for_root(root)?;
        Ok(Some(journal))
    }

    pub(crate) fn write(&self) -> Result<()> {
        validate_publication_id(&self.id)?;
        let path = Self::path_from_artifact(&self.index_state);
        let parent = path.parent().ok_or_else(|| {
            anyhow::anyhow!("publication journal has no parent: {}", path.display())
        })?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create publication journal directory {}",
                parent.display()
            )
        })?;

        let staging = parent.join(format!(".index-publication.staging-{}", self.id));
        let backup = parent.join(format!(".index-publication.backup-{}", self.id));
        fs::write(&staging, serde_json::to_vec_pretty(self)?).with_context(|| {
            format!("failed to write publication journal {}", staging.display())
        })?;
        if path.exists() {
            fs::rename(&path, &backup).with_context(|| {
                format!(
                    "failed to stage previous publication journal {}",
                    path.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&staging, &path) {
            if backup.exists() {
                let _ = fs::rename(&backup, &path);
            }
            let _ = fs::remove_file(&staging);
            return Err(error).with_context(|| {
                format!("failed to publish publication journal {}", path.display())
            });
        }
        if backup.exists() {
            fs::remove_file(&backup).with_context(|| {
                format!(
                    "failed to remove publication journal backup {}",
                    backup.display()
                )
            })?;
        }
        Ok(())
    }

    pub(crate) fn clear(&self) -> Result<()> {
        let path = Self::path_from_artifact(&self.index_state);
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!("failed to clear publication journal {}", path.display())
            })?;
        }
        Ok(())
    }

    pub(crate) fn path(root: &Path) -> PathBuf {
        root.join(".athanor/state/index-publication.json")
    }

    pub(crate) fn prepared(&self) -> &PreparedSnapshot {
        &self.prepared
    }

    pub(crate) fn snapshot(&self) -> &SnapshotId {
        self.prepared.snapshot()
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn read_model(&self) -> &Path {
        &self.read_model
    }

    pub(crate) fn index_state(&self) -> &Path {
        &self.index_state
    }

    fn path_from_artifact(index_state: &Path) -> PathBuf {
        index_state.with_file_name("index-publication.json")
    }

    fn validate_for_root(&self, root: &Path) -> Result<()> {
        validate_publication_id(&self.id)?;
        let expected_read_model = expected_read_model(root);
        if self.read_model != expected_read_model {
            bail!(
                "publication journal read-model path {} does not match expected {}",
                self.read_model.display(),
                expected_read_model.display()
            );
        }
        let expected_index_state = expected_index_state(root);
        if self.index_state != expected_index_state {
            bail!(
                "publication journal index-state path {} does not match expected {}",
                self.index_state.display(),
                expected_index_state.display()
            );
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for IndexPublicationJournal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct JournalV2 {
            schema: String,
            prepared: PreparedSnapshot,
            id: String,
            read_model: PathBuf,
            index_state: PathBuf,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct JournalV1 {
            schema: String,
            snapshot: String,
            id: String,
            read_model: PathBuf,
            index_state: PathBuf,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum JournalWire {
            V2(JournalV2),
            V1(JournalV1),
        }

        match JournalWire::deserialize(deserializer)? {
            JournalWire::V2(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V2 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
                    prepared: journal.prepared,
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                })
            }
            JournalWire::V1(journal) => {
                if journal.schema != INDEX_PUBLICATION_JOURNAL_SCHEMA_V1 {
                    return Err(D::Error::custom(format!(
                        "unsupported publication journal schema {}",
                        journal.schema
                    )));
                }
                Ok(Self {
                    schema: INDEX_PUBLICATION_JOURNAL_SCHEMA_V2.to_string(),
                    prepared: PreparedSnapshot::new(SnapshotId(journal.snapshot)),
                    id: journal.id,
                    read_model: journal.read_model,
                    index_state: journal.index_state,
                })
            }
        }
    }
}

/// Stages read-model and state artefacts, publishes the typed canonical handle, then finalizes all
/// backups. Any failure before canonical publication rolls back staged artefacts and aborts the
/// prepared snapshot outside the caller cancellation/deadline budget.
pub(crate) async fn publish_prepared_index(
    root: &Path,
    store: &AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
    output: &IndexPipelineOutput,
    prepared: PreparedSnapshot,
    operation: &OperationContext,
) -> Result<IndexPublicationOutcome> {
    if prepared.snapshot() != &output.snapshot {
        bail!(
            "prepared snapshot {} does not match pipeline output {}",
            prepared.snapshot().0,
            output.snapshot.0
        );
    }

    let journal = IndexPublicationJournal::new(root, prepared.clone());
    journal.write()?;

    let read_model_started = Instant::now();
    let prepared_read_model = match JsonlReadModelWriter::new(output_dir)
        .prepare_with_publication_id(output, journal.id())
    {
        Ok(prepared_read_model) => prepared_read_model,
        Err(error) => {
            let _ = journal.clear();
            abort_prepared_with_error(store, &prepared, error).await?;
            unreachable!("abort_prepared_with_error always returns an error")
        }
    };
    let read_model_write_ms = elapsed_ms(read_model_started.elapsed());

    let index_state_started = Instant::now();
    let prepared_index_state = match state_store.prepare_with_publication_id(
        &IndexState::from_sources(&output.snapshot.0, &output.files),
        journal.id(),
    ) {
        Ok(prepared_index_state) => prepared_index_state,
        Err(error) => {
            let rollback_error = prepared_read_model.rollback().err();
            let error = if let Some(rollback_error) = rollback_error {
                error.context(format!("failed to rollback read model: {rollback_error}"))
            } else {
                error
            };
            let _ = journal.clear();
            abort_prepared_with_error(store, &prepared, error).await?;
            unreachable!("abort_prepared_with_error always returns an error")
        }
    };
    let index_state_write_ms = elapsed_ms(index_state_started.elapsed());

    if let Err(error) = store.publish_prepared(&prepared, operation).await {
        let state_rollback_error = prepared_index_state.rollback().err();
        let read_model_rollback_error = prepared_read_model.rollback().err();
        let error = anyhow::Error::new(error).context(
            "failed to publish prepared canonical snapshot after read model and index state",
        );
        let error = if let Some(rollback_error) = state_rollback_error {
            error.context(format!("failed to rollback index state: {rollback_error}"))
        } else {
            error
        };
        let error = if let Some(rollback_error) = read_model_rollback_error {
            error.context(format!("failed to rollback read model: {rollback_error}"))
        } else {
            error
        };
        let _ = journal.clear();
        abort_prepared_with_error(store, &prepared, error).await?;
        unreachable!("abort_prepared_with_error always returns an error")
    }

    // The journal deliberately remains until both application artefacts are finalized. If either
    // finalize step fails, recovery sees the committed canonical snapshot and removes stale backups.
    let read_model = prepared_read_model.finalize()?;
    prepared_index_state.finalize()?;
    journal.clear()?;

    Ok(IndexPublicationOutcome {
        read_model,
        read_model_write_ms,
        index_state_write_ms,
    })
}

/// Recovers an interrupted typed or legacy publication journal. Committed canonical snapshots keep
/// the newly published application artefacts; uncommitted snapshots restore backups and are aborted
/// through the cancellation-independent typed cleanup path.
pub(crate) async fn recover_interrupted_publication(
    root: &Path,
    store: &AthanorStore,
) -> Result<()> {
    let Some(journal) = IndexPublicationJournal::load(root)? else {
        return Ok(());
    };

    let committed = store
        .load_latest_snapshot()
        .await?
        .and_then(|snapshot| snapshot.snapshot)
        .is_some_and(|snapshot| snapshot == *journal.snapshot());

    if committed {
        cleanup_publication_artifacts(&journal)?;
    } else {
        rollback_publication_artifacts(&journal)?;
        match store.abort_prepared(journal.prepared()).await {
            Ok(()) | Err(CoreError::NotFound(_)) => {}
            Err(error) => {
                return Err(anyhow::Error::new(error).context("failed to abort recovered snapshot"));
            }
        }
    }

    journal.clear()
}

async fn abort_prepared_with_error(
    store: &AthanorStore,
    prepared: &PreparedSnapshot,
    error: anyhow::Error,
) -> Result<()> {
    match store.abort_prepared(prepared).await {
        Ok(()) => Err(error),
        Err(abort_error) => Err(error.context(format!(
            "failed to abort prepared snapshot {}: {abort_error}",
            prepared.snapshot().0
        ))),
    }
}

fn cleanup_publication_artifacts(journal: &IndexPublicationJournal) -> Result<()> {
    let (read_staging, read_backup) = publication_paths(journal.read_model(), journal.id())?;
    let (state_staging, state_backup) = publication_paths(journal.index_state(), journal.id())?;
    if read_staging.exists() {
        fs::remove_dir_all(read_staging)?;
    }
    if read_backup.exists() {
        fs::remove_dir_all(read_backup)?;
    }
    if state_staging.exists() {
        fs::remove_file(state_staging)?;
    }
    if state_backup.exists() {
        fs::remove_file(state_backup)?;
    }
    Ok(())
}

fn rollback_publication_artifacts(journal: &IndexPublicationJournal) -> Result<()> {
    restore_publication_directory(
        journal.read_model(),
        journal.id(),
        &journal.snapshot().0,
    )?;
    restore_publication_file(
        journal.index_state(),
        journal.id(),
        &journal.snapshot().0,
    )?;
    Ok(())
}

fn publication_paths(path: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
    validate_publication_id(id)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("publication artifact has no parent: {}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid publication artifact: {}", path.display()))?;
    Ok((
        parent.join(format!(".{name}.staging-{id}")),
        parent.join(format!(".{name}.backup-{id}")),
    ))
}

fn restore_publication_directory(path: &Path, id: &str, snapshot: &str) -> Result<()> {
    let (staging, backup) = publication_paths(path, id)?;
    if staging.exists() {
        fs::remove_dir_all(staging)?;
    }
    if backup.exists() {
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        fs::rename(backup, path)?;
    } else if read_model_snapshot(path).is_some_and(|current| current == snapshot) {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn restore_publication_file(path: &Path, id: &str, snapshot: &str) -> Result<()> {
    let (staging, backup) = publication_paths(path, id)?;
    if staging.exists() {
        fs::remove_file(staging)?;
    }
    if backup.exists() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(backup, path)?;
    } else if state_snapshot(path).is_some_and(|current| current == snapshot) {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn expected_read_model(root: &Path) -> PathBuf {
    root.join(".athanor/generated/current/jsonl")
}

fn expected_index_state(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-state.json")
}

fn validate_publication_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("invalid publication id `{id}`");
    }
    Ok(())
}

fn publication_id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn read_model_snapshot(path: &Path) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(&fs::read(path.join("manifest.json")).ok()?)
        .ok()?
        .get("snapshot")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn state_snapshot(path: &Path) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(&fs::read(path).ok()?)
        .ok()?
        .get("snapshot")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use athanor_core::KnowledgeStore;
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;
    use serde_json::json;

    use super::*;

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("athanor-index-publication-{label}-{nonce}"));
        fs::create_dir_all(&root).expect("create publication test root");
        root
    }

    #[test]
    fn version_two_round_trip_preserves_typed_prepared_handle() {
        let root = PathBuf::from("project");
        let journal = IndexPublicationJournal::with_id(
            &root,
            PreparedSnapshot::new(SnapshotId("snap_test_0002".to_string())),
            "publication-2".to_string(),
        );

        let value = serde_json::to_value(&journal).expect("serialize version-two journal");
        assert_eq!(value["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V2);
        assert_eq!(value["prepared"], "snap_test_0002");
        assert!(value.get("snapshot").is_none());

        let decoded: IndexPublicationJournal =
            serde_json::from_value(value).expect("deserialize version-two journal");
        assert_eq!(decoded, journal);
        assert_eq!(decoded.snapshot().0, "snap_test_0002");
        assert_eq!(decoded.id(), "publication-2");
        assert_eq!(
            decoded.read_model(),
            Path::new("project/.athanor/generated/current/jsonl")
        );
        assert_eq!(
            decoded.index_state(),
            Path::new("project/.athanor/state/index-state.json")
        );
    }

    #[test]
    fn version_one_journal_is_normalized_to_typed_version_two() {
        let value = json!({
            "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V1,
            "snapshot": "snap_test_0001",
            "id": "publication-1",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        });

        let decoded: IndexPublicationJournal =
            serde_json::from_value(value).expect("deserialize version-one journal");
        assert_eq!(decoded.snapshot().0, "snap_test_0001");
        assert_eq!(decoded.prepared().snapshot().0, "snap_test_0001");

        let normalized = serde_json::to_value(decoded).expect("serialize normalized journal");
        assert_eq!(normalized["schema"], INDEX_PUBLICATION_JOURNAL_SCHEMA_V2);
        assert_eq!(normalized["prepared"], "snap_test_0001");
        assert!(normalized.get("snapshot").is_none());
    }

    #[test]
    fn unknown_journal_schema_is_rejected() {
        let value = json!({
            "schema": "athanor.index_publication.v999",
            "prepared": "snap_test_unknown",
            "id": "publication-unknown",
            "read_model": "project/.athanor/generated/current/jsonl",
            "index_state": "project/.athanor/state/index-state.json"
        });

        let error = serde_json::from_value::<IndexPublicationJournal>(value)
            .expect_err("unknown publication journal schema must fail closed");
        assert!(
            error
                .to_string()
                .contains("unsupported publication journal schema")
        );
    }

    #[test]
    fn journal_persistence_round_trip_is_atomic_and_clearable() {
        let root = test_root("journal");
        let journal = IndexPublicationJournal::with_id(
            &root,
            PreparedSnapshot::new(SnapshotId("snap_test_persisted".to_string())),
            "publication-persisted".to_string(),
        );

        journal.write().expect("write typed publication journal");
        assert!(IndexPublicationJournal::path(&root).is_file());
        let loaded = IndexPublicationJournal::load(&root)
            .expect("load typed publication journal")
            .expect("typed publication journal exists");
        assert_eq!(loaded, journal);

        loaded.clear().expect("clear typed publication journal");
        assert!(!IndexPublicationJournal::path(&root).exists());
        assert!(
            IndexPublicationJournal::load(&root)
                .expect("load cleared publication journal")
                .is_none()
        );
        fs::remove_dir_all(root).expect("remove journal test root");
    }

    #[test]
    fn journal_load_rejects_paths_outside_expected_project_artifacts() {
        let root = test_root("path-validation");
        let path = IndexPublicationJournal::path(&root);
        fs::create_dir_all(path.parent().expect("journal parent")).unwrap();
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V2,
                "prepared": "snap_test_escape",
                "id": "publication-escape",
                "read_model": root.join("../outside"),
                "index_state": root.join(".athanor/state/index-state.json")
            }))
            .unwrap(),
        )
        .unwrap();

        let error = IndexPublicationJournal::load(&root)
            .expect_err("journal path traversal must fail closed");
        assert!(error.to_string().contains("does not match expected"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn publication_id_rejects_path_separators() {
        let root = test_root("id-validation");
        let journal = IndexPublicationJournal::with_id(
            &root,
            PreparedSnapshot::new(SnapshotId("snap_test_id".to_string())),
            "../../escape".to_string(),
        );
        let error = journal
            .write()
            .expect_err("publication id path traversal must fail closed");
        assert!(error.to_string().contains("invalid publication id"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn typed_recovery_rolls_back_uncommitted_publication() {
        let root = test_root("rollback");
        let output_dir = expected_read_model(&root);
        let state_path = expected_index_state(&root);
        fs::create_dir_all(&output_dir).unwrap();
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_old"}"#,
        )
        .unwrap();
        fs::write(&state_path, r#"{"snapshot":"snap_old"}"#).unwrap();

        let journal = IndexPublicationJournal::with_id(
            &root,
            PreparedSnapshot::new(SnapshotId("snap_jsonl_00000001".to_string())),
            "test-publication".to_string(),
        );
        let (_, read_backup) = publication_paths(&output_dir, journal.id()).unwrap();
        let (_, state_backup) = publication_paths(&state_path, journal.id()).unwrap();
        fs::rename(&output_dir, &read_backup).unwrap();
        fs::rename(&state_path, &state_backup).unwrap();
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(&state_path, r#"{"snapshot":"snap_jsonl_00000001"}"#).unwrap();
        journal.write().unwrap();

        let store = AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        recover_interrupted_publication(&root, &store)
            .await
            .unwrap();

        assert_eq!(
            read_model_snapshot(&output_dir).as_deref(),
            Some("snap_old")
        );
        assert_eq!(state_snapshot(&state_path).as_deref(), Some("snap_old"));
        assert!(!IndexPublicationJournal::path(&root).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn typed_recovery_finalizes_after_canonical_commit() {
        let root = test_root("finalize");
        let output_dir = expected_read_model(&root);
        let state_path = expected_index_state(&root);
        fs::create_dir_all(&output_dir).unwrap();
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            output_dir.join("manifest.json"),
            r#"{"snapshot":"snap_jsonl_00000001"}"#,
        )
        .unwrap();
        fs::write(&state_path, r#"{"snapshot":"snap_jsonl_00000001"}"#).unwrap();

        let store = AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_test".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .unwrap();
        let prepared = PreparedSnapshot::new(snapshot.clone());
        let journal = IndexPublicationJournal::with_id(
            &root,
            prepared,
            "test-publication".to_string(),
        );
        let (_, read_backup) = publication_paths(&output_dir, journal.id()).unwrap();
        let (_, state_backup) = publication_paths(&state_path, journal.id()).unwrap();
        fs::create_dir_all(&read_backup).unwrap();
        fs::write(
            read_backup.join("manifest.json"),
            r#"{"snapshot":"snap_old"}"#,
        )
        .unwrap();
        fs::write(&state_backup, r#"{"snapshot":"snap_old"}"#).unwrap();
        journal.write().unwrap();
        store.commit_snapshot(snapshot).await.unwrap();

        recover_interrupted_publication(&root, &store)
            .await
            .unwrap();

        assert_eq!(
            read_model_snapshot(&output_dir).as_deref(),
            Some("snap_jsonl_00000001")
        );
        assert_eq!(
            state_snapshot(&state_path).as_deref(),
            Some("snap_jsonl_00000001")
        );
        assert!(!read_backup.exists());
        assert!(!state_backup.exists());
        assert!(!IndexPublicationJournal::path(&root).exists());
        fs::remove_dir_all(root).unwrap();
    }
}
