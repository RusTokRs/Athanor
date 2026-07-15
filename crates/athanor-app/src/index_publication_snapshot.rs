use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    PreparedSnapshot, SnapshotBatch,
};
use athanor_domain::SnapshotId;

use crate::index_publication_journal::IndexPublicationJournal;
use crate::{
    AthanorStore, IndexPipelineOutput, IndexState, IndexStateStore, JsonlReadModelWriter,
};

pub(crate) use crate::index_publication_atomic_legacy::{
    IndexPublicationOutcome, recover_interrupted_publication,
};

/// Stages application artefacts behind a durable snapshot-native journal, then atomically publishes
/// the complete canonical batch and commit marker.
///
/// The prepared handle is retained only at the compatibility call boundary. Journal persistence,
/// exact commit probing, and rollback authority use the canonical snapshot identity directly.
pub(crate) async fn publish_prepared_index(
    root: &Path,
    store: &AthanorStore,
    state_store: &IndexStateStore,
    output_dir: &Path,
    output: &IndexPipelineOutput,
    prepared: PreparedSnapshot,
    operation: &OperationContext,
) -> Result<IndexPublicationOutcome> {
    let snapshot = prepared.into_snapshot();
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
        let error = anyhow::Error::new(publish_error).context(
            "failed to publish prepared canonical snapshot after read model and index state",
        );
        match exact_snapshot_is_committed(store, &snapshot).await {
            Ok(true) => {
                // The exact canonical generation and marker are durable. Keep the journal and staged
                // application artefacts so recovery can finish them; abort would violate commit.
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

    // The journal remains until both application artefacts are finalized. If either finalize step
    // fails, exact canonical probing lets recovery keep this committed generation even when a
    // separate backend latest pointer was not updated.
    let read_model = prepared_read_model.finalize()?;
    prepared_index_state.finalize()?;
    journal.clear()?;

    Ok(IndexPublicationOutcome {
        read_model,
        read_model_write_ms,
        index_state_write_ms,
    })
}

async fn exact_snapshot_is_committed(
    store: &AthanorStore,
    snapshot: &SnapshotId,
) -> Result<bool> {
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
        Ok(None)
        | Err(CoreError::NotFound(_))
        | Err(CoreError::SnapshotNotCommitted(_)) => Ok(false),
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

fn elapsed_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}
