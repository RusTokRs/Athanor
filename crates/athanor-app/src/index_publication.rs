use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext};
use athanor_domain::{GenerationId, SnapshotId};
use athanor_projector_support::replace_output_file;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::index_current::IndexCurrent;
use crate::{AthanorStore, IndexPipelineOutput, IndexStateStore};

mod legacy {
    include!("index_publication_snapshot.rs");
}

pub(crate) use legacy::IndexPublicationOutcome;

const INDEX_CURRENT_PUBLICATION_SCHEMA: &str = "athanor.index_current_publication.v1";
const LEGACY_READ_MODEL_PATH: &str = ".athanor/generated/current/jsonl";
const LEGACY_INDEX_STATE_PATH: &str = ".athanor/state/index-state.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexCurrentPublicationJournal {
    schema: String,
    snapshot: SnapshotId,
    generation: GenerationId,
}

impl IndexCurrentPublicationJournal {
    fn new(snapshot: SnapshotId) -> Self {
        let generation = GenerationId::for_snapshot(&snapshot);
        Self {
            schema: INDEX_CURRENT_PUBLICATION_SCHEMA.to_string(),
            snapshot,
            generation,
        }
    }

    fn load(root: &Path) -> Result<Option<Self>> {
        let path = Self::path(root);
        if !path.exists() {
            return Ok(None);
        }
        let journal: Self = serde_json::from_slice(&fs::read(&path).with_context(|| {
            format!(
                "failed to read index current publication journal {}",
                path.display()
            )
        })?)
        .with_context(|| {
            format!(
                "failed to parse index current publication journal {}",
                path.display()
            )
        })?;
        journal.validate()?;
        Ok(Some(journal))
    }

    fn write(&self, root: &Path) -> Result<()> {
        self.validate()?;
        if let Some(existing) = Self::load(root)? {
            if existing == *self {
                return Ok(());
            }
            bail!(
                "index current publication for {} is still pending",
                existing.snapshot.0
            );
        }
        let path = Self::path(root);
        let content = serde_json::to_string_pretty(self)
            .context("failed to serialize index current publication journal")?;
        replace_output_file(&path, &content, "index current publication journal")
            .map_err(anyhow::Error::new)
    }

    fn clear(&self, root: &Path) -> Result<()> {
        let path = Self::path(root);
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!(
                    "failed to clear index current publication journal {}",
                    path.display()
                )
            })?;
        }
        Ok(())
    }

    fn path(root: &Path) -> PathBuf {
        root.join(".athanor/state/index-current-publication.json")
    }

    fn validate(&self) -> Result<()> {
        if self.schema != INDEX_CURRENT_PUBLICATION_SCHEMA {
            bail!(
                "unsupported index current publication journal schema `{}`",
                self.schema
            );
        }
        if self.snapshot.0.trim().is_empty() {
            bail!("index current publication journal has an empty snapshot identity");
        }
        let expected = GenerationId::for_snapshot(&self.snapshot);
        if self.generation != expected {
            bail!(
                "index current publication generation `{}` does not match snapshot `{}`",
                self.generation,
                self.snapshot.0
            );
        }
        Ok(())
    }
}

pub(crate) async fn publish_index_snapshot(
    root: &Path,
    store: &AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
    output: &IndexPipelineOutput,
    snapshot: SnapshotId,
    operation: &OperationContext,
) -> Result<IndexPublicationOutcome> {
    if !uses_legacy_runtime_layout(root, state_store, output_dir) {
        return legacy::publish_index_snapshot(
            root,
            store,
            state_store,
            output_dir,
            output,
            snapshot,
            operation,
        )
        .await;
    }

    let pointer_journal = IndexCurrentPublicationJournal::new(snapshot.clone());
    if let Err(error) = pointer_journal.write(root) {
        return abort_snapshot_with_error(store, &snapshot, error).await;
    }

    match legacy::publish_index_snapshot(
        root,
        store,
        state_store,
        output_dir,
        output,
        snapshot.clone(),
        operation,
    )
    .await
    {
        Ok(outcome) => {
            publish_current_generation(root, &pointer_journal).with_context(|| {
                format!(
                    "canonical snapshot {} committed but index current pointer remains pending",
                    snapshot.0
                )
            })?;
            pointer_journal.clear(root)?;
            Ok(outcome)
        }
        Err(error) => match exact_snapshot_is_committed(store, &snapshot).await {
            Ok(true) => Err(error.context(format!(
                "canonical snapshot {} committed; index current pointer recovery remains pending",
                snapshot.0
            ))),
            Ok(false) => {
                if let Err(abort_error) = abort_uncommitted_snapshot(store, &snapshot).await {
                    return Err(error.context(format!(
                        "failed to abort uncommitted index current snapshot: {abort_error}"
                    )));
                }
                if let Err(clear_error) = pointer_journal.clear(root) {
                    return Err(error.context(format!(
                        "failed to clear uncommitted index current publication: {clear_error}"
                    )));
                }
                Err(error)
            }
            Err(status_error) => Err(error.context(format!(
                "failed to determine index current publication state: {status_error}"
            ))),
        },
    }
}

pub(crate) async fn recover_interrupted_publication(
    root: &Path,
    store: &AthanorStore,
) -> Result<()> {
    legacy::recover_interrupted_publication(root, store).await?;

    let Some(pointer_journal) = IndexCurrentPublicationJournal::load(root)? else {
        return Ok(());
    };
    if exact_snapshot_is_committed(store, &pointer_journal.snapshot).await? {
        publish_current_generation(root, &pointer_journal)?;
    } else {
        abort_uncommitted_snapshot(store, &pointer_journal.snapshot).await?;
    }
    pointer_journal.clear(root)
}

fn uses_legacy_runtime_layout(
    root: &Path,
    state_store: &IndexStateStore,
    output_dir: &Path,
) -> bool {
    state_store.path() == root.join(LEGACY_INDEX_STATE_PATH)
        && output_dir == root.join(LEGACY_READ_MODEL_PATH)
}

fn publish_current_generation(root: &Path, journal: &IndexCurrentPublicationJournal) -> Result<()> {
    journal.validate()?;
    let migration_current = IndexCurrent::for_snapshot(journal.snapshot.clone());
    if migration_current.generation() != &journal.generation {
        bail!(
            "index current pointer generation {} does not match journal {}",
            migration_current.generation(),
            journal.generation
        );
    }

    let source_read_model = root.join(LEGACY_READ_MODEL_PATH);
    let source_state = root.join(LEGACY_INDEX_STATE_PATH);
    validate_artifact_identity(
        &source_read_model.join("manifest.json"),
        crate::read_model::JSONL_MANIFEST_SCHEMA,
        &journal.snapshot,
        &journal.generation,
        "legacy read-model manifest",
    )?;
    validate_artifact_identity(
        &source_state,
        crate::index_state::INDEX_STATE_SCHEMA,
        &journal.snapshot,
        &journal.generation,
        "legacy index state",
    )?;

    let target_read_model = migration_current.read_model_path(root);
    let target_state = migration_current.index_state_path(root);
    publish_immutable_directory(&source_read_model, &target_read_model)?;
    publish_immutable_file(&source_state, &target_state)?;

    validate_artifact_identity(
        &target_read_model.join("manifest.json"),
        crate::read_model::JSONL_MANIFEST_SCHEMA,
        &journal.snapshot,
        &journal.generation,
        "immutable read-model manifest",
    )?;
    validate_artifact_identity(
        &target_state,
        crate::index_state::INDEX_STATE_SCHEMA,
        &journal.snapshot,
        &journal.generation,
        "immutable index state",
    )?;
    crate::artifact_checksum::validate_read_model_matches(
        &source_read_model,
        &target_read_model,
    )?;
    crate::artifact_checksum::validate_file_matches(
        &source_state,
        &target_state,
        "index state",
    )?;

    let read_model_manifest_sha256 =
        crate::artifact_checksum::seal_read_model(&target_read_model)?;
    let index_state_sha256 = crate::artifact_checksum::sha256_file(&target_state)?;
    let current = IndexCurrent::for_snapshot_with_checksums(
        journal.snapshot.clone(),
        read_model_manifest_sha256,
        index_state_sha256,
    );
    current.write(root)
}

fn publish_immutable_directory(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        if target.is_dir() {
            return Ok(());
        }
        bail!(
            "immutable index generation path is not a directory: {}",
            target.display()
        );
    }
    let parent = target.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "immutable index generation path has no parent: {}",
            target.display()
        )
    })?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid generation directory {}", target.display()))?;
    let staging = parent.join(format!(".{name}.staging-current-{}", publication_nonce()));
    remove_path_if_exists(&staging)?;
    fs::create_dir(&staging)
        .with_context(|| format!("failed to create generation staging {}", staging.display()))?;
    if let Err(error) = copy_directory_contents(source, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    match fs::rename(&staging, target) {
        Ok(()) => Ok(()),
        Err(_) if target.is_dir() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(error).with_context(|| {
                format!(
                    "failed to publish immutable index generation {}",
                    target.display()
                )
            })
        }
    }
}

fn copy_directory_contents(source: &Path, target: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read source directory {}", source.display()))?
    {
        let entry = entry.with_context(|| format!("failed to inspect {}", source.display()))?;
        let file_type = entry.file_type().with_context(|| {
            format!(
                "failed to inspect source artifact {}",
                entry.path().display()
            )
        })?;
        let destination = target.join(entry.file_name());
        if file_type.is_symlink() {
            bail!(
                "index generation source contains a symlink: {}",
                entry.path().display()
            );
        }
        if file_type.is_dir() {
            fs::create_dir(&destination).with_context(|| {
                format!(
                    "failed to create generation directory {}",
                    destination.display()
                )
            })?;
            copy_directory_contents(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &destination).with_context(|| {
                format!(
                    "failed to copy generation artifact {} to {}",
                    entry.path().display(),
                    destination.display()
                )
            })?;
        } else {
            bail!(
                "index generation source contains an unsupported entry: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn publish_immutable_file(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        if target.is_file() {
            return Ok(());
        }
        bail!(
            "immutable index state path is not a file: {}",
            target.display()
        );
    }
    let parent = target.parent().ok_or_else(|| {
        anyhow::anyhow!("immutable index state has no parent: {}", target.display())
    })?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid immutable index state {}", target.display()))?;
    let staging = parent.join(format!(".{name}.staging-current-{}", publication_nonce()));
    remove_path_if_exists(&staging)?;
    fs::copy(source, &staging).with_context(|| {
        format!(
            "failed to stage immutable index state {} from {}",
            staging.display(),
            source.display()
        )
    })?;
    match fs::rename(&staging, target) {
        Ok(()) => Ok(()),
        Err(_) if target.is_file() => {
            let _ = fs::remove_file(&staging);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&staging);
            Err(error).with_context(|| {
                format!(
                    "failed to publish immutable index state {}",
                    target.display()
                )
            })
        }
    }
}

fn validate_artifact_identity(
    path: &Path,
    expected_schema: &str,
    expected_snapshot: &SnapshotId,
    expected_generation: &GenerationId,
    label: &str,
) -> Result<()> {
    let value: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {label} {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {label} {}", path.display()))?;
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{label} {} has no schema", path.display()))?;
    if schema != expected_schema {
        bail!(
            "{label} {} has schema `{schema}`, expected `{expected_schema}`",
            path.display()
        );
    }
    let snapshot = value
        .get("snapshot")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{label} {} has no snapshot identity", path.display()))?;
    if snapshot != expected_snapshot.0.as_str() {
        bail!(
            "{label} {} identifies snapshot `{snapshot}`, expected `{}`",
            path.display(),
            expected_snapshot.0
        );
    }
    let generation = value
        .get("generation")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{label} {} has no generation identity", path.display()))?;
    if generation != expected_generation.as_str() {
        bail!(
            "{label} {} identifies generation `{generation}`, expected `{expected_generation}`",
            path.display()
        );
    }
    Ok(())
}

async fn exact_snapshot_is_committed(store: &AthanorStore, snapshot: &SnapshotId) -> Result<bool> {
    match store.load_snapshot(snapshot).await {
        Ok(Some(canonical)) => {
            if canonical.snapshot.as_ref() != Some(snapshot) {
                bail!(
                    "exact canonical snapshot {} returned identity {:?}",
                    snapshot.0,
                    canonical.snapshot
                );
            }
            Ok(true)
        }
        Ok(None) | Err(CoreError::NotFound(_)) | Err(CoreError::SnapshotNotCommitted(_)) => {
            Ok(false)
        }
        Err(error) => Err(anyhow::Error::new(error).context(format!(
            "failed to probe exact canonical snapshot {}",
            snapshot.0
        ))),
    }
}

async fn abort_uncommitted_snapshot(store: &AthanorStore, snapshot: &SnapshotId) -> Result<()> {
    match store.abort_snapshot(snapshot.clone()).await {
        Ok(()) | Err(CoreError::NotFound(_)) => Ok(()),
        Err(error) => Err(anyhow::Error::new(error).context(format!(
            "failed to abort uncommitted snapshot {}",
            snapshot.0
        ))),
    }
}

async fn abort_snapshot_with_error<T>(
    store: &AthanorStore,
    snapshot: &SnapshotId,
    error: anyhow::Error,
) -> Result<T> {
    match store.abort_snapshot(snapshot.clone()).await {
        Ok(()) | Err(CoreError::NotFound(_)) => Err(error),
        Err(abort_error) => Err(error.context(format!(
            "failed to abort snapshot {} after index current journal error: {abort_error}",
            snapshot.0
        ))),
    }
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove stale staging {}", path.display()))?;
    } else if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove stale staging {}", path.display()))?;
    }
    Ok(())
}

fn publication_nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_journal_round_trip_preserves_generation_identity() {
        let root = std::env::temp_dir().join(format!(
            "athanor-index-current-journal-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = IndexCurrentPublicationJournal::new(SnapshotId("snap_test".to_string()));

        journal.write(&root).unwrap();
        assert_eq!(
            IndexCurrentPublicationJournal::load(&root).unwrap(),
            Some(journal.clone())
        );
        journal.clear(&root).unwrap();
        assert!(!IndexCurrentPublicationJournal::path(&root).exists());

        fs::remove_dir_all(root).unwrap();
    }
}
