use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{
    CanonicalSnapshotStore, CoreError, OperationContext, PreparedSnapshot,
    PreparedSnapshotPublication,
};

use crate::index_publication_inner::IndexPublicationJournal;
use crate::{AthanorStore, IndexPipelineOutput, IndexStateStore};

pub(crate) use crate::index_publication_inner::IndexPublicationOutcome;

/// Publishes one prepared index generation and guarantees cleanup for failures that happen before a
/// durable recovery journal exists.
///
/// The inner coordinator already owns rollback once journal persistence succeeds. This wrapper closes
/// the earlier failure window: if the journal path does not exist after an error, canonical publish
/// could not have started, so the prepared snapshot is safe to abort without probing backend status.
pub(crate) async fn publish_prepared_index(
    root: &Path,
    store: &AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
    output: &IndexPipelineOutput,
    prepared: PreparedSnapshot,
    operation: &OperationContext,
) -> Result<IndexPublicationOutcome> {
    match crate::index_publication_inner::publish_prepared_index(
        root,
        store,
        state_store,
        output_dir,
        output,
        prepared.clone(),
        operation,
    )
    .await
    {
        Ok(publication) => Ok(publication),
        Err(error) => cleanup_after_coordinator_error(root, store, &prepared, error).await,
    }
}

/// Validates every recovery-controlled path and identity before the inner coordinator removes or
/// renames files.
///
/// Malformed artifact types, schemas, or snapshot identities fail closed while the durable journal is
/// still present. Recovery never replaces a read-model directory with a file, restores mixed backup
/// generations, or clears a committed journal whose current application artifacts point elsewhere.
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

    validate_recovery_artifacts(&journal, committed)?;
    crate::index_publication_inner::recover_interrupted_publication(root, store).await
}

async fn cleanup_after_coordinator_error(
    root: &Path,
    store: &AthanorStore,
    prepared: &PreparedSnapshot,
    error: anyhow::Error,
) -> Result<IndexPublicationOutcome> {
    if !IndexPublicationJournal::path(root).exists() {
        return abort_prepared_with_error(store, prepared, error).await;
    }

    let latest = match store.load_latest_snapshot().await {
        Ok(latest) => latest,
        Err(status_error) => {
            return Err(error.context(format!(
                "failed to determine publication state after coordinator error: {status_error}"
            )));
        }
    };
    let committed = latest
        .and_then(|snapshot| snapshot.snapshot)
        .is_some_and(|snapshot| snapshot == *prepared.snapshot());
    if committed {
        return Err(error);
    }

    abort_prepared_with_error(store, prepared, error).await
}

async fn abort_prepared_with_error(
    store: &AthanorStore,
    prepared: &PreparedSnapshot,
    error: anyhow::Error,
) -> Result<IndexPublicationOutcome> {
    match store.abort_prepared(prepared).await {
        Ok(()) | Err(CoreError::NotFound(_)) => Err(error),
        Err(abort_error) => Err(error.context(format!(
            "failed to abort prepared snapshot {} after coordinator error: {abort_error}",
            prepared.snapshot().0
        ))),
    }
}

fn validate_recovery_artifacts(
    journal: &IndexPublicationJournal,
    committed: bool,
) -> Result<()> {
    let (read_staging, read_backup) = recovery_paths(journal.read_model(), journal.id())?;
    let (state_staging, state_backup) = recovery_paths(journal.index_state(), journal.id())?;

    require_directory_if_present(journal.read_model(), "current read model")?;
    require_directory_if_present(&read_staging, "staged read model")?;
    require_directory_if_present(&read_backup, "read model backup")?;
    require_file_if_present(journal.index_state(), "current index state")?;
    require_file_if_present(&state_staging, "staged index state")?;
    require_file_if_present(&state_backup, "index state backup")?;

    let expected = journal.snapshot().0.as_str();
    let read_current = read_model_identity_if_present(journal.read_model(), "current read model")?;
    let read_staged = read_model_identity_if_present(&read_staging, "staged read model")?;
    let read_previous = read_model_identity_if_present(&read_backup, "read model backup")?;
    let state_current = state_identity_if_present(journal.index_state(), "current index state")?;
    let state_staged = state_identity_if_present(&state_staging, "staged index state")?;
    let state_previous = state_identity_if_present(&state_backup, "index state backup")?;

    require_expected_if_present(read_staged.as_deref(), expected, "staged read model")?;
    require_expected_if_present(state_staged.as_deref(), expected, "staged index state")?;

    if committed {
        require_expected(read_current.as_deref(), expected, "current read model")?;
        require_expected(state_current.as_deref(), expected, "current index state")?;
    } else {
        if read_backup.exists() {
            require_expected_if_present(
                read_current.as_deref(),
                expected,
                "current read model replaced during rollback",
            )?;
        }
        if state_backup.exists() {
            require_expected_if_present(
                state_current.as_deref(),
                expected,
                "current index state replaced during rollback",
            )?;
        }
    }

    require_matching_if_both_present(
        read_previous.as_deref(),
        state_previous.as_deref(),
        "read-model and index-state backups",
    )
}

fn recovery_paths(path: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
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

fn read_model_identity_if_present(path: &Path, label: &str) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let manifest = path.join("manifest.json");
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(&manifest)
            .with_context(|| format!("failed to read recovery {label} manifest {}", manifest.display()))?,
    )
    .with_context(|| format!("failed to parse recovery {label} manifest {}", manifest.display()))?;
    artifact_identity(
        &value,
        crate::read_model::JSONL_MANIFEST_SCHEMA,
        label,
        &manifest,
    )
    .map(Some)
}

fn state_identity_if_present(path: &Path, label: &str) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(path)
            .with_context(|| format!("failed to read recovery {label} {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse recovery {label} {}", path.display()))?;
    artifact_identity(
        &value,
        crate::index_state::INDEX_STATE_SCHEMA,
        label,
        path,
    )
    .map(Some)
}

fn artifact_identity(
    value: &serde_json::Value,
    expected_schema: &str,
    label: &str,
    path: &Path,
) -> Result<String> {
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("publication recovery {label} {} has no schema", path.display()))?;
    if schema != expected_schema {
        bail!(
            "publication recovery {label} {} has schema `{schema}`, expected `{expected_schema}`",
            path.display()
        );
    }
    value
        .get("snapshot")
        .and_then(serde_json::Value::as_str)
        .filter(|snapshot| !snapshot.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "publication recovery {label} {} has no non-empty snapshot identity",
                path.display()
            )
        })
}

fn require_expected(actual: Option<&str>, expected: &str, label: &str) -> Result<()> {
    let actual = actual.ok_or_else(|| {
        anyhow::anyhow!("publication recovery {label} is missing for snapshot {expected}")
    })?;
    require_expected_if_present(Some(actual), expected, label)
}

fn require_expected_if_present(actual: Option<&str>, expected: &str, label: &str) -> Result<()> {
    if let Some(actual) = actual {
        if actual != expected {
            bail!(
                "publication recovery {label} snapshot `{actual}` does not match journal `{expected}`"
            );
        }
    }
    Ok(())
}

fn require_matching_if_both_present(
    left: Option<&str>,
    right: Option<&str>,
    label: &str,
) -> Result<()> {
    if let (Some(left), Some(right)) = (left, right) {
        if left != right {
            bail!(
                "publication recovery {label} refer to different snapshots `{left}` and `{right}`"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use athanor_core::{
        CanonicalSnapshotStore, KnowledgeStore, OperationContext, PreparedSnapshotPublication,
        SnapshotBatch,
    };
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;

    use super::publish_prepared_index;
    use crate::{
        AffectedFileSet, AthanorStore, IndexPipelineMetrics, IndexPipelineOutput, IndexStateStore,
    };

    #[tokio::test]
    async fn journal_write_failure_aborts_prepared_snapshot() {
        let root = test_root("journal-write-failure");
        fs::create_dir_all(root.join(".athanor")).expect("create project metadata directory");
        fs::write(root.join(".athanor/state"), "blocked")
            .expect("block publication journal directory");

        let store = AthanorStore::new(JsonlKnowledgeStore::new(
            root.join(".athanor/store/canonical/jsonl"),
        ));
        let snapshot = store
            .begin_snapshot(
                RepoId("repo_journal_write_failure".to_string()),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot: None,
                    working_tree: true,
                },
            )
            .await
            .expect("begin snapshot");
        store
            .put_snapshot(snapshot.clone(), SnapshotBatch::default())
            .await
            .expect("write snapshot batch");
        let operation = OperationContext::new("test.publication.journal-write-failure");
        let prepared = store
            .prepare_publication(snapshot.clone(), &operation)
            .await
            .expect("prepare snapshot");
        let output = IndexPipelineOutput {
            snapshot: snapshot.clone(),
            files: Vec::new(),
            entities: Vec::new(),
            facts: Vec::new(),
            relations: Vec::new(),
            diagnostics: Vec::new(),
            affected_files: AffectedFileSet::default(),
            metrics: IndexPipelineMetrics::default(),
        };
        let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
        let output_dir = root.join(".athanor/generated/current/jsonl");

        let error = publish_prepared_index(
            &root,
            &store,
            &state_store,
            &output_dir,
            &output,
            prepared,
            &operation,
        )
        .await
        .expect_err("journal persistence failure must fail publication");

        assert!(
            error
                .chain()
                .any(|cause| cause.to_string().contains("publication journal directory"))
        );
        assert!(
            store
                .load_snapshot(&snapshot)
                .await
                .expect("load aborted snapshot")
                .is_none(),
            "prepared snapshot must be removed when no recovery journal was created"
        );
        assert!(
            store
                .load_latest_snapshot()
                .await
                .expect("load latest snapshot")
                .is_none()
        );

        fs::remove_dir_all(root).expect("remove journal failure test root");
    }

    fn test_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_nanos();
        std::env::temp_dir().join(format!("athanor-publication-{label}-{nonce}"))
    }
}
