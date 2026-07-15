include!("index_publication_finalize_tests_inner.rs");

#[async_trait]
impl athanor_core::AtomicSnapshotPublication for SabotagingStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        athanor_core::AtomicSnapshotPublication::publish_snapshot_batch(
            &self.inner,
            snapshot,
            batch,
        )
        .await?;
        self.sabotage()
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        athanor_core::AtomicSnapshotPublication::publish_snapshot_batch_with_context(
            &self.inner,
            snapshot,
            batch,
            context,
        )
        .await?;
        self.sabotage()
    }
}

#[tokio::test]
async fn read_model_finalize_failure_recovers_via_snapshot_api() {
    assert_finalize_failure_recovers_via_snapshot_api(
        SabotageMode::ReadModelFinalize,
        "failed to remove read model backup",
    )
    .await;
}

#[tokio::test]
async fn index_state_finalize_failure_recovers_via_snapshot_api() {
    assert_finalize_failure_recovers_via_snapshot_api(
        SabotageMode::IndexStateFinalize,
        "failed to remove index state backup",
    )
    .await;
}

#[tokio::test]
async fn journal_clear_failure_recovers_via_snapshot_api() {
    assert_finalize_failure_recovers_via_snapshot_api(
        SabotageMode::JournalClear,
        "failed to clear publication journal",
    )
    .await;
}

async fn assert_finalize_failure_recovers_via_snapshot_api(
    mode: SabotageMode,
    expected_error: &str,
) {
    let root = test_root(&format!("{}-snapshot-api", mode.label()));
    let DirectPublicationFixture {
        backend,
        store,
        snapshot,
        output,
        operation,
        state_store,
        output_dir,
    } = direct_fixture(&root, mode).await;

    let error = crate::index_publication::publish_index_snapshot(
        &root,
        &store,
        &state_store,
        &output_dir,
        &output,
        snapshot.clone(),
        &operation,
    )
    .await
    .expect_err("injected finalization fault must fail publication");

    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(expected_error)),
        "expected `{expected_error}` in error chain: {error:#}"
    );
    assert_latest_snapshot(&store, &snapshot).await;
    assert_current_generation(&root, &snapshot);
    assert!(publication_journal(&root).exists());

    backend.repair();
    recover_interrupted_publication(&root, &store)
        .await
        .expect("recover committed generation after transient finalization fault");

    assert_latest_snapshot(&store, &snapshot).await;
    assert_current_generation(&root, &snapshot);
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove direct finalization fault fixture");
}

struct DirectPublicationFixture {
    backend: SabotagingStore,
    store: AthanorStore,
    snapshot: SnapshotId,
    output: IndexPipelineOutput,
    operation: OperationContext,
    state_store: IndexStateStore,
    output_dir: PathBuf,
}

async fn direct_fixture(root: &Path, mode: SabotageMode) -> DirectPublicationFixture {
    let backend = SabotagingStore::new(root, mode);
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    seed_previous_generation(&backend.inner, &output_dir, &state_store).await;

    let store = AthanorStore::new(backend.clone());
    let snapshot = store
        .begin_snapshot(
            RepoId(format!("repo_finalize_direct_{}", mode.label())),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin direct snapshot");
    store
        .put_snapshot(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect("write direct snapshot batch");
    let operation = OperationContext::new(format!(
        "test.publication.direct.{}",
        mode.label()
    ));
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

    DirectPublicationFixture {
        backend,
        store,
        snapshot,
        output,
        operation,
        state_store,
        output_dir,
    }
}
