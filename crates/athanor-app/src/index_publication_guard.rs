use std::path::Path;

use anyhow::{Context, Result};
use athanor_core::{
    CanonicalSnapshotStore, CoreError, OperationContext, PreparedSnapshot,
    PreparedSnapshotPublication,
};

use crate::index_publication_inner::IndexPublicationJournal;
use crate::{AthanorStore, IndexPipelineOutput, IndexStateStore};

pub(crate) use crate::index_publication_inner::{
    IndexPublicationOutcome, recover_interrupted_publication,
};

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use athanor_core::{CanonicalSnapshotStore, KnowledgeStore, SnapshotBatch};
    use athanor_domain::{RepoId, SnapshotBase};
    use athanor_store_jsonl::JsonlKnowledgeStore;

    use super::*;
    use crate::{AffectedFileSet, IndexPipelineMetrics};

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
