use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    SnapshotBatch,
};
use athanor_domain::{GenerationId, SnapshotId};

use crate::index_publication_journal::IndexPublicationJournal;
use crate::{
    AthanorStore, IndexPipelineOutput, IndexState, IndexStateStore, JsonlReadModelReport,
    JsonlReadModelWriter,
};

#[derive(Debug)]
pub(crate) struct IndexPublicationOutcome {
    pub(crate) read_model: JsonlReadModelReport,
    pub(crate) read_model_write_ms: u64,
    pub(crate) index_state_write_ms: u64,
}

/// Stages application artefacts behind a durable snapshot-native journal, then atomically publishes
/// the complete canonical batch and commit marker.
pub(crate) async fn publish_index_snapshot(
    root: &Path,
    store: &AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
    output: &IndexPipelineOutput,
    snapshot: SnapshotId,
    operation: &OperationContext,
) -> Result<IndexPublicationOutcome> {
    if snapshot != output.snapshot {
        bail!(
            "publication snapshot {} does not match pipeline output {}",
            snapshot.0,
            output.snapshot.0
        );
    }

    let journal = IndexPublicationJournal::new(root, snapshot.clone());
    if let Err(error) = journal.write() {
        return abort_snapshot_with_error(store, &snapshot, error).await;
    }

    let read_model_started = Instant::now();
    let prepared_read_model = match JsonlReadModelWriter::new(output_dir)
        .prepare_with_publication_id(output, journal.id())
    {
        Ok(prepared_read_model) => prepared_read_model,
        Err(error) => {
            let _ = journal.clear();
            return abort_snapshot_with_error(store, &snapshot, error).await;
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
            return abort_snapshot_with_error(store, &snapshot, error).await;
        }
    };
    let index_state_write_ms = elapsed_ms(index_state_started.elapsed());

    let batch = SnapshotBatch {
        entities: output.entities.clone(),
        facts: output.facts.clone(),
        relations: output.relations.clone(),
        diagnostics: output.diagnostics.clone(),
    };
    if let Err(publish_error) = store
        .publish_snapshot_batch_with_context(snapshot.clone(), batch, operation)
        .await
    {
        let error = anyhow::Error::new(publish_error)
            .context("failed to publish canonical snapshot after read model and index state");
        match exact_snapshot_is_committed(store, &snapshot).await {
            Ok(true) => {
                return Err(error);
            }
            Ok(false) => {
                let state_rollback_error = prepared_index_state.rollback().err();
                let read_model_rollback_error = prepared_read_model.rollback().err();
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
                return abort_snapshot_with_error(store, &snapshot, error).await;
            }
            Err(status_error) => {
                return Err(error.context(format!(
                    "failed to determine exact publication state after atomic coordinator error: {status_error}"
                )));
            }
        }
    }

    let read_model = prepared_read_model.finalize()?;
    prepared_index_state.finalize()?;
    journal.clear()?;

    Ok(IndexPublicationOutcome {
        read_model,
        read_model_write_ms,
        index_state_write_ms,
    })
}

/// Recovers one interrupted publication using the snapshot identity stored in the durable journal.
pub(crate) async fn recover_interrupted_publication(
    root: &Path,
    store: &AthanorStore,
) -> Result<()> {
    let Some(journal) = IndexPublicationJournal::load(root)? else {
        return Ok(());
    };
    let committed = exact_snapshot_is_committed(store, journal.snapshot()).await?;

    validate_recovery_artifacts(&journal, committed)?;
    if committed {
        cleanup_publication_artifacts(&journal)?;
    } else {
        rollback_publication_artifacts(&journal)?;
        match store.abort_snapshot(journal.snapshot().clone()).await {
            Ok(()) | Err(CoreError::NotFound(_)) => {}
            Err(error) => {
                return Err(anyhow::Error::new(error).context("failed to abort recovered snapshot"));
            }
        }
    }

    journal.clear()
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

async fn abort_snapshot_with_error<T>(
    store: &AthanorStore,
    snapshot: &SnapshotId,
    error: anyhow::Error,
) -> Result<T> {
    match store.abort_snapshot(snapshot.clone()).await {
        Ok(()) | Err(CoreError::NotFound(_)) => Err(error),
        Err(abort_error) => Err(error.context(format!(
            "failed to abort snapshot {} after coordinator error: {abort_error}",
            snapshot.0
        ))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactIdentity {
    snapshot: String,
    generation: Option<String>,
}

fn validate_recovery_artifacts(journal: &IndexPublicationJournal, committed: bool) -> Result<()> {
    let (read_staging, read_backup) = publication_paths(journal.read_model(), journal.id())?;
    let (state_staging, state_backup) = publication_paths(journal.index_state(), journal.id())?;

    require_directory_if_present(journal.read_model(), "current read model")?;
    require_directory_if_present(&read_staging, "staged read model")?;
    require_directory_if_present(&read_backup, "read model backup")?;
    require_file_if_present(journal.index_state(), "current index state")?;
    require_file_if_present(&state_staging, "staged index state")?;
    require_file_if_present(&state_backup, "index state backup")?;

    let expected_snapshot = journal.snapshot().0.as_str();
    let expected_generation = journal.generation().as_str();
    let require_generation = journal.requires_generation_identity();
    let read_current = read_model_identity_if_present(journal.read_model(), "current read model")?;
    let read_staged = read_model_identity_if_present(&read_staging, "staged read model")?;
    let read_previous = read_model_identity_if_present(&read_backup, "read model backup")?;
    let current_state_schema = if committed || state_backup.exists() {
        StateSchemaPolicy::ExactCurrent
    } else {
        StateSchemaPolicy::VersionedHistorical
    };
    let state_current = state_identity_if_present(
        journal.index_state(),
        "current index state",
        current_state_schema,
    )?;
    let state_staged = state_identity_if_present(
        &state_staging,
        "staged index state",
        StateSchemaPolicy::ExactCurrent,
    )?;
    let state_previous = state_identity_if_present(
        &state_backup,
        "index state backup",
        StateSchemaPolicy::VersionedHistorical,
    )?;

    require_expected_identity_if_present(
        read_staged.as_ref(),
        expected_snapshot,
        expected_generation,
        require_generation,
        "staged read model",
    )?;
    require_expected_identity_if_present(
        state_staged.as_ref(),
        expected_snapshot,
        expected_generation,
        require_generation,
        "staged index state",
    )?;

    if committed {
        require_expected_identity(
            read_current.as_ref(),
            expected_snapshot,
            expected_generation,
            require_generation,
            "current read model",
        )?;
        require_expected_identity(
            state_current.as_ref(),
            expected_snapshot,
            expected_generation,
            require_generation,
            "current index state",
        )?;
    } else {
        if read_backup.exists() {
            require_expected_identity_if_present(
                read_current.as_ref(),
                expected_snapshot,
                expected_generation,
                require_generation,
                "current read model replaced during rollback",
            )?;
        }
        if state_backup.exists() {
            require_expected_identity_if_present(
                state_current.as_ref(),
                expected_snapshot,
                expected_generation,
                require_generation,
                "current index state replaced during rollback",
            )?;
        }
    }

    require_matching_if_both_present(
        read_previous.as_ref(),
        state_previous.as_ref(),
        "read-model and index-state backups",
    )
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
    restore_publication_directory(journal.read_model(), journal.id(), &journal.snapshot().0)?;
    restore_publication_file(journal.index_state(), journal.id(), &journal.snapshot().0)
}

fn publication_paths(path: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
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
    } else if read_model_identity_if_present(path, "current read model")?
        .is_some_and(|current| current.snapshot == snapshot)
    {
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
    } else if state_identity_if_present(
        path,
        "current index state",
        StateSchemaPolicy::VersionedHistorical,
    )?
    .is_some_and(|current| current.snapshot == snapshot)
    {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn require_directory_if_present(path: &Path, label: &str) -> Result<()> {
    if path.exists() && !path.is_dir() {
        bail!(
            "publication recovery {label} {} must be a directory",
            path.display()
        );
    }
    Ok(())
}

fn require_file_if_present(path: &Path, label: &str) -> Result<()> {
    if path.exists() && !path.is_file() {
        bail!(
            "publication recovery {label} {} must be a regular file",
            path.display()
        );
    }
    Ok(())
}

fn read_model_identity_if_present(path: &Path, label: &str) -> Result<Option<ArtifactIdentity>> {
    if !path.exists() {
        return Ok(None);
    }
    let manifest = path.join("manifest.json");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).with_context(|| {
            format!(
                "failed to read recovery {label} manifest {}",
                manifest.display()
            )
        })?)
        .with_context(|| {
            format!(
                "failed to parse recovery {label} manifest {}",
                manifest.display()
            )
        })?;
    artifact_identity(
        &value,
        crate::read_model::JSONL_MANIFEST_SCHEMA,
        label,
        &manifest,
        SchemaPolicy::Exact,
    )
    .map(Some)
}

#[derive(Debug, Clone, Copy)]
enum StateSchemaPolicy {
    ExactCurrent,
    VersionedHistorical,
}

fn state_identity_if_present(
    path: &Path,
    label: &str,
    policy: StateSchemaPolicy,
) -> Result<Option<ArtifactIdentity>> {
    if !path.exists() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(path)
            .with_context(|| format!("failed to read recovery {label} {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse recovery {label} {}", path.display()))?;
    let schema_policy = match policy {
        StateSchemaPolicy::ExactCurrent => SchemaPolicy::Exact,
        StateSchemaPolicy::VersionedHistorical => SchemaPolicy::Versioned("athanor.index_state.v"),
    };
    artifact_identity(
        &value,
        crate::index_state::INDEX_STATE_SCHEMA,
        label,
        path,
        schema_policy,
    )
    .map(Some)
}

#[derive(Debug, Clone, Copy)]
enum SchemaPolicy {
    Exact,
    Versioned(&'static str),
}

fn artifact_identity(
    value: &serde_json::Value,
    expected_schema: &str,
    label: &str,
    path: &Path,
    policy: SchemaPolicy,
) -> Result<ArtifactIdentity> {
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "publication recovery {label} {} has no schema",
                path.display()
            )
        })?;
    let schema_valid = match policy {
        SchemaPolicy::Exact => schema == expected_schema,
        SchemaPolicy::Versioned(prefix) => is_versioned_schema(schema, prefix),
    };
    if !schema_valid {
        bail!(
            "publication recovery {label} {} has schema `{schema}`, expected `{expected_schema}`",
            path.display()
        );
    }
    let snapshot = value
        .get("snapshot")
        .and_then(serde_json::Value::as_str)
        .filter(|snapshot| !snapshot.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "publication recovery {label} {} has no non-empty snapshot identity",
                path.display()
            )
        })?;
    let generation = match value.get("generation") {
        None => None,
        Some(serde_json::Value::String(generation)) if !generation.trim().is_empty() => {
            let expected = GenerationId::for_snapshot(&SnapshotId(snapshot.clone()));
            if generation != expected.as_str() {
                bail!(
                    "publication recovery {label} {} generation `{generation}` does not match snapshot `{snapshot}`",
                    path.display()
                );
            }
            Some(generation.clone())
        }
        Some(_) => {
            bail!(
                "publication recovery {label} {} has an invalid generation identity",
                path.display()
            );
        }
    };
    Ok(ArtifactIdentity {
        snapshot,
        generation,
    })
}

fn is_versioned_schema(schema: &str, prefix: &str) -> bool {
    let Some(version) = schema.strip_prefix(prefix) else {
        return false;
    };
    let digits = version
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 {
        return false;
    }
    let suffix = &version[digits..];
    suffix.is_empty()
        || (suffix.starts_with('-')
            && suffix.len() > 1
            && suffix[1..]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
}

fn require_expected_identity(
    actual: Option<&ArtifactIdentity>,
    expected_snapshot: &str,
    expected_generation: &str,
    require_generation: bool,
    label: &str,
) -> Result<()> {
    let actual = actual.ok_or_else(|| {
        anyhow::anyhow!(
            "publication recovery {label} is missing for snapshot {expected_snapshot}"
        )
    })?;
    require_expected_identity_if_present(
        Some(actual),
        expected_snapshot,
        expected_generation,
        require_generation,
        label,
    )
}

fn require_expected_identity_if_present(
    actual: Option<&ArtifactIdentity>,
    expected_snapshot: &str,
    expected_generation: &str,
    require_generation: bool,
    label: &str,
) -> Result<()> {
    let Some(actual) = actual else {
        return Ok(());
    };
    if actual.snapshot != expected_snapshot {
        bail!(
            "publication recovery {label} snapshot `{}` does not match journal `{expected_snapshot}`",
            actual.snapshot
        );
    }
    match actual.generation.as_deref() {
        Some(generation) if generation != expected_generation => {
            bail!(
                "publication recovery {label} generation `{generation}` does not match journal `{expected_generation}`"
            );
        }
        None if require_generation => {
            bail!(
                "publication recovery {label} has no generation identity for journal `{expected_generation}`"
            );
        }
        _ => {}
    }
    Ok(())
}

fn require_matching_if_both_present(
    left: Option<&ArtifactIdentity>,
    right: Option<&ArtifactIdentity>,
    label: &str,
) -> Result<()> {
    if let (Some(left), Some(right)) = (left, right) {
        if left.snapshot != right.snapshot {
            bail!(
                "publication recovery {label} refer to different snapshots `{}` and `{}`",
                left.snapshot,
                right.snapshot
            );
        }
        if let (Some(left_generation), Some(right_generation)) =
            (left.generation.as_deref(), right.generation.as_deref())
            && left_generation != right_generation
        {
            bail!(
                "publication recovery {label} refer to different generations `{left_generation}` and `{right_generation}`"
            );
        }
    }
    Ok(())
}

fn elapsed_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn artifact_identity_rejects_generation_mismatch() {
        let value = json!({
            "schema": crate::read_model::JSONL_MANIFEST_SCHEMA,
            "snapshot": "snap_one",
            "generation": "gen_snap_two"
        });

        let error = artifact_identity(
            &value,
            crate::read_model::JSONL_MANIFEST_SCHEMA,
            "test artifact",
            Path::new("manifest.json"),
            SchemaPolicy::Exact,
        )
        .expect_err("mismatched generation must fail closed");

        assert!(error.to_string().contains("does not match snapshot"));
    }

    #[test]
    fn generation_aware_journal_requires_generation_in_artifact() {
        let identity = ArtifactIdentity {
            snapshot: "snap_one".to_string(),
            generation: None,
        };

        let error = require_expected_identity(
            Some(&identity),
            "snap_one",
            "gen_snap_one",
            true,
            "test artifact",
        )
        .expect_err("generation-aware recovery must reject legacy artifact");

        assert!(error.to_string().contains("has no generation identity"));
    }
}
